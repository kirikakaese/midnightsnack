// SPDX-License-Identifier: GPL-3.0-or-later
//! Updates from the GitHub releases, verified with DECK's release key before they are installed
//! (minisign; the key is compiled into release builds). Two feeds on the latest stable release:
//! `latest.json` (stable) and `beta.json` (the newest release, betas and release candidates
//! included). Builds without the key (local builds) never check.
//!
//! Hosts run live events, so nothing restarts on its own: an update is either installed on
//! request (only while no output is open) or, with automatic installation, downloaded in the
//! background and installed when the app quits.

use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::HostState;

/// Public key of DECK's release signing key, set by the release workflow.
pub const PUBKEY: Option<&str> = option_env!("DECK_UPDATER_PUBKEY");
/// Base URL of the update feeds, set by the release workflow.
const FEED: Option<&str> = option_env!("DECK_UPDATE_FEED");
const DEFAULT_FEED: &str = "https://github.com/kirikakaese/midnightsnack/releases/latest/download";
/// How often the background task looks whether a check is due.
const TICK: Duration = Duration::from_secs(30 * 60);

pub fn pubkey() -> Option<&'static str> {
    PUBKEY.map(str::trim).filter(|k| !k.is_empty())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Interval {
    #[default]
    Daily,
    Weekly,
    Monthly,
}

impl Interval {
    fn ms(self) -> u64 {
        const DAY: u64 = 86_400_000;
        match self {
            Interval::Daily => DAY,
            Interval::Weekly => 7 * DAY,
            Interval::Monthly => 30 * DAY,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    pub automatically_check: bool,
    pub interval: Interval,
    /// Download in the background and install when the app quits.
    pub automatically_install: bool,
    pub include_beta: bool,
    pub last_check_ms: Option<u64>,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        UpdateSettings {
            automatically_check: true,
            interval: Interval::Daily,
            automatically_install: false,
            include_beta: false,
            last_check_ms: None,
        }
    }
}

impl UpdateSettings {
    fn check_due(&self, now_ms: u64) -> bool {
        self.automatically_check
            && self
                .last_check_ms
                .is_none_or(|t| now_ms.saturating_sub(t) >= self.interval.ms())
    }

    /// The feed to read. A prerelease build (a release candidate) always reads the beta feed,
    /// so testers get the next candidate and then the final release.
    fn feed(&self, running_prerelease: bool) -> String {
        let base = FEED
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .unwrap_or(DEFAULT_FEED);
        let file = if self.include_beta || running_prerelease {
            "beta.json"
        } else {
            "latest.json"
        };
        format!("{}/{file}", base.trim_end_matches('/'))
    }
}

/// What the Updates section shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available {
        version: String,
        notes: String,
    },
    Downloading {
        version: String,
        percent: Option<u8>,
    },
    /// Downloaded; installs when the app quits (or now, on request).
    Ready {
        version: String,
    },
    Installing {
        version: String,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    /// `false` in builds without DECK's release key, or when a package manager installed it.
    pub supported: bool,
    /// Installed from a Linux package (.deb): the package manager updates it.
    pub package_managed: bool,
    pub current_version: String,
    pub settings: UpdateSettings,
    pub status: UpdateStatus,
}

struct Pending {
    update: Update,
    bytes: Option<Vec<u8>>,
}

pub struct Updates {
    status: Mutex<UpdateStatus>,
    pending: tokio::sync::Mutex<Option<Pending>>,
}

impl Default for Updates {
    fn default() -> Self {
        Updates {
            status: Mutex::new(UpdateStatus::Idle),
            pending: tokio::sync::Mutex::new(None),
        }
    }
}

fn now_ms() -> u64 {
    u64::try_from(midnightsnack_core::now_ms()).unwrap_or(0)
}

fn settings(app: &AppHandle) -> UpdateSettings {
    let state = app.state::<HostState>();
    let s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
    s.updates.clone()
}

/// On Linux only the AppImage can replace itself; a .deb belongs to the package manager.
fn package_managed(app: &AppHandle) -> bool {
    cfg!(target_os = "linux") && app.env().appimage.is_none()
}

fn can_update(app: &AppHandle) -> bool {
    pubkey().is_some() && !package_managed(app)
}

fn info(app: &AppHandle) -> UpdateInfo {
    UpdateInfo {
        supported: can_update(app),
        package_managed: package_managed(app),
        current_version: app.package_info().version.to_string(),
        settings: settings(app),
        status: app
            .state::<Updates>()
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone(),
    }
}

fn set_status(app: &AppHandle, status: UpdateStatus) {
    *app.state::<Updates>()
        .status
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = status;
    let _ = app.emit("update-status", info(app));
}

fn outputs_open(app: &AppHandle) -> bool {
    app.webview_windows()
        .keys()
        .any(|label| crate::output::output_id(label).is_some())
}

/// Checks the feed; remembers a newer release and, with automatic installation, downloads it.
async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    if !can_update(app) {
        return Err("unsupported".into());
    }
    set_status(app, UpdateStatus::Checking);
    let settings = settings(app);
    let result = async {
        let prerelease = !app.package_info().version.pre.is_empty();
        let url = settings
            .feed(prerelease)
            .parse()
            .map_err(|_| "invalid feed".to_owned())?;
        let updater = app
            .updater_builder()
            .endpoints(vec![url])
            .map_err(|e| e.to_string())?
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        updater.check().await.map_err(|e| e.to_string())
    }
    .await;
    {
        let state = app.state::<HostState>();
        state
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .updates
            .last_check_ms = Some(now_ms());
        state.save_settings();
    }
    match result {
        Ok(Some(update)) => {
            let version = update.version.clone();
            tracing::info!(%version, "update available");
            set_status(
                app,
                UpdateStatus::Available {
                    version: version.clone(),
                    notes: update.body.clone().unwrap_or_default(),
                },
            );
            *app.state::<Updates>().pending.lock().await = Some(Pending {
                update,
                bytes: None,
            });
            if settings.automatically_install {
                download(app).await?;
            }
            Ok(Some(version))
        }
        Ok(None) => {
            *app.state::<Updates>().pending.lock().await = None;
            set_status(app, UpdateStatus::UpToDate);
            Ok(None)
        }
        Err(message) => {
            tracing::warn!(%message, "update check failed");
            set_status(
                app,
                UpdateStatus::Failed {
                    message: message.clone(),
                },
            );
            Err(message)
        }
    }
}

/// Downloads (and verifies) the pending update.
async fn download(app: &AppHandle) -> Result<(), String> {
    let updates = app.state::<Updates>();
    let mut pending = updates.pending.lock().await;
    let Some(p) = pending.as_mut() else {
        return Err("no update".into());
    };
    if p.bytes.is_some() {
        return Ok(());
    }
    let version = p.update.version.clone();
    set_status(
        app,
        UpdateStatus::Downloading {
            version: version.clone(),
            percent: Some(0),
        },
    );
    let mut received = 0usize;
    let mut last_percent = 0u8;
    let result = p
        .update
        .download(
            |chunk, total| {
                received += chunk;
                let percent = total
                    .filter(|t| *t > 0)
                    .map(|t| (received as u64 * 100 / t).min(100) as u8);
                if percent.is_some_and(|p| p >= last_percent + 5) {
                    last_percent = percent.unwrap_or(0);
                    set_status(
                        app,
                        UpdateStatus::Downloading {
                            version: version.clone(),
                            percent,
                        },
                    );
                }
            },
            || {},
        )
        .await;
    match result {
        Ok(bytes) => {
            p.bytes = Some(bytes);
            set_status(app, UpdateStatus::Ready { version });
            Ok(())
        }
        Err(e) => {
            let message = e.to_string();
            tracing::warn!(%message, "update download failed");
            set_status(
                app,
                UpdateStatus::Failed {
                    message: message.clone(),
                },
            );
            Err(message)
        }
    }
}

/// Runs for the lifetime of the app: checks when due.
pub async fn run(app: AppHandle) {
    if !can_update(&app) {
        tracing::info!("this build does not update itself (no update key, or a package)");
        return;
    }
    // Let the app start before the first network request.
    tokio::time::sleep(Duration::from_secs(20)).await;
    loop {
        if settings(&app).check_due(now_ms()) {
            let _ = check(&app).await;
        }
        tokio::time::sleep(TICK).await;
    }
}

/// Installs a downloaded update as the app quits (automatic installation).
pub fn install_on_exit(app: &AppHandle) {
    let updates = app.state::<Updates>();
    let Ok(pending) = updates.pending.try_lock() else {
        return;
    };
    if let Some(Pending {
        update,
        bytes: Some(bytes),
    }) = pending.as_ref()
    {
        tracing::info!(version = %update.version, "installing update on quit");
        if let Err(e) = update.install(bytes) {
            tracing::warn!(error = %e, "installing the update failed");
        }
    }
}

#[tauri::command]
pub fn update_info(app: AppHandle) -> UpdateInfo {
    info(&app)
}

#[tauri::command]
pub fn set_update_settings(app: AppHandle, state: State<'_, HostState>, settings: UpdateSettings) {
    {
        let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
        let last_check_ms = s.updates.last_check_ms;
        s.updates = UpdateSettings {
            last_check_ms,
            ..settings
        };
    }
    state.save_settings();
    let _ = app.emit("update-status", info(&app));
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<Option<String>, String> {
    check(&app).await
}

/// Downloads if needed, installs and restarts. Refused while an output is open.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    if outputs_open(&app) {
        return Err("outputs_open".into());
    }
    download(&app).await?;
    let updates = app.state::<Updates>();
    // Taken out, so that quitting for the restart does not install it a second time.
    let Some(Pending {
        update,
        bytes: Some(bytes),
    }) = updates.pending.lock().await.take()
    else {
        return Err("no update".into());
    };
    set_status(
        &app,
        UpdateStatus::Installing {
            version: update.version.clone(),
        },
    );
    update.install(&bytes).map_err(|e| {
        let message = e.to_string();
        set_status(
            &app,
            UpdateStatus::Failed {
                message: message.clone(),
            },
        );
        message
    })?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_are_due_by_interval() {
        let day = 86_400_000;
        let mut s = UpdateSettings::default();
        assert!(s.check_due(10 * day), "never checked");
        s.last_check_ms = Some(10 * day);
        assert!(!s.check_due(10 * day + day / 2));
        assert!(s.check_due(11 * day));
        s.interval = Interval::Weekly;
        assert!(!s.check_due(11 * day));
        assert!(s.check_due(17 * day));
        s.automatically_check = false;
        assert!(!s.check_due(100 * day));
    }

    #[test]
    fn beta_users_read_the_beta_feed() {
        let mut s = UpdateSettings::default();
        assert!(s
            .feed(false)
            .ends_with("/releases/latest/download/latest.json"));
        assert!(
            s.feed(true).ends_with("/beta.json"),
            "release candidates follow betas"
        );
        s.include_beta = true;
        assert!(s
            .feed(false)
            .ends_with("/releases/latest/download/beta.json"));
    }

    #[test]
    fn older_settings_files_get_defaults() {
        let s: UpdateSettings = serde_json::from_str(r#"{"include_beta":true}"#).unwrap();
        assert!(s.include_beta && s.automatically_check && !s.automatically_install);
    }
}
