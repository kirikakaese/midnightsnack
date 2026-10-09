// SPDX-License-Identifier: GPL-3.0-or-later
//! Fires auto-advances (slide delays and media end) at the time the engine asks for.

use std::sync::Arc;
use std::time::Duration;

use midnightsnack_core::now_ms;

use crate::state::{lock, AppState};

/// Re-check at least this often, in case the host clock jumps.
const MAX_SLEEP: Duration = Duration::from_secs(5);

pub async fn run(state: Arc<AppState>) {
    loop {
        let due = lock(&state.engine).auto_advance_at();
        let sleep = match due {
            Some(at) => Duration::from_millis((at - now_ms()).max(0) as u64).min(MAX_SLEEP),
            None => MAX_SLEEP,
        };
        tokio::select! {
            _ = tokio::time::sleep(sleep) => {
                let change = lock(&state.engine).tick(now_ms());
                state.after_change(change);
            }
            // Something changed: recompute the deadline.
            _ = state.schedule.notified() => {}
        }
    }
}
