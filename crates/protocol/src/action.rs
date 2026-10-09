// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    CaptureSource, CueId, MediaOptions, OutputDef, Overlay, Position, Role, TestPattern, TextTheme,
    TimerCue, Transition, WebInfo,
};

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

    // --- media playback (cue on the output) ---
    MediaPlay,
    MediaPause,
    MediaSeek {
        position_ms: u32,
    },
    MediaRestart,

    // --- global countdown and stage ---
    CountdownSet {
        duration_ms: u32,
        label: String,
    },
    CountdownStart,
    CountdownPause,
    CountdownReset,
    SetStageMessage {
        text: Option<String>,
    },

    // --- outputs ---
    /// Shows a test pattern on every output, or hides it.
    SetTestPattern {
        pattern: Option<TestPattern>,
    },
    /// Adds or replaces (by id) an output definition.
    PutOutput {
        output: OutputDef,
    },
    RemoveOutput {
        output_id: String,
    },
    SetCueTargets {
        cue_id: CueId,
        targets: Option<Vec<String>>,
    },

    // --- overlays ---
    SetOverlayVisible {
        overlay_id: String,
        visible: bool,
    },
    ToggleOverlay {
        overlay_id: String,
    },
    /// Adds or replaces (by id) an overlay definition.
    PutOverlay {
        overlay: Overlay,
    },
    RemoveOverlay {
        overlay_id: String,
    },

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
    AddText {
        name: String,
        text: String,
        lyrics: bool,
        at_index: Option<u32>,
    },
    SetCueText {
        cue_id: CueId,
        text: String,
        lyrics: bool,
    },
    AddTimer {
        name: String,
        timer: TimerCue,
        at_index: Option<u32>,
    },
    SetCueTimer {
        cue_id: CueId,
        timer: TimerCue,
    },
    /// `theme: None` returns to the show default.
    SetCueTheme {
        cue_id: CueId,
        theme: Option<TextTheme>,
    },
    SetDefaultTheme {
        theme: TextTheme,
    },
    SetCueTransition {
        cue_id: CueId,
        transition: Option<Transition>,
    },
    SetDefaultTransition {
        transition: Transition,
    },
    SetCueAutoAdvance {
        cue_id: CueId,
        after_ms: Option<u32>,
    },
    SetMediaOptions {
        cue_id: CueId,
        options: MediaOptions,
    },
    AddWeb {
        name: String,
        web: WebInfo,
        at_index: Option<u32>,
    },
    SetWebOptions {
        cue_id: CueId,
        web: WebInfo,
    },
    AddCapture {
        name: String,
        source: CaptureSource,
        at_index: Option<u32>,
    },
    SetCapture {
        cue_id: CueId,
        source: CaptureSource,
        fps: u32,
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
    /// Image for the logo screen; `None` restores the built-in logo.
    SetLogoImage {
        path: Option<String>,
    },
    /// Background image of a cue's theme, or of the default theme when `cue_id` is `None`.
    SetBackgroundImage {
        cue_id: Option<CueId>,
        path: Option<String>,
    },
    SetOverlayImage {
        overlay_id: String,
        path: Option<String>,
    },

    // --- reports from host output windows (local only) ---
    MediaLoaded {
        cue_id: CueId,
        duration_ms: u32,
    },
    MediaEnded {
        cue_id: CueId,
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
