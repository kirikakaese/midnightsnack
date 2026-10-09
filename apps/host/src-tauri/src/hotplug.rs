// SPDX-License-Identifier: GPL-3.0-or-later
//! Display hotplug: when an output's display disappears the output window is hidden (it must
//! never jump onto the operator's screen) and the operator is warned; when the display comes
//! back the window returns to it.

use std::collections::BTreeSet;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::output::{self, DisplayInfo};
use crate::HostState;

const POLL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DisplayStatus {
    pub displays: Vec<DisplayInfo>,
    /// Output ids whose display is not connected.
    pub missing: Vec<String>,
}

pub fn status(app: &AppHandle) -> DisplayStatus {
    let displays = output::list_displays(app);
    let names: BTreeSet<&str> = displays.iter().map(|d| d.name.as_str()).collect();
    let state = app.state::<HostState>();
    let settings = state.settings.lock().unwrap_or_else(|e| e.into_inner());
    let missing = settings
        .outputs
        .iter()
        .filter(|(_, p)| p.open && !p.windowed)
        .filter(|(_, p)| p.display.as_deref().is_some_and(|d| !names.contains(d)))
        .map(|(id, _)| id.clone())
        .collect();
    DisplayStatus { displays, missing }
}

pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("midnightsnack-displays".into())
        .spawn(move || {
            let mut last = status(&app);
            loop {
                std::thread::sleep(POLL);
                let now = status(&app);
                if now == last {
                    continue;
                }
                for id in &now.missing {
                    if !last.missing.contains(id) {
                        tracing::warn!(output = %id, "display of output disconnected; hiding it");
                        if let Some(w) = output::window(&app, id) {
                            let _ = w.hide();
                        }
                    }
                }
                let state = app.state::<HostState>();
                let placements = state.settings.lock().unwrap_or_else(|e| e.into_inner()).outputs.clone();
                for id in &last.missing {
                    if !now.missing.contains(id) {
                        if let Some(screen) = placements.get(id).and_then(|p| p.display.clone()) {
                            tracing::info!(output = %id, display = %screen, "display reconnected; restoring output");
                            output::restore(&app, id, &screen);
                        }
                    }
                }
                let _ = app.emit("displays-changed", &now);
                last = now;
            }
        })
        .expect("spawn display watcher");
}
