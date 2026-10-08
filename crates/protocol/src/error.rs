// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Stable error identifiers. User-facing text lives in the frontend locale files
/// (`error.<code>`), never in Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ErrorCode {
    ProtocolMismatch,
    MalformedMessage,
    Unauthorized,
    Forbidden,
    /// The action may only be sent from the host machine itself.
    LocalOnly,
    NotFound,
    /// The action is valid but cannot be applied in the current state.
    InvalidState,
    InvalidPin,
    InvalidJoinToken,
    /// Too many failed pairing attempts; try again later.
    PairingLocked,
    PairingDenied,
    UnsupportedFile,
    /// The PDF engine could not be loaded on the host.
    PdfEngineMissing,
    /// The file could not be read or written.
    Io,
    ShowFileInvalid,
    Internal,
}
