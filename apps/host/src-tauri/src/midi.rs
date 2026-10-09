// SPDX-License-Identifier: GPL-3.0-or-later
//! MIDI controllers: presses on any connected input run their bound action with the operator
//! role. In learn mode the next press is reported to the operator UI instead.

use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use midnightsnack_control::midi::{self, MidiListener};
use midnightsnack_core::Origin;
use midnightsnack_protocol::{MidiTrigger, Role};
use midnightsnack_server::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::HostState;

/// MIDI settings of this host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MidiSettings {
    pub enabled: bool,
    pub bindings: Vec<midnightsnack_protocol::MidiBinding>,
}

impl Default for MidiSettings {
    fn default() -> Self {
        MidiSettings {
            enabled: true,
            bindings: Vec::new(),
        }
    }
}

/// A pending learn request.
#[derive(Default)]
pub struct Learn(Mutex<Option<tokio::sync::oneshot::Sender<MidiTrigger>>>);

impl Learn {
    /// Waits for the next press (up to `timeout`).
    pub async fn next(&self, timeout: Duration) -> Option<MidiTrigger> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
        tokio::time::timeout(timeout, rx).await.ok()?.ok()
    }

    fn take(&self) -> Option<tokio::sync::oneshot::Sender<MidiTrigger>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

pub fn spawn(app: AppHandle, server: Arc<AppState>) {
    let (tx, rx) = mpsc::channel();
    MidiListener::spawn(tx);
    std::thread::Builder::new()
        .name("midnightsnack-midi-actions".into())
        .spawn(move || {
            for press in rx {
                // Activity indicator in the operator UI.
                let _ = app.emit("midi-press", press.trigger);
                if let Some(learner) = app.state::<Learn>().take() {
                    let _ = learner.send(press.trigger);
                    continue;
                }
                let action = {
                    let state = app.state::<HostState>();
                    let s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
                    if !s.midi.enabled {
                        continue;
                    }
                    midi::action_for(&s.midi.bindings, &press.trigger).cloned()
                };
                let Some(action) = action else { continue };
                let origin = Origin {
                    role: Role::Operator,
                    local: true,
                };
                let server = server.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = server.perform(origin, action).await {
                        tracing::debug!(?e, "MIDI action refused");
                    }
                });
            }
        })
        .expect("spawn MIDI action thread");
}
