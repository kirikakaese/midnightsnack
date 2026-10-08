// SPDX-License-Identifier: GPL-3.0-or-later
//! Show model, cue engine and the single action dispatcher every input source goes through.
//!
//! This crate contains no UI, networking or platform code so it can be exhaustively
//! unit-tested.

pub use midnightsnack_protocol as protocol;

/// Application version, shared by the host and the embedded server.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::APP_VERSION.is_empty());
    }
}
