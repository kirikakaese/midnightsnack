// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared server state and the action entry point used by every transport.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use midnightsnack_capture::CaptureHub;
use midnightsnack_core::{dispatch, now_ms, Change, Dispatched, Engine, Origin, Show};
use midnightsnack_protocol::{
    Action, ErrorCode, HostInfo, PairingInfo, Position, Role, ShowSnapshot,
};
use midnightsnack_render::{RenderService, SlideSource, TargetSize};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Notify};

use crate::devices::DeviceStore;
use crate::pairing::Pairing;
use crate::util::{read_json, write_json_atomic};

/// Thumbnail size used by operator previews and remotes.
pub const THUMB_SIZE: TargetSize = TargetSize {
    width: 640,
    height: 360,
};
/// Output size assumed until an output window reports its viewport.
pub const DEFAULT_OUTPUT_SIZE: TargetSize = TargetSize {
    width: 1920,
    height: 1080,
};
/// Slides pre-rendered beyond the next one.
const PREFETCH_AHEAD: usize = 3;

/// Something changed; connections decide what to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Show,
    Live,
    Devices,
    Pairing,
    RenderProgress(u32),
    /// Disconnect a device (`Some`) or every remote device (`None`).
    Kick(Option<String>),
    /// A device's role changed.
    Session(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub auto_approve: Option<Role>,
}

pub struct AppState {
    pub host: HostInfo,
    pub engine: Mutex<Engine>,
    pub devices: Mutex<DeviceStore>,
    pub pairing: Mutex<Pairing>,
    pub settings: Mutex<Settings>,
    pub render: RenderService,
    pub capture: CaptureHub,
    pub events: broadcast::Sender<Event>,
    /// media key -> device id
    pub media_keys: Mutex<HashMap<String, String>>,
    pub output_size: Mutex<TargetSize>,
    /// `http://<ip>:<port>` for each usable interface.
    pub base_urls: Mutex<Vec<String>>,
    pub data_dir: Option<PathBuf>,
    pub autosave: Notify,
    /// Wakes the auto-advance scheduler after any change.
    pub schedule: Notify,
}

/// Locks a mutex, recovering from poisoning (a panicking connection task must not take the
/// whole show down).
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    pub fn new(
        host: HostInfo,
        render: RenderService,
        data_dir: Option<PathBuf>,
        engine: Engine,
    ) -> Arc<Self> {
        let settings: Settings = data_dir
            .as_ref()
            .and_then(|d| read_json(&d.join("settings.json")))
            .unwrap_or_default();
        let devices = DeviceStore::load(data_dir.as_ref().map(|d| d.join("devices.json")));
        let (events, _) = broadcast::channel(256);
        Arc::new(AppState {
            host,
            engine: Mutex::new(engine),
            devices: Mutex::new(devices),
            pairing: Mutex::new(Pairing::new(settings.auto_approve)),
            settings: Mutex::new(settings),
            render,
            capture: CaptureHub::new(),
            events,
            media_keys: Mutex::new(HashMap::new()),
            output_size: Mutex::new(DEFAULT_OUTPUT_SIZE),
            base_urls: Mutex::new(Vec::new()),
            data_dir,
            autosave: Notify::new(),
            schedule: Notify::new(),
        })
    }

    pub fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }

    pub fn save_settings(&self) {
        if let Some(d) = &self.data_dir {
            let s = lock(&self.settings).clone();
            if let Err(e) = write_json_atomic(&d.join("settings.json"), &s) {
                tracing::error!(error = %e, "failed to save settings");
            }
        }
    }

    pub fn pairing_info(&self) -> PairingInfo {
        let p = lock(&self.pairing);
        let token = p.join_token().to_owned();
        PairingInfo {
            pin: p.pin().to_owned(),
            join_urls: lock(&self.base_urls)
                .iter()
                .map(|b| format!("{b}/join#t={token}"))
                .collect(),
            auto_approve: p.auto_approve,
        }
    }

    pub fn show_snapshot(&self, role: Role) -> ShowSnapshot {
        lock(&self.engine).show_snapshot(role == Role::Admin)
    }

    /// The single entry point for actions from any transport.
    pub async fn perform(
        self: &Arc<Self>,
        origin: Origin,
        action: Action,
    ) -> Result<(), ErrorCode> {
        let result = {
            let mut engine = lock(&self.engine);
            dispatch(&mut engine, origin, action, now_ms())
        };
        match result? {
            Dispatched::Applied(change) => {
                self.after_change(change);
                Ok(())
            }
            Dispatched::Host(action) => crate::host_actions::run(self, *action).await,
        }
    }

    /// Broadcasts a change, schedules prefetching and autosave.
    pub fn after_change(&self, change: Change) {
        if change.show {
            self.emit(Event::Show);
        }
        if change.live {
            self.emit(Event::Live);
        }
        if change.any() {
            self.prefetch();
            self.autosave.notify_one();
            self.schedule.notify_one();
        }
    }

    /// Resolves a slide to something the renderer understands. `None` for blank cues or
    /// invalid positions.
    pub fn slide_source(&self, cue_id: &str, slide: u32) -> Option<SlideSource> {
        let engine = lock(&self.engine);
        slide_source(engine.show(), cue_id, slide)
    }

    /// Pre-renders what is likely to be shown next at output and thumbnail size.
    pub fn prefetch(&self) {
        let output = *lock(&self.output_size);
        let sources: Vec<SlideSource> = {
            let engine = lock(&self.engine);
            let show = engine.show();
            let live = engine.live_state(0);
            let mut positions: Vec<Position> =
                [live.output, live.program, live.next.clone(), live.prev]
                    .into_iter()
                    .flatten()
                    .collect();
            if let Some(next) = live.next {
                positions.extend(following(show, &next, PREFETCH_AHEAD - 1));
            }
            positions
                .iter()
                .filter_map(|p| slide_source(show, &p.cue_id, p.slide))
                .collect()
        };
        for s in sources {
            self.render.prefetch(s.clone(), output);
            self.render.prefetch(s, THUMB_SIZE);
        }
    }
}

/// Where converted office documents are stored.
pub fn converted_dir(state: &AppState) -> std::path::PathBuf {
    state.render.cache().root().join("converted")
}

pub fn slide_source(show: &Show, cue_id: &str, slide: u32) -> Option<SlideSource> {
    use midnightsnack_core::CueContent::*;
    let cue = show.cue(cue_id)?;
    if slide >= cue.slide_count() {
        return None;
    }
    match &cue.content {
        Pdf { file, .. } => Some(SlideSource::PdfPage {
            path: show.resolve(file)?,
            page: slide,
        }),
        Image { file } => Some(SlideSource::Image {
            path: show.resolve(file)?,
        }),
        ImageFolder { files } => Some(SlideSource::Image {
            path: show.resolve(files.get(slide as usize)?)?,
        }),
        // Rendered by the clients themselves (video element, text layout).
        Blank { .. } | Media { .. } | Text { .. } | Timer { .. } | Web { .. } | Capture { .. } => {
            None
        }
    }
}

/// Up to `n` positions after `from`, crossing cue boundaries.
fn following(show: &Show, from: &Position, n: usize) -> Vec<Position> {
    let mut out = Vec::new();
    let Some(start) = show.cue_index(&from.cue_id) else {
        return out;
    };
    let mut slide = from.slide + 1;
    for cue in &show.cues[start..] {
        while slide < cue.slide_count() {
            if out.len() == n {
                return out;
            }
            out.push(Position {
                cue_id: cue.id.clone(),
                slide,
            });
            slide += 1;
        }
        slide = 0;
    }
    out
}
