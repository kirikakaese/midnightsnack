// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Identifier of a cue (UUID string).
pub type CueId = String;

/// Permission level of a connected device. Ordered from least to most privileged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Role {
    StageViewer,
    Presenter,
    Operator,
    Admin,
}

/// A slide inside a cue. `slide` is zero-based.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Position {
    pub cue_id: CueId,
    pub slide: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CueKind {
    Pdf,
    Image,
    ImageFolder,
    Blank,
}

/// What clients need to know about a cue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CueSummary {
    pub id: CueId,
    pub name: String,
    pub kind: CueKind,
    pub slide_count: u32,
    /// CSS color of the cue's color tag, e.g. `#ef4444`.
    pub color: Option<String>,
    /// Cue-level notes.
    pub notes: String,
    /// Speaker notes per slide (may be shorter than `slide_count`).
    pub slide_notes: Vec<String>,
    /// Background color for blank cues.
    pub background: Option<String>,
}

/// The show structure. Sent whenever the cue list changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ShowSnapshot {
    pub id: String,
    pub title: String,
    pub cues: Vec<CueSummary>,
    /// Path of the show file on the host, if saved. Only sent to admins.
    pub path: Option<String>,
    /// Unsaved changes exist.
    pub dirty: bool,
    pub revision: u64,
}

/// Master states, applied on top of the program content.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Masters {
    pub blackout: bool,
    pub freeze: bool,
    pub logo: bool,
}

/// A stopwatch. Elapsed time is `accumulated_ms + (now - running_since_ms)` while running.
/// Timestamps are Unix epoch milliseconds of the host clock; `LiveState::host_time_ms` lets
/// clients compensate for clock offset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Stopwatch {
    #[ts(type = "number")]
    pub accumulated_ms: i64,
    #[ts(type = "number | null")]
    pub running_since_ms: Option<i64>,
}

impl Stopwatch {
    pub fn elapsed_ms(&self, now_ms: i64) -> i64 {
        self.accumulated_ms + self.running_since_ms.map_or(0, |s| (now_ms - s).max(0))
    }
    pub fn is_running(&self) -> bool {
        self.running_since_ms.is_some()
    }
}

/// Live show state. Sent on every change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LiveState {
    /// Where the operator is.
    pub program: Option<Position>,
    /// What the audience sees (differs from `program` while frozen).
    pub output: Option<Position>,
    /// What `next` would go to.
    pub next: Option<Position>,
    /// What `prev` would go to.
    pub prev: Option<Position>,
    pub masters: Masters,
    /// Show running time.
    pub show_timer: Stopwatch,
    /// Time on the current slide.
    pub slide_timer: Stopwatch,
    /// Host clock at the time this state was sent.
    #[ts(type = "number")]
    pub host_time_ms: i64,
    pub revision: u64,
}
