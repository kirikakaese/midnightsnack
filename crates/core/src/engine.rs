// SPDX-License-Identifier: GPL-3.0-or-later
//! The cue engine: owns the show and the live state and applies actions to them.

use std::path::PathBuf;

use crate::model::{is_valid_color, new_id, split_text, Asset, Cue, CueContent, MediaRef, Show};
use std::collections::BTreeMap;

use crate::protocol::{
    Action, CaptureSource, Countdown, CueRef, Drawing, ErrorCode, LiveState, Masters, MediaOptions,
    MediaPlayback, OutputDef, OutputFeed, OutputLive, Overlay, OverlayKind, Position, ShowSnapshot,
    StateSummary, Stopwatch, Stroke, TestPattern, TextTheme, TimerCue, TimerMode, Transition,
    WebInfo,
};

/// Longest accepted duration for timers, countdowns and auto-advance (24 h).
const MAX_DURATION_MS: u32 = 24 * 60 * 60 * 1000;
/// Longest accepted text cue source.
const MAX_TEXT_CHARS: usize = 100_000;

/// What an applied action changed, so callers only broadcast what is needed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Change {
    pub show: bool,
    pub live: bool,
}

impl Change {
    pub const NONE: Change = Change {
        show: false,
        live: false,
    };
    pub const LIVE: Change = Change {
        show: false,
        live: true,
    };
    pub const SHOW: Change = Change {
        show: true,
        live: false,
    };
    pub const BOTH: Change = Change {
        show: true,
        live: true,
    };

    pub fn merge(self, other: Change) -> Change {
        Change {
            show: self.show || other.show,
            live: self.live || other.live,
        }
    }

    pub fn any(self) -> bool {
        self.show || self.live
    }
}

#[derive(Debug, Clone, Default)]
struct Live {
    program: Option<Position>,
    /// While frozen, outputs keep what they show and stop following the program.
    frozen: bool,
    /// What each program output shows. An output keeps its last targeted cue.
    outputs: BTreeMap<String, Option<Position>>,
    test_pattern: Option<TestPattern>,
    capture_lost: Vec<String>,
    web_nav: Option<crate::protocol::WebNav>,
    drawing: Option<Drawing>,
    blackout: bool,
    logo: bool,
    show_timer: Stopwatch,
    slide_timer: Stopwatch,
    media: Option<MediaPlayback>,
    countdown: Countdown,
    overlays_visible: Vec<String>,
    stage_message: Option<String>,
}

/// Show + live state. All mutation goes through [`Engine::apply`] or the explicit helpers used
/// by host-level services (adding cues, replacing the show).
#[derive(Debug, Clone)]
pub struct Engine {
    show: Show,
    live: Live,
    show_revision: u64,
    live_revision: u64,
    dirty: bool,
    path: Option<PathBuf>,
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new(Show::default())
    }
}

impl Engine {
    pub fn new(show: Show) -> Self {
        Engine {
            show,
            live: Live::default(),
            show_revision: 1,
            live_revision: 1,
            dirty: false,
            path: None,
        }
    }

    pub fn show(&self) -> &Show {
        &self.show
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn program(&self) -> Option<&Position> {
        self.live.program.as_ref()
    }

    /// What the main output shows (ignoring blackout/logo).
    pub fn output(&self) -> Option<&Position> {
        self.output_position(self.show.main_output_id())
    }

    /// What a given output shows.
    pub fn output_position(&self, output_id: &str) -> Option<&Position> {
        self.live.outputs.get(output_id).and_then(Option::as_ref)
    }

    fn program_outputs(&self) -> impl Iterator<Item = &OutputDef> {
        self.show
            .outputs
            .iter()
            .filter(|o| o.feed == OutputFeed::Program)
    }

    /// Sends the program to every output its cue targets (unless frozen).
    fn route_program(&mut self) {
        if self.live.frozen {
            return;
        }
        let program = self.live.program.clone();
        let targets = program
            .as_ref()
            .and_then(|p| self.show.cue(&p.cue_id))
            .map(|c| c.targets.clone());
        let ids: Vec<String> = self.program_outputs().map(|o| o.id.clone()).collect();
        for id in ids {
            let targeted = match &targets {
                // Nothing live: every output goes empty.
                None => true,
                Some(None) => true,
                Some(Some(list)) => list.contains(&id),
            };
            if targeted {
                self.live.outputs.insert(id, program.clone());
            }
        }
    }

    pub fn masters(&self) -> Masters {
        Masters {
            blackout: self.live.blackout,
            freeze: self.live.frozen,
            logo: self.live.logo,
        }
    }

    // ---------------------------------------------------------------- navigation helpers

    fn cue_at(&self, index: usize) -> Option<&Cue> {
        self.show.cues.get(index)
    }

    fn first_slide_from(&self, start: usize) -> Option<Position> {
        self.show.cues[start.min(self.show.cues.len())..]
            .iter()
            .find(|c| c.slide_count() > 0)
            .map(|c| Position {
                cue_id: c.id.clone(),
                slide: 0,
            })
    }

    fn last_slide_before(&self, end: usize) -> Option<Position> {
        self.show.cues[..end.min(self.show.cues.len())]
            .iter()
            .rev()
            .find(|c| c.slide_count() > 0)
            .map(|c| Position {
                cue_id: c.id.clone(),
                slide: c.slide_count() - 1,
            })
    }

    fn is_valid(&self, p: &Position) -> bool {
        self.show
            .cue(&p.cue_id)
            .is_some_and(|c| p.slide < c.slide_count())
    }

    /// The live cue is a web page that receives next/prev as arrow keys.
    pub fn forwards_keys(&self) -> bool {
        self.live
            .program
            .as_ref()
            .and_then(|p| self.show.cue(&p.cue_id))
            .is_some_and(|c| matches!(&c.content, CueContent::Web { web } if web.forward_keys))
    }

    fn forward_key(&mut self, forward: bool) -> Change {
        let cue_id = self
            .live
            .program
            .as_ref()
            .map(|p| p.cue_id.clone())
            .unwrap_or_default();
        let seq = self.live.web_nav.as_ref().map_or(1, |n| n.seq + 1);
        self.live.web_nav = Some(crate::protocol::WebNav {
            cue_id,
            forward,
            seq,
        });
        Change::LIVE
    }

    /// Position `next` would move to.
    pub fn next_position(&self) -> Option<Position> {
        let Some(p) = &self.live.program else {
            return self.first_slide_from(0);
        };
        let idx = self.show.cue_index(&p.cue_id)?;
        let cue = self.cue_at(idx)?;
        if p.slide + 1 < cue.slide_count() {
            Some(Position {
                cue_id: p.cue_id.clone(),
                slide: p.slide + 1,
            })
        } else {
            self.first_slide_from(idx + 1)
        }
    }

    /// Position `prev` would move to.
    pub fn prev_position(&self) -> Option<Position> {
        let p = self.live.program.as_ref()?;
        let idx = self.show.cue_index(&p.cue_id)?;
        if p.slide > 0 {
            Some(Position {
                cue_id: p.cue_id.clone(),
                slide: p.slide - 1,
            })
        } else {
            self.last_slide_before(idx)
        }
    }

    fn next_cue_position(&self) -> Option<Position> {
        match &self.live.program {
            None => self.first_slide_from(0),
            Some(p) => self.first_slide_from(self.show.cue_index(&p.cue_id)? + 1),
        }
    }

    fn prev_cue_position(&self) -> Option<Position> {
        let p = self.live.program.as_ref()?;
        let idx = self.show.cue_index(&p.cue_id)?;
        if p.slide > 0 {
            return Some(Position {
                cue_id: p.cue_id.clone(),
                slide: 0,
            });
        }
        self.show.cues[..idx]
            .iter()
            .rev()
            .find(|c| c.slide_count() > 0)
            .map(|c| Position {
                cue_id: c.id.clone(),
                slide: 0,
            })
    }

    fn set_program(&mut self, target: Option<Position>, now_ms: i64) -> Change {
        if target == self.live.program {
            return Change::NONE;
        }
        // The show clock starts the first time something goes live.
        if target.is_some()
            && self.live.program.is_none()
            && !self.live.show_timer.is_running()
            && self.live.show_timer.accumulated_ms == 0
        {
            self.live.show_timer.running_since_ms = Some(now_ms);
        }
        self.live.program = target;
        self.live.slide_timer = Stopwatch {
            accumulated_ms: 0,
            running_since_ms: self.live.program.as_ref().map(|_| now_ms),
        };
        self.route_program();
        Change::LIVE
    }

    /// Jumps to a slide. Exposed for host services and tests.
    pub fn go_to(&mut self, cue_id: &str, slide: u32, now_ms: i64) -> Result<Change, ErrorCode> {
        let p = Position {
            cue_id: cue_id.to_owned(),
            slide,
        };
        if !self.is_valid(&p) {
            return Err(ErrorCode::NotFound);
        }
        Ok(self.set_program(Some(p), now_ms))
    }

    // ---------------------------------------------------------------- snapshots

    pub fn show_snapshot(&self, include_path: bool) -> ShowSnapshot {
        ShowSnapshot {
            id: self.show.id.clone(),
            title: self.show.title.clone(),
            cues: self.show.cues.iter().map(Cue::summary).collect(),
            path: if include_path {
                self.path.as_ref().map(|p| p.to_string_lossy().into_owned())
            } else {
                None
            },
            dirty: self.dirty,
            revision: self.show_revision,
            default_transition: self.show.default_transition,
            default_theme: self.show.default_theme.clone(),
            overlays: self.show.overlays.clone(),
            logo: self.show.logo.clone(),
            outputs: self.show.outputs.clone(),
        }
    }

    /// Compact state for control surfaces.
    pub fn summary(&self, now_ms: i64) -> StateSummary {
        let describe = |p: Option<&Position>| -> Option<CueRef> {
            let p = p?;
            let index = self.show.cues.iter().position(|c| c.id == p.cue_id)?;
            let cue = &self.show.cues[index];
            Some(CueRef {
                cue_id: cue.id.clone(),
                name: cue.name.clone(),
                cue_number: index as u32 + 1,
                slide: p.slide + 1,
                slide_count: cue.slide_count(),
            })
        };
        let countdown = &self.live.countdown;
        StateSummary {
            show_title: self.show.title.clone(),
            cue_count: self.show.cues.len() as u32,
            program: describe(self.live.program.as_ref()),
            output: describe(self.output()),
            next: describe(self.next_position().as_ref()),
            masters: self.masters(),
            show_timer_ms: self.live.show_timer.elapsed_ms(now_ms),
            slide_timer_ms: self.live.slide_timer.elapsed_ms(now_ms),
            countdown_remaining_ms: countdown.duration_ms as i64
                - countdown.elapsed.elapsed_ms(now_ms),
            countdown_running: countdown.elapsed.is_running(),
            overlays_visible: self.live.overlays_visible.clone(),
        }
    }

    pub fn live_state(&self, now_ms: i64) -> LiveState {
        LiveState {
            program: self.live.program.clone(),
            output: self.output().cloned(),
            next: self.next_position(),
            prev: self.prev_position(),
            masters: self.masters(),
            show_timer: self.live.show_timer,
            slide_timer: self.live.slide_timer,
            media: self.live.media.clone(),
            countdown: self.live.countdown.clone(),
            overlays_visible: self.live.overlays_visible.clone(),
            stage_message: self.live.stage_message.clone(),
            outputs: self
                .program_outputs()
                .map(|o| OutputLive {
                    output_id: o.id.clone(),
                    position: self.output_position(&o.id).cloned(),
                })
                .collect(),
            test_pattern: self.live.test_pattern,
            capture_lost: self.live.capture_lost.clone(),
            web_nav: self.live.web_nav.clone(),
            drawing: self.live.drawing.clone(),
            auto_advance_at_ms: self.auto_advance_at(),
            host_time_ms: now_ms,
            revision: self.live_revision,
        }
    }

    // ---------------------------------------------------------------- mutation

    fn bump(&mut self, change: Change) -> Change {
        if change.show {
            self.show_revision += 1;
            self.dirty = true;
        }
        if change.live {
            self.live_revision += 1;
        }
        change
    }

    /// Applies an engine-level action. Host-level actions (files, devices) return
    /// `InvalidState`; the dispatcher routes those to host services instead.
    pub fn apply(&mut self, action: &Action, now_ms: i64) -> Result<Change, ErrorCode> {
        let mut change = self.apply_inner(action, now_ms)?;
        if change.show && self.revalidate(now_ms) {
            change.live = true;
        }
        if self.sync_media(now_ms) {
            change.live = true;
        }
        if self.reconcile_drawing() {
            change.live = true;
        }
        // Metadata reported by the output is not an edit by the user.
        let was_dirty = self.dirty;
        let change = self.bump(change);
        if matches!(action, Action::MediaLoaded { .. }) {
            self.dirty = was_dirty;
        }
        Ok(change)
    }

    /// Drawings belong to the slide on the main output; drop them once it shows something else.
    fn reconcile_drawing(&mut self) -> bool {
        let stale = self
            .live
            .drawing
            .as_ref()
            .is_some_and(|d| self.output() != Some(&d.position));
        if stale {
            self.live.drawing = None;
        }
        stale
    }

    /// Keeps positions valid after the cue list changed (e.g. a text cue lost slides).
    fn revalidate(&mut self, now_ms: i64) -> bool {
        let fix = |show: &Show, p: &Position| -> Option<Position> {
            let count = show.cue(&p.cue_id)?.slide_count();
            (count > 0).then(|| Position {
                cue_id: p.cue_id.clone(),
                slide: p.slide.min(count - 1),
            })
        };
        let mut changed = false;
        if let Some(p) = self.live.program.clone() {
            let fixed = fix(&self.show, &p);
            if fixed.as_ref() != Some(&p) {
                self.live.program = None;
                self.set_program(fixed, now_ms);
                changed = true;
            }
        }
        for slot in self.live.outputs.values_mut() {
            if let Some(p) = slot.clone() {
                let fixed = fix(&self.show, &p);
                if fixed.as_ref() != Some(&p) {
                    *slot = fixed;
                    changed = true;
                }
            }
        }
        changed
    }

    /// Media playback follows the cue on the output. Returns true if it changed.
    fn sync_media(&mut self, now_ms: i64) -> bool {
        let output_cue = self.output().and_then(|p| self.show.cue(&p.cue_id));
        let wanted = match output_cue.map(|c| (&c.id, &c.content)) {
            Some((id, CueContent::Media { options, .. })) => Some((id.clone(), *options)),
            _ => None,
        };
        match (wanted, &self.live.media) {
            (None, None) => false,
            (None, Some(_)) => {
                self.live.media = None;
                true
            }
            (Some((id, _)), Some(m)) if m.cue_id == id => false,
            (Some((id, options)), _) => {
                self.live.media = Some(MediaPlayback {
                    cue_id: id,
                    position: Stopwatch {
                        accumulated_ms: options.start_ms as i64,
                        running_since_ms: Some(now_ms),
                    },
                    ended: false,
                });
                true
            }
        }
    }

    fn media_cue(&self) -> Option<(&MediaPlayback, MediaOptions, Option<u32>)> {
        let m = self.live.media.as_ref()?;
        match &self.show.cue(&m.cue_id)?.content {
            CueContent::Media {
                options,
                duration_ms,
                ..
            } => Some((m, *options, *duration_ms)),
            _ => None,
        }
    }

    /// When the program should advance on its own, if ever.
    pub fn auto_advance_at(&self) -> Option<i64> {
        let p = self.live.program.as_ref()?;
        let cue = self.show.cue(&p.cue_id)?;
        let mut due: Option<i64> = None;
        if let (Some(ms), Some(since)) =
            (cue.auto_advance_ms, self.live.slide_timer.running_since_ms)
        {
            due = Some(since - self.live.slide_timer.accumulated_ms + ms as i64);
        }
        if let Some((m, options, duration)) = self.media_cue() {
            let end = options.end_ms.or(duration);
            if m.cue_id == cue.id && options.auto_advance && !options.loop_playback && !m.ended {
                if let (Some(end), Some(since)) = (end, m.position.running_since_ms) {
                    let at = since + (end as i64 - m.position.accumulated_ms).max(0);
                    due = Some(due.map_or(at, |d| d.min(at)));
                }
            }
        }
        due
    }

    /// Advances the program if an auto-advance is due. Called periodically by the host.
    pub fn tick(&mut self, now_ms: i64) -> Change {
        match self.auto_advance_at() {
            Some(at) if at <= now_ms => {
                let target = self.next_position();
                let mut c = match target {
                    Some(t) => self.set_program(Some(t), now_ms),
                    // End of show: stop trying.
                    None => {
                        self.live.slide_timer.running_since_ms = None;
                        self.stop_media_at_end();
                        Change::LIVE
                    }
                };
                if self.sync_media(now_ms) {
                    c.live = true;
                }
                self.bump(c)
            }
            _ => Change::NONE,
        }
    }

    fn stop_media_at_end(&mut self) {
        let end = self
            .media_cue()
            .map(|(m, o, d)| (o.end_ms.or(d), m.position));
        if let (Some(m), Some((end, pos))) = (self.live.media.as_mut(), end) {
            m.position = Stopwatch {
                accumulated_ms: end.map_or(pos.accumulated_ms, |e| e as i64),
                running_since_ms: None,
            };
            m.ended = true;
        }
    }

    fn media_mut(&mut self) -> Result<&mut MediaPlayback, ErrorCode> {
        self.live.media.as_mut().ok_or(ErrorCode::InvalidState)
    }

    fn apply_inner(&mut self, action: &Action, now_ms: i64) -> Result<Change, ErrorCode> {
        use Action::*;
        Ok(match action {
            Go | Next if self.forwards_keys() => self.forward_key(true),
            Prev if self.forwards_keys() => self.forward_key(false),
            Go | Next => {
                let target = self.next_position();
                match target {
                    Some(t) => self.set_program(Some(t), now_ms),
                    None => Change::NONE,
                }
            }
            Prev => match self.prev_position() {
                Some(t) => self.set_program(Some(t), now_ms),
                None => Change::NONE,
            },
            NextCue => match self.next_cue_position() {
                Some(t) => self.set_program(Some(t), now_ms),
                None => Change::NONE,
            },
            PrevCue => match self.prev_cue_position() {
                Some(t) => self.set_program(Some(t), now_ms),
                None => Change::NONE,
            },
            GoTo { position } => self.go_to(&position.cue_id, position.slide, now_ms)?,

            SetBlackout { on } => self.set_flag(|l| &mut l.blackout, *on),
            ToggleBlackout => self.set_flag(|l| &mut l.blackout, !self.live.blackout),
            SetLogo { on } => self.set_flag(|l| &mut l.logo, *on),
            ToggleLogo => self.set_flag(|l| &mut l.logo, !self.live.logo),
            SetFreeze { on } => self.set_freeze(*on),
            ToggleFreeze => self.set_freeze(!self.live.frozen),
            Panic => {
                let c1 = self.set_flag(|l| &mut l.logo, true);
                let c2 = self.set_freeze(false);
                c1.merge(c2)
            }

            TimerStart => {
                if self.live.show_timer.is_running() {
                    Change::NONE
                } else {
                    self.live.show_timer.running_since_ms = Some(now_ms);
                    Change::LIVE
                }
            }
            TimerPause => {
                let t = &mut self.live.show_timer;
                match t.running_since_ms.take() {
                    Some(since) => {
                        t.accumulated_ms += (now_ms - since).max(0);
                        Change::LIVE
                    }
                    None => Change::NONE,
                }
            }
            TimerReset => {
                let running = self.live.show_timer.is_running();
                self.live.show_timer = Stopwatch {
                    accumulated_ms: 0,
                    running_since_ms: running.then_some(now_ms),
                };
                if self.live.program.is_some() {
                    self.live.slide_timer = Stopwatch {
                        accumulated_ms: 0,
                        running_since_ms: Some(now_ms),
                    };
                }
                Change::LIVE
            }

            MediaPlay => {
                let m = self.media_mut()?;
                if m.position.is_running() {
                    Change::NONE
                } else {
                    if m.ended {
                        m.ended = false;
                    }
                    m.position.running_since_ms = Some(now_ms);
                    Change::LIVE
                }
            }
            MediaPause => {
                let m = self.media_mut()?;
                match m.position.running_since_ms.take() {
                    Some(since) => {
                        m.position.accumulated_ms += (now_ms - since).max(0);
                        Change::LIVE
                    }
                    None => Change::NONE,
                }
            }
            MediaSeek { position_ms } => {
                let m = self.media_mut()?;
                m.position.accumulated_ms = *position_ms as i64;
                if m.position.is_running() || m.ended {
                    m.position.running_since_ms = Some(now_ms);
                }
                m.ended = false;
                Change::LIVE
            }
            MediaRestart => {
                let start = self.media_cue().ok_or(ErrorCode::InvalidState)?.1.start_ms;
                let m = self.media_mut()?;
                m.position = Stopwatch {
                    accumulated_ms: start as i64,
                    running_since_ms: Some(now_ms),
                };
                m.ended = false;
                Change::LIVE
            }
            MediaLoaded {
                cue_id,
                duration_ms,
            } => match &mut self.cue_mut(cue_id)?.content {
                CueContent::Media { duration_ms: d, .. } if *d != Some(*duration_ms) => {
                    *d = Some(*duration_ms);
                    Change::BOTH
                }
                CueContent::Media { .. } => Change::NONE,
                _ => return Err(ErrorCode::InvalidState),
            },
            MediaEnded { cue_id } => {
                let Some((m, options, _)) = self.media_cue() else {
                    return Ok(Change::NONE);
                };
                if m.cue_id != *cue_id || options.loop_playback || m.ended {
                    return Ok(Change::NONE);
                }
                self.stop_media_at_end();
                let on_program = self
                    .live
                    .program
                    .as_ref()
                    .is_some_and(|p| p.cue_id == *cue_id);
                if options.auto_advance && on_program {
                    if let Some(t) = self.next_position() {
                        self.set_program(Some(t), now_ms);
                    }
                }
                Change::LIVE
            }

            CountdownSet { duration_ms, label } => {
                if *duration_ms == 0 || *duration_ms > MAX_DURATION_MS {
                    return Err(ErrorCode::InvalidState);
                }
                self.live.countdown = Countdown {
                    duration_ms: *duration_ms,
                    label: clean_text(label, 100),
                    elapsed: Stopwatch::default(),
                };
                Change::LIVE
            }
            CountdownStart => {
                let sw = &mut self.live.countdown.elapsed;
                if sw.is_running() {
                    Change::NONE
                } else {
                    sw.running_since_ms = Some(now_ms);
                    Change::LIVE
                }
            }
            CountdownPause => {
                let sw = &mut self.live.countdown.elapsed;
                match sw.running_since_ms.take() {
                    Some(since) => {
                        sw.accumulated_ms += (now_ms - since).max(0);
                        Change::LIVE
                    }
                    None => Change::NONE,
                }
            }
            CountdownReset => {
                self.live.countdown.elapsed = Stopwatch::default();
                Change::LIVE
            }
            SetStageMessage { text } => {
                let text = text
                    .as_deref()
                    .map(|t| clean_multiline(t, 500))
                    .filter(|t| !t.is_empty());
                if text == self.live.stage_message {
                    Change::NONE
                } else {
                    self.live.stage_message = text;
                    Change::LIVE
                }
            }

            SetOverlayVisible {
                overlay_id,
                visible,
            } => self.set_overlay_visible(overlay_id, *visible)?,
            ToggleOverlay { overlay_id } => {
                let visible = !self.live.overlays_visible.contains(overlay_id);
                self.set_overlay_visible(overlay_id, visible)?
            }
            PutOverlay { overlay } => {
                let overlay = self.validate_overlay(overlay)?;
                match self.show.overlays.iter_mut().find(|o| o.id == overlay.id) {
                    Some(o) => *o = overlay,
                    None => self.show.overlays.push(overlay),
                }
                self.show.prune_assets();
                Change::SHOW
            }
            RemoveOverlay { overlay_id } => {
                let before = self.show.overlays.len();
                self.show.overlays.retain(|o| o.id != *overlay_id);
                if before == self.show.overlays.len() {
                    return Err(ErrorCode::NotFound);
                }
                self.live.overlays_visible.retain(|id| id != overlay_id);
                self.show.prune_assets();
                Change::BOTH
            }

            AddText {
                name,
                text,
                lyrics,
                at_index,
            } => {
                let content = text_content(text, *lyrics, None)?;
                let name = clean_text(name, 200);
                let cue = Cue::new(if name.is_empty() { "Text".into() } else { name }, content);
                self.insert_cues_inner(vec![cue], at_index.map(|i| i as usize))
            }
            SetCueText {
                cue_id,
                text,
                lyrics,
            } => {
                let cue = self.cue_mut(cue_id)?;
                let CueContent::Text { theme, .. } = &cue.content else {
                    return Err(ErrorCode::InvalidState);
                };
                cue.content = text_content(text, *lyrics, theme.clone())?;
                Change::SHOW
            }
            AddTimer {
                name,
                timer,
                at_index,
            } => {
                let timer = validate_timer(timer, &self.show)?;
                let name = clean_text(name, 200);
                let cue = Cue::new(
                    if name.is_empty() {
                        "Timer".into()
                    } else {
                        name
                    },
                    CueContent::Timer { timer },
                );
                self.insert_cues_inner(vec![cue], at_index.map(|i| i as usize))
            }
            SetCueTimer { cue_id, timer } => {
                let timer = validate_timer(timer, &self.show)?;
                let cue = self.cue_mut(cue_id)?;
                if !matches!(cue.content, CueContent::Timer { .. }) {
                    return Err(ErrorCode::InvalidState);
                }
                cue.content = CueContent::Timer { timer };
                self.show.prune_assets();
                Change::SHOW
            }
            SetCueTheme { cue_id, theme } => {
                let theme = theme
                    .as_ref()
                    .map(|t| validate_theme(t, &self.show))
                    .transpose()?;
                let slot = self
                    .cue_mut(cue_id)?
                    .content
                    .theme_mut()
                    .ok_or(ErrorCode::InvalidState)?;
                *slot = theme;
                self.show.prune_assets();
                Change::SHOW
            }
            SetDefaultTheme { theme } => {
                self.show.default_theme = validate_theme(theme, &self.show)?;
                self.show.prune_assets();
                Change::SHOW
            }
            SetCueTransition { cue_id, transition } => {
                let t = transition.map(validate_transition).transpose()?;
                self.cue_mut(cue_id)?.transition = t;
                Change::SHOW
            }
            SetDefaultTransition { transition } => {
                self.show.default_transition = validate_transition(*transition)?;
                Change::SHOW
            }
            SetCueAutoAdvance { cue_id, after_ms } => {
                if after_ms.is_some_and(|ms| !(100..=MAX_DURATION_MS).contains(&ms)) {
                    return Err(ErrorCode::InvalidState);
                }
                self.cue_mut(cue_id)?.auto_advance_ms = *after_ms;
                Change::BOTH
            }
            SetMediaOptions { cue_id, options } => {
                let valid_volume = (0.0..=1.0).contains(&options.volume);
                let valid_trim = options.end_ms.is_none_or(|e| e > options.start_ms);
                if !valid_volume || !valid_trim {
                    return Err(ErrorCode::InvalidState);
                }
                match &mut self.cue_mut(cue_id)?.content {
                    CueContent::Media { options: o, .. } => *o = *options,
                    _ => return Err(ErrorCode::InvalidState),
                }
                Change::BOTH
            }

            DrawStroke { stroke } => {
                let stroke = validate_stroke(stroke)?;
                let Some(position) = self.output().cloned() else {
                    return Err(ErrorCode::InvalidState);
                };
                let drawing = self.live.drawing.get_or_insert_with(|| Drawing {
                    position,
                    strokes: Vec::new(),
                });
                drawing.strokes.push(stroke);
                if drawing.strokes.len() > Drawing::MAX_STROKES {
                    drawing.strokes.remove(0);
                }
                Change::LIVE
            }
            ClearDrawing => {
                if self.live.drawing.take().is_some() {
                    Change::LIVE
                } else {
                    Change::NONE
                }
            }
            SetTestPattern { pattern } => {
                if self.live.test_pattern == *pattern {
                    Change::NONE
                } else {
                    self.live.test_pattern = *pattern;
                    Change::LIVE
                }
            }
            PutOutput { output } => {
                let output = validate_output(output)?;
                match self.show.outputs.iter_mut().find(|o| o.id == output.id) {
                    Some(o) => *o = output,
                    None => self.show.outputs.push(output),
                }
                if self.program_outputs().next().is_none() {
                    return Err(ErrorCode::InvalidState);
                }
                self.live.outputs.retain(|id, _| {
                    self.show
                        .outputs
                        .iter()
                        .any(|o| o.id == *id && o.feed == OutputFeed::Program)
                });
                // A new output starts with what the program would show there.
                let program = self.live.program.clone();
                let main = self.output().cloned();
                for o in self
                    .show
                    .outputs
                    .iter()
                    .filter(|o| o.feed == OutputFeed::Program)
                {
                    self.live
                        .outputs
                        .entry(o.id.clone())
                        .or_insert_with(|| main.clone().or(program.clone()));
                }
                Change::BOTH
            }
            RemoveOutput { output_id } => {
                let before = self.show.outputs.len();
                let removed: Vec<OutputDef> = self
                    .show
                    .outputs
                    .iter()
                    .filter(|o| o.id == *output_id)
                    .cloned()
                    .collect();
                self.show.outputs.retain(|o| o.id != *output_id);
                if before == self.show.outputs.len() {
                    return Err(ErrorCode::NotFound);
                }
                if self.program_outputs().next().is_none() {
                    self.show.outputs.extend(removed);
                    return Err(ErrorCode::InvalidState);
                }
                self.live.outputs.remove(output_id);
                for cue in &mut self.show.cues {
                    if let Some(t) = &mut cue.targets {
                        t.retain(|id| id != output_id);
                    }
                }
                Change::BOTH
            }
            SetCueTargets { cue_id, targets } => {
                if let Some(list) = targets {
                    if list
                        .iter()
                        .any(|id| !self.show.outputs.iter().any(|o| o.id == *id))
                    {
                        return Err(ErrorCode::NotFound);
                    }
                }
                let mut list = targets.clone();
                if let Some(l) = &mut list {
                    l.dedup();
                }
                self.cue_mut(cue_id)?.targets = list;
                if self
                    .live
                    .program
                    .as_ref()
                    .is_some_and(|p| p.cue_id == *cue_id)
                {
                    self.route_program();
                }
                Change::BOTH
            }
            AddWeb {
                name,
                web,
                at_index,
            } => {
                let web = validate_web(web)?;
                let name = clean_text(name, 200);
                let cue = Cue::new(
                    if name.is_empty() {
                        web.url.clone()
                    } else {
                        name
                    },
                    CueContent::Web { web },
                );
                self.insert_cues_inner(vec![cue], at_index.map(|i| i as usize))
            }
            SetWebOptions { cue_id, web } => {
                let web = validate_web(web)?;
                match &mut self.cue_mut(cue_id)?.content {
                    CueContent::Web { web: w } => *w = web,
                    _ => return Err(ErrorCode::InvalidState),
                }
                Change::SHOW
            }
            AddCapture {
                name,
                source,
                at_index,
            } => {
                let source = validate_capture_source(source)?;
                let name = clean_text(name, 200);
                let capture = crate::protocol::CaptureInfo { source, fps: 30 };
                let cue = Cue::new(
                    if name.is_empty() {
                        "Capture".into()
                    } else {
                        name
                    },
                    CueContent::Capture { capture },
                );
                self.insert_cues_inner(vec![cue], at_index.map(|i| i as usize))
            }
            SetCapture {
                cue_id,
                source,
                fps,
            } => {
                let source = validate_capture_source(source)?;
                if !(1..=60).contains(fps) {
                    return Err(ErrorCode::InvalidState);
                }
                match &mut self.cue_mut(cue_id)?.content {
                    CueContent::Capture { capture } => {
                        capture.source = source;
                        capture.fps = *fps;
                    }
                    _ => return Err(ErrorCode::InvalidState),
                }
                Change::SHOW
            }

            RenameShow { title } => {
                self.show.title = clean_text(title, 200);
                Change::SHOW
            }
            RenameCue { cue_id, name } => {
                self.cue_mut(cue_id)?.name = clean_text(name, 200);
                Change::SHOW
            }
            SetCueNotes { cue_id, notes } => {
                self.cue_mut(cue_id)?.notes = notes.chars().take(20_000).collect();
                Change::SHOW
            }
            SetCueColor { cue_id, color } => {
                if let Some(c) = color {
                    if !is_valid_color(c) {
                        return Err(ErrorCode::InvalidState);
                    }
                }
                self.cue_mut(cue_id)?.color = color.clone();
                Change::SHOW
            }
            MoveCue { cue_id, to_index } => {
                let from = self.show.cue_index(cue_id).ok_or(ErrorCode::NotFound)?;
                let cue = self.show.cues.remove(from);
                let to = (*to_index as usize).min(self.show.cues.len());
                self.show.cues.insert(to, cue);
                // Program stays on the same cue; next/prev are recomputed.
                Change::BOTH
            }
            RemoveCue { cue_id } => self.remove_cue(cue_id, now_ms)?,
            AddBlank { color, at_index } => {
                if !is_valid_color(color) {
                    return Err(ErrorCode::InvalidState);
                }
                let cue = Cue::new(
                    "Blank",
                    CueContent::Blank {
                        color: color.clone(),
                    },
                );
                self.insert_cues_inner(vec![cue], at_index.map(|i| i as usize))
            }

            AddFiles { .. }
            | SetLogoImage { .. }
            | SetBackgroundImage { .. }
            | SetOverlayImage { .. }
            | NewShow
            | OpenShow { .. }
            | SaveShow { .. }
            | ApprovePairing { .. }
            | DenyPairing { .. }
            | SetDeviceRole { .. }
            | RevokeDevice { .. }
            | DisconnectAll
            | SetAutoApprove { .. }
            | AcceptUpload { .. }
            | RejectUpload { .. }
            | SetAutoAcceptUploads { .. }
            | SetApiLocalOnly { .. }
            | ConfigureOsc { .. } => return Err(ErrorCode::InvalidState),
        })
    }

    fn set_flag(&mut self, field: impl FnOnce(&mut Live) -> &mut bool, on: bool) -> Change {
        let f = field(&mut self.live);
        if *f == on {
            Change::NONE
        } else {
            *f = on;
            Change::LIVE
        }
    }

    fn set_freeze(&mut self, on: bool) -> Change {
        if on == self.live.frozen {
            return Change::NONE;
        }
        self.live.frozen = on;
        // Releasing the freeze catches the outputs up with the program.
        self.route_program();
        Change::LIVE
    }

    fn set_overlay_visible(&mut self, id: &str, visible: bool) -> Result<Change, ErrorCode> {
        if !self.show.overlays.iter().any(|o| o.id == id) {
            return Err(ErrorCode::NotFound);
        }
        let shown = self.live.overlays_visible.iter().any(|v| v == id);
        Ok(match (visible, shown) {
            (true, false) => {
                self.live.overlays_visible.push(id.to_owned());
                Change::LIVE
            }
            (false, true) => {
                self.live.overlays_visible.retain(|v| v != id);
                Change::LIVE
            }
            _ => Change::NONE,
        })
    }

    fn validate_overlay(&self, o: &Overlay) -> Result<Overlay, ErrorCode> {
        if !is_valid_color(&o.color)
            || !is_valid_color(&o.background)
            || !(10..=400).contains(&o.scale)
        {
            return Err(ErrorCode::InvalidState);
        }
        let kind = match &o.kind {
            OverlayKind::LowerThird { title, subtitle } => OverlayKind::LowerThird {
                title: clean_text(title, 200),
                subtitle: clean_text(subtitle, 200),
            },
            OverlayKind::LogoBug { image } => {
                if image
                    .as_ref()
                    .is_some_and(|id| self.show.asset(id).is_none())
                {
                    return Err(ErrorCode::NotFound);
                }
                OverlayKind::LogoBug {
                    image: image.clone(),
                }
            }
            OverlayKind::Ticker { text, speed } => OverlayKind::Ticker {
                text: clean_text(text, 2000),
                speed: (*speed).clamp(1, 100),
            },
            other => other.clone(),
        };
        let id = if o.id.trim().is_empty() {
            new_id()
        } else {
            clean_text(&o.id, 64)
        };
        Ok(Overlay {
            id,
            name: clean_text(&o.name, 100),
            kind,
            ..o.clone()
        })
    }

    /// Updates a converted PDF cue after its source document changed and was converted again.
    pub fn update_converted(
        &mut self,
        cue_id: &str,
        page_count: u32,
        notes: Vec<String>,
        now_ms: i64,
    ) -> Result<Change, ErrorCode> {
        let cue = self.cue_mut(cue_id)?;
        match &mut cue.content {
            CueContent::Pdf {
                page_count: pc,
                source: Some(_),
                ..
            } => *pc = page_count,
            _ => return Err(ErrorCode::InvalidState),
        }
        cue.slide_notes = notes;
        let mut c = Change::SHOW;
        if self.revalidate(now_ms) {
            c.live = true;
        }
        Ok(self.bump(c))
    }

    /// Records which capture cues have lost their source. Called by the capture service.
    pub fn set_capture_lost(&mut self, mut lost: Vec<String>) -> Change {
        lost.sort();
        lost.dedup();
        if lost == self.live.capture_lost {
            return Change::NONE;
        }
        self.live.capture_lost = lost;
        self.bump(Change::LIVE)
    }

    /// Registers a file as a show asset and returns its id.
    pub fn add_asset(&mut self, file: MediaRef) -> String {
        let id = new_id();
        self.show.assets.push(Asset {
            id: id.clone(),
            file,
        });
        id
    }

    /// Sets (or clears) the logo screen image. Used by host services after checking the file.
    pub fn set_logo_asset(&mut self, asset: Option<String>) -> Change {
        self.show.logo = asset;
        self.show.prune_assets();
        self.bump(Change::SHOW)
    }

    /// Sets (or clears) a background image on a cue's theme or the default theme.
    pub fn set_background_asset(
        &mut self,
        cue_id: Option<&str>,
        asset: Option<String>,
    ) -> Result<Change, ErrorCode> {
        match cue_id {
            None => self.show.default_theme.background_image = asset,
            Some(id) => {
                let default = self.show.default_theme.clone();
                let slot = self
                    .cue_mut(id)?
                    .content
                    .theme_mut()
                    .ok_or(ErrorCode::InvalidState)?;
                slot.get_or_insert(default).background_image = asset;
            }
        }
        self.show.prune_assets();
        Ok(self.bump(Change::SHOW))
    }

    /// Sets (or clears) the image of a logo-bug overlay.
    pub fn set_overlay_asset(
        &mut self,
        overlay_id: &str,
        asset: Option<String>,
    ) -> Result<Change, ErrorCode> {
        let o = self
            .show
            .overlays
            .iter_mut()
            .find(|o| o.id == overlay_id)
            .ok_or(ErrorCode::NotFound)?;
        match &mut o.kind {
            OverlayKind::LogoBug { image } => *image = asset,
            _ => return Err(ErrorCode::InvalidState),
        }
        self.show.prune_assets();
        Ok(self.bump(Change::SHOW))
    }

    fn cue_mut(&mut self, id: &str) -> Result<&mut Cue, ErrorCode> {
        self.show
            .cues
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(ErrorCode::NotFound)
    }

    fn remove_cue(&mut self, cue_id: &str, now_ms: i64) -> Result<Change, ErrorCode> {
        let idx = self.show.cue_index(cue_id).ok_or(ErrorCode::NotFound)?;
        self.show.cues.remove(idx);
        let mut change = Change::SHOW;
        if self
            .live
            .program
            .as_ref()
            .is_some_and(|p| p.cue_id == cue_id)
        {
            // Fall forward to whatever now sits at the removed index, else nothing.
            let target = self.first_slide_from(idx);
            self.live.program = None;
            change = change.merge(self.set_program(target, now_ms));
            change.live = true;
        }
        for slot in self.live.outputs.values_mut() {
            if slot.as_ref().is_some_and(|p| p.cue_id == cue_id) {
                *slot = None;
            }
        }
        Ok(Change {
            live: true,
            ..change
        })
    }

    fn insert_cues_inner(&mut self, cues: Vec<Cue>, at: Option<usize>) -> Change {
        let at = at.unwrap_or(self.show.cues.len()).min(self.show.cues.len());
        self.show.cues.splice(at..at, cues);
        Change::BOTH
    }

    /// Inserts cues created by a host service (e.g. after inspecting added files).
    pub fn insert_cues(&mut self, cues: Vec<Cue>, at: Option<usize>) -> Change {
        let c = self.insert_cues_inner(cues, at);
        self.bump(c)
    }

    /// Like [`Engine::restore_position`] but also starts media; used by tests and services.
    pub fn sync(&mut self, now_ms: i64) -> Change {
        if self.sync_media(now_ms) {
            self.bump(Change::LIVE)
        } else {
            Change::NONE
        }
    }

    /// Replaces the whole show (new / open). Master states are kept so a blackout survives
    /// loading a show; position and timers reset.
    pub fn replace_show(&mut self, show: Show, path: Option<PathBuf>) -> Change {
        self.show = show;
        self.path = path;
        self.live.program = None;
        self.live.outputs.clear();
        self.live.capture_lost.clear();
        self.live.show_timer = Stopwatch::default();
        self.live.slide_timer = Stopwatch::default();
        self.live.media = None;
        self.live.overlays_visible.clear();
        let c = self.bump(Change::BOTH);
        self.dirty = false;
        c
    }

    /// Flags unsaved changes (used when restoring an autosave).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Records a successful save.
    pub fn mark_saved(&mut self, path: PathBuf) -> Change {
        self.path = Some(path);
        self.dirty = false;
        self.show_revision += 1;
        Change::SHOW
    }

    /// Restores a position after crash recovery, if it is still valid.
    pub fn restore_position(&mut self, position: Option<Position>, now_ms: i64) -> Change {
        match position.filter(|p| self.is_valid(p)) {
            Some(p) => {
                let mut c = self.set_program(Some(p), now_ms);
                if self.sync_media(now_ms) {
                    c.live = true;
                }
                self.bump(c)
            }
            None => Change::NONE,
        }
    }

    /// Mutable access for host services that rewrite media references (e.g. after bundling).
    pub fn show_mut_untracked(&mut self) -> &mut Show {
        &mut self.show
    }
}

fn clean_multiline(s: &str, max: usize) -> String {
    s.chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .take(max)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn text_content(
    text: &str,
    lyrics: bool,
    theme: Option<TextTheme>,
) -> Result<CueContent, ErrorCode> {
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(ErrorCode::InvalidState);
    }
    let source = clean_multiline(text, MAX_TEXT_CHARS);
    Ok(CueContent::Text {
        slides: split_text(&source, lyrics),
        source,
        lyrics,
        theme,
    })
}

fn validate_output(o: &OutputDef) -> Result<OutputDef, ErrorCode> {
    // Ids name host windows, so keep them simple.
    let id = o.id.trim().to_owned();
    let id_ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !id_ok || o.margin > 20 {
        return Err(ErrorCode::InvalidState);
    }
    let name = clean_text(&o.name, 100);
    Ok(OutputDef {
        id,
        name: if name.is_empty() {
            "Output".into()
        } else {
            name
        },
        ..o.clone()
    })
}

/// Points are clamped to the slide; empty or oversized strokes are refused.
fn validate_stroke(s: &Stroke) -> Result<Stroke, ErrorCode> {
    if !is_valid_color(&s.color)
        || s.points.is_empty()
        || s.points.len() > Stroke::MAX_POINTS
        || !s.width.is_finite()
        || s.points.iter().flatten().any(|v| !v.is_finite())
    {
        return Err(ErrorCode::InvalidState);
    }
    Ok(Stroke {
        color: s.color.clone(),
        width: s.width.clamp(Stroke::MIN_WIDTH, Stroke::MAX_WIDTH),
        points: s
            .points
            .iter()
            .map(|[x, y]| [x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)])
            .collect(),
    })
}

/// Only plain web pages: no `file:`, `javascript:` or `data:` URLs.
fn validate_web(w: &WebInfo) -> Result<WebInfo, ErrorCode> {
    let url = w.url.trim();
    let lower = url.to_ascii_lowercase();
    let scheme_ok = lower.starts_with("https://") || lower.starts_with("http://");
    let host_ok = url.split("://").nth(1).is_some_and(|rest| {
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        !host.is_empty() && !host.contains(char::is_whitespace)
    });
    if !scheme_ok || !host_ok || url.len() > 2000 || !(25..=400).contains(&w.zoom) {
        return Err(ErrorCode::InvalidState);
    }
    Ok(WebInfo {
        url: url.to_owned(),
        ..w.clone()
    })
}

fn validate_capture_source(s: &CaptureSource) -> Result<CaptureSource, ErrorCode> {
    Ok(match s {
        CaptureSource::Screen { name } if !name.trim().is_empty() => CaptureSource::Screen {
            name: clean_text(name, 200),
        },
        CaptureSource::Window { app, title }
            if !(app.trim().is_empty() && title.trim().is_empty()) =>
        {
            CaptureSource::Window {
                app: clean_text(app, 200),
                title: clean_text(title, 500),
            }
        }
        _ => return Err(ErrorCode::InvalidState),
    })
}

fn validate_transition(t: Transition) -> Result<Transition, ErrorCode> {
    if t.duration_ms > Transition::MAX_DURATION_MS {
        return Err(ErrorCode::InvalidState);
    }
    Ok(t)
}

fn validate_theme(t: &TextTheme, show: &Show) -> Result<TextTheme, ErrorCode> {
    let font_ok = !t.font_family.trim().is_empty()
        && t.font_family.len() <= 200
        && !t.font_family.contains(['{', '}', ';', '<', '>', '\\']);
    if !font_ok
        || !is_valid_color(&t.color)
        || !is_valid_color(&t.background)
        || t.font_size.is_some_and(|s| !(1..=100).contains(&s))
    {
        return Err(ErrorCode::InvalidState);
    }
    if t.background_image
        .as_ref()
        .is_some_and(|id| show.asset(id).is_none())
    {
        return Err(ErrorCode::NotFound);
    }
    Ok(t.clone())
}

fn validate_timer(t: &TimerCue, show: &Show) -> Result<TimerCue, ErrorCode> {
    match &t.mode {
        TimerMode::Countdown { duration_ms }
            if *duration_ms == 0 || *duration_ms > MAX_DURATION_MS =>
        {
            return Err(ErrorCode::InvalidState)
        }
        TimerMode::CountdownTo { time } if !is_valid_clock_time(time) => {
            return Err(ErrorCode::InvalidState)
        }
        _ => {}
    }
    if !is_valid_color(&t.overtime_color) {
        return Err(ErrorCode::InvalidState);
    }
    let theme = t
        .theme
        .as_ref()
        .map(|th| validate_theme(th, show))
        .transpose()?;
    Ok(TimerCue {
        label: clean_text(&t.label, 200),
        theme,
        ..t.clone()
    })
}

/// `HH:MM`, 24-hour clock.
fn is_valid_clock_time(s: &str) -> bool {
    let Some((h, m)) = s.split_once(':') else {
        return false;
    };
    h.len() == 2
        && m.len() == 2
        && h.parse::<u8>().is_ok_and(|h| h < 24)
        && m.parse::<u8>().is_ok_and(|m| m < 60)
}

fn clean_text(s: &str, max: usize) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::MediaRef;

    pub(crate) fn pdf(name: &str, pages: u32) -> Cue {
        Cue::new(
            name,
            CueContent::Pdf {
                file: MediaRef::linked(format!("/{name}.pdf")),
                page_count: pages,
                source: None,
            },
        )
    }

    /// Cue 0: PDF with 3 slides. Cue 1: PDF with 2 slides.
    pub(crate) fn two_cue_engine() -> Engine {
        let show = Show {
            title: "Test".into(),
            cues: vec![pdf("a", 3), pdf("b", 2)],
            ..Show::default()
        };
        Engine::new(show)
    }

    fn at(e: &Engine) -> Option<(usize, u32)> {
        e.program()
            .map(|p| (e.show().cue_index(&p.cue_id).unwrap(), p.slide))
    }

    #[test]
    fn next_walks_slides_then_cues_and_stops_at_end() {
        let mut e = two_cue_engine();
        let mut seen = vec![];
        for _ in 0..7 {
            e.apply(&Action::Next, 0).unwrap();
            seen.push(at(&e).unwrap());
        }
        assert_eq!(
            seen,
            vec![(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 1), (1, 1)]
        );
    }

    #[test]
    fn prev_walks_back_across_cues() {
        let mut e = two_cue_engine();
        let b = e.show().cues[1].id.clone();
        e.go_to(&b, 0, 0).unwrap();
        e.apply(&Action::Prev, 0).unwrap();
        assert_eq!(at(&e), Some((0, 2)));
        e.apply(&Action::Prev, 0).unwrap();
        e.apply(&Action::Prev, 0).unwrap();
        assert_eq!(at(&e), Some((0, 0)));
        assert_eq!(e.apply(&Action::Prev, 0).unwrap(), Change::NONE);
    }

    #[test]
    fn empty_cues_are_skipped() {
        let show = Show {
            cues: vec![
                Cue::new("empty", CueContent::ImageFolder { files: vec![] }),
                pdf("a", 1),
                Cue::new("empty2", CueContent::ImageFolder { files: vec![] }),
                pdf("b", 1),
            ],
            ..Show::default()
        };
        let mut e = Engine::new(show);
        e.apply(&Action::Next, 0).unwrap();
        assert_eq!(at(&e), Some((1, 0)));
        e.apply(&Action::Next, 0).unwrap();
        assert_eq!(at(&e), Some((3, 0)));
        e.apply(&Action::Prev, 0).unwrap();
        assert_eq!(at(&e), Some((1, 0)));
    }

    #[test]
    fn cue_jumps() {
        let mut e = two_cue_engine();
        e.apply(&Action::NextCue, 0).unwrap();
        assert_eq!(at(&e), Some((0, 0)));
        e.apply(&Action::Next, 0).unwrap();
        e.apply(&Action::NextCue, 0).unwrap();
        assert_eq!(at(&e), Some((1, 0)));
        e.apply(&Action::Next, 0).unwrap();
        e.apply(&Action::PrevCue, 0).unwrap();
        assert_eq!(
            at(&e),
            Some((1, 0)),
            "prev cue first rewinds to the cue start"
        );
        e.apply(&Action::PrevCue, 0).unwrap();
        assert_eq!(at(&e), Some((0, 0)));
    }

    #[test]
    fn go_to_validates() {
        let mut e = two_cue_engine();
        let a = e.show().cues[0].id.clone();
        assert_eq!(e.go_to(&a, 3, 0), Err(ErrorCode::NotFound));
        assert_eq!(e.go_to("nope", 0, 0), Err(ErrorCode::NotFound));
        assert_eq!(e.go_to(&a, 2, 0), Ok(Change::LIVE));
    }

    #[test]
    fn freeze_holds_output_while_program_moves() {
        let mut e = two_cue_engine();
        e.apply(&Action::Next, 0).unwrap();
        let frozen = e.program().cloned();
        e.apply(&Action::ToggleFreeze, 0).unwrap();
        e.apply(&Action::Next, 0).unwrap();
        e.apply(&Action::Next, 0).unwrap();
        assert_eq!(e.output().cloned(), frozen);
        assert_ne!(e.program().cloned(), frozen);
        assert!(e.masters().freeze);
        e.apply(&Action::ToggleFreeze, 0).unwrap();
        assert_eq!(e.output(), e.program());
    }

    #[test]
    fn masters_toggle_and_panic() {
        let mut e = two_cue_engine();
        e.apply(&Action::ToggleBlackout, 0).unwrap();
        assert!(e.masters().blackout);
        assert_eq!(
            e.apply(&Action::SetBlackout { on: true }, 0).unwrap(),
            Change::NONE
        );
        e.apply(&Action::SetFreeze { on: true }, 0).unwrap();
        e.apply(&Action::Panic, 0).unwrap();
        let m = e.masters();
        assert!(m.logo && !m.freeze && m.blackout);
    }

    #[test]
    fn navigation_under_blackout_keeps_blackout() {
        let mut e = two_cue_engine();
        e.apply(&Action::SetBlackout { on: true }, 0).unwrap();
        e.apply(&Action::Next, 0).unwrap();
        assert!(e.masters().blackout);
        assert!(e.program().is_some());
    }

    #[test]
    fn timers() {
        let mut e = two_cue_engine();
        assert!(!e.live_state(0).show_timer.is_running());
        e.apply(&Action::Next, 1_000).unwrap();
        let s = e.live_state(5_000);
        assert_eq!(s.show_timer.elapsed_ms(5_000), 4_000);
        e.apply(&Action::Next, 3_000).unwrap();
        assert_eq!(e.live_state(5_000).slide_timer.elapsed_ms(5_000), 2_000);
        e.apply(&Action::TimerPause, 6_000).unwrap();
        assert_eq!(e.live_state(60_000).show_timer.elapsed_ms(60_000), 5_000);
        e.apply(&Action::TimerStart, 10_000).unwrap();
        assert_eq!(e.live_state(11_000).show_timer.elapsed_ms(11_000), 6_000);
        e.apply(&Action::TimerReset, 12_000).unwrap();
        assert_eq!(e.live_state(13_000).show_timer.elapsed_ms(13_000), 1_000);
    }

    #[test]
    fn move_cue_keeps_program() {
        let mut e = two_cue_engine();
        e.apply(&Action::Next, 0).unwrap();
        let a = e.show().cues[0].id.clone();
        e.apply(
            &Action::MoveCue {
                cue_id: a.clone(),
                to_index: 99,
            },
            0,
        )
        .unwrap();
        assert_eq!(e.show().cues[1].id, a);
        assert_eq!(e.program().unwrap().cue_id, a);
        assert!(e.is_dirty());
    }

    #[test]
    fn removing_live_cue_falls_forward() {
        let mut e = two_cue_engine();
        e.apply(&Action::Next, 0).unwrap();
        let a = e.show().cues[0].id.clone();
        let b = e.show().cues[1].id.clone();
        e.apply(&Action::RemoveCue { cue_id: a }, 0).unwrap();
        assert_eq!(e.program().unwrap().cue_id, b);
        e.apply(&Action::RemoveCue { cue_id: b }, 0).unwrap();
        assert!(e.program().is_none());
    }

    #[test]
    fn removing_frozen_cue_clears_output() {
        let mut e = two_cue_engine();
        e.apply(&Action::Next, 0).unwrap();
        e.apply(&Action::SetFreeze { on: true }, 0).unwrap();
        let a = e.show().cues[0].id.clone();
        e.apply(&Action::RemoveCue { cue_id: a }, 0).unwrap();
        assert!(e.output().is_none());
        assert!(e.masters().freeze);
    }

    #[test]
    fn edits_validate_and_mark_dirty() {
        let mut e = two_cue_engine();
        let a = e.show().cues[0].id.clone();
        let rev = e.show_snapshot(false).revision;
        e.apply(
            &Action::RenameCue {
                cue_id: a.clone(),
                name: "  Hello\u{7}  ".into(),
            },
            0,
        )
        .unwrap();
        assert_eq!(e.show().cues[0].name, "Hello");
        assert!(e.show_snapshot(false).revision > rev);
        assert_eq!(
            e.apply(
                &Action::SetCueColor {
                    cue_id: a.clone(),
                    color: Some("red;x".into())
                },
                0
            ),
            Err(ErrorCode::InvalidState)
        );
        e.apply(
            &Action::SetCueColor {
                cue_id: a,
                color: Some("#ff0000".into()),
            },
            0,
        )
        .unwrap();
        e.apply(
            &Action::AddBlank {
                color: "#000".into(),
                at_index: Some(0),
            },
            0,
        )
        .unwrap();
        assert_eq!(e.show().cues.len(), 3);
        assert!(e.is_dirty());
        e.mark_saved("/x.msnack".into());
        assert!(!e.is_dirty());
    }

    #[test]
    fn host_actions_are_not_engine_actions() {
        let mut e = two_cue_engine();
        assert_eq!(e.apply(&Action::NewShow, 0), Err(ErrorCode::InvalidState));
    }

    #[test]
    fn replace_show_resets_position_but_keeps_blackout() {
        let mut e = two_cue_engine();
        e.apply(&Action::Next, 0).unwrap();
        e.apply(&Action::SetBlackout { on: true }, 0).unwrap();
        e.replace_show(Show::default(), None);
        assert!(e.program().is_none());
        assert!(e.masters().blackout);
        assert!(!e.is_dirty());
    }

    #[test]
    fn restore_position_ignores_invalid() {
        let mut e = two_cue_engine();
        let a = e.show().cues[0].id.clone();
        assert_eq!(
            e.restore_position(
                Some(Position {
                    cue_id: a.clone(),
                    slide: 9
                }),
                0
            ),
            Change::NONE
        );
        assert!(
            e.restore_position(
                Some(Position {
                    cue_id: a,
                    slide: 1
                }),
                0
            )
            .live
        );
    }

    #[test]
    fn live_state_reports_next_and_prev() {
        let mut e = two_cue_engine();
        let s = e.live_state(0);
        assert!(s.prev.is_none());
        assert_eq!(s.next.unwrap().slide, 0);
        e.apply(&Action::Next, 0).unwrap();
        e.apply(&Action::Next, 0).unwrap();
        let s = e.live_state(0);
        assert_eq!(s.prev.unwrap().slide, 0);
        assert_eq!(s.next.unwrap().slide, 2);
    }
}
