// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{CueId, Position, Role};

/// Everything any input source can ask the host to do. Every action passes through the single
/// dispatcher in `crates/core`, which checks it against the sender's role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "action", rename_all = "snake_case")]
#[ts(export)]
pub enum Action {
    // --- navigation ---
    /// Start the show, or advance like `next`.
    Go,
    Next,
    Prev,
    NextCue,
    PrevCue,
    GoTo {
        position: Position,
    },

    // --- master states ---
    SetBlackout {
        on: bool,
    },
    ToggleBlackout,
    SetFreeze {
        on: bool,
    },
    ToggleFreeze,
    SetLogo {
        on: bool,
    },
    ToggleLogo,
    /// Immediately show the logo screen and release freeze.
    Panic,

    // --- timers ---
    TimerStart,
    TimerPause,
    TimerReset,

    // --- show editing ---
    RenameShow {
        title: String,
    },
    RenameCue {
        cue_id: CueId,
        name: String,
    },
    SetCueNotes {
        cue_id: CueId,
        notes: String,
    },
    SetCueColor {
        cue_id: CueId,
        color: Option<String>,
    },
    MoveCue {
        cue_id: CueId,
        to_index: u32,
    },
    RemoveCue {
        cue_id: CueId,
    },
    AddBlank {
        color: String,
        at_index: Option<u32>,
    },

    // --- host file operations (local only: paths refer to the host's file system) ---
    AddFiles {
        paths: Vec<String>,
        at_index: Option<u32>,
    },
    NewShow,
    OpenShow {
        path: String,
    },
    SaveShow {
        path: Option<String>,
        embed_media: bool,
    },

    // --- devices ---
    ApprovePairing {
        request_id: String,
        role: Role,
    },
    DenyPairing {
        request_id: String,
    },
    SetDeviceRole {
        device_id: String,
        role: Role,
    },
    RevokeDevice {
        device_id: String,
    },
    DisconnectAll,
    SetAutoApprove {
        role: Option<Role>,
    },
}
