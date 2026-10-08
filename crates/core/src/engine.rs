// SPDX-License-Identifier: GPL-3.0-or-later
//! The cue engine: owns the show and the live state and applies actions to them.

use std::path::PathBuf;

use crate::model::{is_valid_color, Cue, CueContent, Show};
use crate::protocol::{Action, ErrorCode, LiveState, Masters, Position, ShowSnapshot, Stopwatch};

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
    /// `Some` while frozen: what the output keeps showing.
    frozen_output: Option<Option<Position>>,
    blackout: bool,
    logo: bool,
    show_timer: Stopwatch,
    slide_timer: Stopwatch,
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

    /// What the audience currently sees (ignoring blackout/logo).
    pub fn output(&self) -> Option<&Position> {
        match &self.live.frozen_output {
            Some(frozen) => frozen.as_ref(),
            None => self.live.program.as_ref(),
        }
    }

    pub fn masters(&self) -> Masters {
        Masters {
            blackout: self.live.blackout,
            freeze: self.live.frozen_output.is_some(),
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
        let change = self.apply_inner(action, now_ms)?;
        Ok(self.bump(change))
    }

    fn apply_inner(&mut self, action: &Action, now_ms: i64) -> Result<Change, ErrorCode> {
        use Action::*;
        Ok(match action {
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
            ToggleFreeze => self.set_freeze(self.live.frozen_output.is_none()),
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
            | NewShow
            | OpenShow { .. }
            | SaveShow { .. }
            | ApprovePairing { .. }
            | DenyPairing { .. }
            | SetDeviceRole { .. }
            | RevokeDevice { .. }
            | DisconnectAll
            | SetAutoApprove { .. } => return Err(ErrorCode::InvalidState),
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
        match (on, self.live.frozen_output.is_some()) {
            (true, false) => {
                self.live.frozen_output = Some(self.live.program.clone());
                Change::LIVE
            }
            (false, true) => {
                self.live.frozen_output = None;
                Change::LIVE
            }
            _ => Change::NONE,
        }
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
        if let Some(Some(frozen)) = &self.live.frozen_output {
            if frozen.cue_id == cue_id {
                self.live.frozen_output = Some(None);
                change.live = true;
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

    /// Replaces the whole show (new / open). Master states are kept so a blackout survives
    /// loading a show; position and timers reset.
    pub fn replace_show(&mut self, show: Show, path: Option<PathBuf>) -> Change {
        self.show = show;
        self.path = path;
        self.live.program = None;
        if self.live.frozen_output.is_some() {
            self.live.frozen_output = Some(None);
        }
        self.live.show_timer = Stopwatch::default();
        self.live.slide_timer = Stopwatch::default();
        let c = self.bump(Change::BOTH);
        self.dirty = false;
        c
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
                let c = self.set_program(Some(p), now_ms);
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
