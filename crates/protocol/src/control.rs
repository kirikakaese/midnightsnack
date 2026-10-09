// SPDX-License-Identifier: GPL-3.0-or-later
//! Control surfaces: MIDI bindings, OSC and API settings.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Action;

/// Default UDP port of the OSC server.
pub const DEFAULT_OSC_PORT: u16 = 4748;

/// What kind of MIDI message triggers a binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MidiKind {
    /// Note on (velocity > 0).
    Note,
    /// Control change crossing the middle (value ≥ 64), like a button press.
    ControlChange,
}

/// A MIDI note or controller on a channel (0–15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MidiTrigger {
    pub kind: MidiKind,
    pub channel: u8,
    pub number: u8,
}

/// A MIDI trigger mapped to an action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MidiBinding {
    pub trigger: MidiTrigger,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OscSettings {
    pub enabled: bool,
    pub port: u16,
}

impl Default for OscSettings {
    fn default() -> Self {
        OscSettings {
            enabled: false,
            port: DEFAULT_OSC_PORT,
        }
    }
}

/// Settings of the host's control interfaces (admins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ControlSettings {
    /// API keys (HTTP, WebSocket) and OSC only work from this computer.
    pub api_local_only: bool,
    pub osc: OscSettings,
    /// Port the OSC server actually listens on, if running.
    pub osc_listening: Option<u16>,
}
