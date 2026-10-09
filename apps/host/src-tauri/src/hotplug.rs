// SPDX-License-Identifier: GPL-3.0-or-later
//! Display hotplug: when an output's display disappears the output window is hidden (it must
//! never jump onto the operator's screen) and the operator is warned; when the display comes
//! back the window returns to it.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::output::{self, DisplayInfo};
use crate::settings::Placement;
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
    let missing = missing_outputs(&settings.outputs, &names);
    DisplayStatus { displays, missing }
}

/// Open full-screen outputs whose remembered display is not connected.
fn missing_outputs(outputs: &BTreeMap<String, Placement>, names: &BTreeSet<&str>) -> Vec<String> {
    outputs
        .iter()
        .filter(|(_, p)| p.open && !p.windowed)
        .filter(|(_, p)| p.display.as_deref().is_some_and(|d| !names.contains(d)))
        .map(|(id, _)| id.clone())
        .collect()
}

/// Outputs that just lost their display, and outputs whose display just came back.
fn changes(last: &[String], now: &[String]) -> (Vec<String>, Vec<String>) {
    let lost = now
        .iter()
        .filter(|id| !last.contains(id))
        .cloned()
        .collect();
    let back = last
        .iter()
        .filter(|id| !now.contains(id))
        .cloned()
        .collect();
    (lost, back)
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
                let (lost, back) = changes(&last.missing, &now.missing);
                for id in &lost {
                    tracing::warn!(output = %id, "display of output disconnected; hiding it");
                    if let Some(w) = output::window(&app, id) {
                        let _ = w.hide();
                    }
                }
                let state = app.state::<HostState>();
                let placements = state.settings.lock().unwrap_or_else(|e| e.into_inner()).outputs.clone();
                for id in &back {
                    if let Some(screen) = placements.get(id).and_then(|p| p.display.clone()) {
                        tracing::info!(output = %id, display = %screen, "display reconnected; restoring output");
                        output::restore(&app, id, &screen);
                    }
                }
                let _ = app.emit("displays-changed", &now);
                last = now;
            }
        })
        .expect("spawn display watcher");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(display: &str, windowed: bool, open: bool) -> Placement {
        Placement {
            display: Some(display.into()),
            windowed,
            open,
        }
    }

    #[test]
    fn only_open_fullscreen_outputs_go_missing() {
        let outputs = BTreeMap::from([
            ("main".to_string(), placement("HDMI-1", false, true)),
            ("side".to_string(), placement("DP-2", false, true)),
            ("rehearsal".to_string(), placement("DP-2", true, true)),
            ("closed".to_string(), placement("DP-2", false, false)),
        ]);
        let names = BTreeSet::from(["eDP-1", "HDMI-1"]);
        assert_eq!(missing_outputs(&outputs, &names), vec!["side".to_string()]);
    }

    #[test]
    fn reports_lost_and_returned_displays() {
        let last = vec!["a".to_string(), "b".to_string()];
        let now = vec!["b".to_string(), "c".to_string()];
        assert_eq!(changes(&last, &now), (vec!["c".into()], vec!["a".into()]));
    }
}
