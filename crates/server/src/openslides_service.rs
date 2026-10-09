// SPDX-License-Identifier: GPL-3.0-or-later
//! Runs the OpenSlides adapter with the host's settings, shares the meeting data with every
//! client and keeps the page counts of OpenSlides cues in line with the data.

use std::path::Path;
use std::sync::Arc;

use midnightsnack_core::now_ms;
use midnightsnack_openslides::{Config, Update};
use midnightsnack_protocol::{OpenSlidesError, OpenSlidesState, OpenSlidesStatus, OsMeetingRef};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::state::{lock, AppState, Event};

/// Connection settings (the password is kept separately, see [`Credentials`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenSlidesSettings {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub meeting_id: Option<u32>,
}

/// Live state of the connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSlidesRuntime {
    pub state: OpenSlidesState,
    pub error: Option<OpenSlidesError>,
    pub meetings: Vec<OsMeetingRef>,
}

impl Default for OpenSlidesRuntime {
    fn default() -> Self {
        OpenSlidesRuntime {
            state: OpenSlidesState::Off,
            error: None,
            meetings: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Credentials {
    password: String,
}

const CREDENTIALS_FILE: &str = "openslides-credentials.json";

/// The stored password (readable only by this user on disk).
pub fn load_password(data_dir: Option<&Path>) -> String {
    data_dir
        .and_then(|d| crate::util::read_json::<Credentials>(&d.join(CREDENTIALS_FILE)))
        .map(|c| c.password)
        .unwrap_or_default()
}

pub fn save_password(state: &AppState, password: &str) {
    *lock(&state.openslides_password) = password.to_owned();
    if let Some(d) = &state.data_dir {
        let path = d.join(CREDENTIALS_FILE);
        let result = if password.is_empty() {
            std::fs::remove_file(&path).or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(e)
                }
            })
        } else {
            crate::util::write_secret_json(
                &path,
                &Credentials {
                    password: password.to_owned(),
                },
            )
        };
        if let Err(e) = result {
            tracing::error!(error = %e, "failed to save OpenSlides credentials");
        }
    }
}

impl AppState {
    pub fn openslides_status(&self) -> OpenSlidesStatus {
        let settings = lock(&self.settings).openslides.clone();
        let runtime = lock(&self.openslides_runtime).clone();
        OpenSlidesStatus {
            enabled: settings.enabled,
            url: settings.url,
            username: settings.username,
            has_password: !lock(&self.openslides_password).is_empty(),
            meeting_id: settings.meeting_id,
            state: runtime.state,
            error: runtime.error,
            meetings: runtime.meetings,
        }
    }

    /// Brings the page counts of OpenSlides cues in line with the current data.
    pub fn update_openslides_pages(&self) {
        let data = lock(&self.openslides_data).clone();
        let change = lock(&self.engine).set_openslides_pages(now_ms(), |slide| {
            midnightsnack_openslides::pages(data.as_deref(), slide)
        });
        if change.show {
            self.emit(Event::Show);
        }
        if change.live {
            self.emit(Event::Live);
        }
    }
}

fn set_runtime(state: &AppState, f: impl FnOnce(&mut OpenSlidesRuntime)) {
    let changed = {
        let mut r = lock(&state.openslides_runtime);
        let before = r.clone();
        f(&mut r);
        *r != before
    };
    if changed {
        state.emit(Event::OpenSlidesStatus);
    }
}

/// Runs for the lifetime of the server, restarting the adapter when the settings change.
pub async fn run(state: Arc<AppState>) {
    tokio::spawn(follow_show(state.clone()));
    loop {
        let settings = lock(&state.settings).openslides.clone();
        if !settings.enabled || settings.url.trim().is_empty() {
            set_runtime(&state, |r| *r = OpenSlidesRuntime::default());
            set_data(&state, None);
            state.openslides_restart.notified().await;
            continue;
        }
        let config = Config {
            url: settings.url.clone(),
            username: settings.username.clone(),
            password: lock(&state.openslides_password).clone(),
        };
        let (tx, mut rx) = mpsc::channel(64);
        let adapter = tokio::spawn(midnightsnack_openslides::run(
            config,
            settings.meeting_id,
            tx,
        ));
        loop {
            tokio::select! {
                update = rx.recv() => match update {
                    Some(u) => apply(&state, u),
                    None => break,
                },
                _ = state.openslides_restart.notified() => break,
            }
        }
        adapter.abort();
    }
}

fn set_data(state: &AppState, data: Option<midnightsnack_protocol::OsMeetingData>) {
    let changed = {
        let mut cur = lock(&state.openslides_data);
        if cur.as_deref() == data.as_ref() {
            false
        } else {
            *cur = data.map(Arc::new);
            true
        }
    };
    if changed {
        state.emit(Event::OpenSlides);
        state.update_openslides_pages();
    }
}

fn apply(state: &AppState, update: Update) {
    match update {
        Update::State(s, error) => set_runtime(state, |r| {
            r.state = s;
            r.error = error;
        }),
        Update::Meetings(meetings) => set_runtime(state, |r| r.meetings = meetings),
        Update::Data(data) => set_data(state, data),
    }
}

/// New or loaded OpenSlides cues get their page counts from the data already there.
async fn follow_show(state: Arc<AppState>) {
    let mut events = state.events.subscribe();
    loop {
        match events.recv().await {
            Ok(Event::Show) | Err(RecvError::Lagged(_)) => state.update_openslides_pages(),
            Ok(_) => {}
            Err(RecvError::Closed) => return,
        }
    }
}
