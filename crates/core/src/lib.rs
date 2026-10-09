// SPDX-License-Identifier: GPL-3.0-or-later
//! Show model, cue engine and the single action dispatcher every input source goes through.
//!
//! This crate contains no UI, networking or platform code so it can be exhaustively
//! unit-tested. Time is always passed in explicitly (`now_ms`, Unix epoch milliseconds).

pub mod bundle;
pub mod dispatcher;
pub mod engine;
#[cfg(test)]
mod live_tests;
pub mod model;
pub mod permissions;

pub use dispatcher::{dispatch, Dispatched, Origin};
pub use engine::{Change, Engine};
pub use midnightsnack_protocol as protocol;
pub use model::{Cue, CueContent, MediaRef, Show};

/// Application version, shared by the host and the embedded server.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Current Unix time in milliseconds.
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::APP_VERSION.is_empty());
    }
}
