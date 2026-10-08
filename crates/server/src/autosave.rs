// SPDX-License-Identifier: GPL-3.0-or-later
//! Crash recovery: the working show and position are written shortly after every change and
//! restored on the next launch.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use midnightsnack_core::{now_ms, Engine, Show};
use midnightsnack_protocol::Position;
use serde::{Deserialize, Serialize};

use crate::state::{lock, AppState};
use crate::util::{read_json, write_json_atomic};

const DEBOUNCE: Duration = Duration::from_millis(1500);
const FILE: &str = "autosave.json";

#[derive(Debug, Serialize, Deserialize)]
struct Autosave {
    show: Show,
    media_dir: Option<PathBuf>,
    path: Option<PathBuf>,
    dirty: bool,
    program: Option<Position>,
}

/// Builds an engine from the last autosave, if any.
pub fn restore(data_dir: &std::path::Path) -> Option<Engine> {
    let saved: Autosave = read_json(&data_dir.join(FILE))?;
    let mut show = saved.show;
    show.media_dir = saved.media_dir;
    let mut engine = Engine::default();
    engine.replace_show(show, saved.path);
    if saved.dirty {
        engine.mark_dirty();
    }
    engine.restore_position(saved.program, now_ms());
    tracing::info!("restored show from autosave");
    Some(engine)
}

pub async fn run(state: Arc<AppState>) {
    let Some(dir) = state.data_dir.clone() else {
        return;
    };
    loop {
        state.autosave.notified().await;
        tokio::time::sleep(DEBOUNCE).await;
        let snapshot = {
            let e = lock(&state.engine);
            Autosave {
                show: e.show().clone(),
                media_dir: e.show().media_dir.clone(),
                path: e.path().cloned(),
                dirty: e.is_dirty(),
                program: e.program().cloned(),
            }
        };
        let path = dir.join(FILE);
        let result = tokio::task::spawn_blocking(move || write_json_atomic(&path, &snapshot)).await;
        if !matches!(result, Ok(Ok(()))) {
            tracing::error!("autosave failed");
        }
    }
}
