// SPDX-License-Identifier: GPL-3.0-or-later
//! Host-only settings (displays, keyboard mapping). Stored as JSON in the app config dir.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HostSettings {
    /// Name of the display the output window was last placed on.
    pub output_display: Option<String>,
    /// Open the output as a normal window (rehearsal on a single screen).
    pub output_windowed: bool,
    /// Display of the stage display window, if one is used.
    pub stage_display: Option<String>,
    pub stage_windowed: bool,
    /// Key (as `KeyboardEvent.key`, or `Shift+F5` style) -> action name. Overrides defaults.
    pub keymap: BTreeMap<String, String>,
    /// Port of the embedded server.
    pub port: Option<u16>,
}

pub fn path(config_dir: &Path) -> PathBuf {
    config_dir.join("host-settings.json")
}

pub fn load(config_dir: &Path) -> HostSettings {
    std::fs::read(path(config_dir))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save(config_dir: &Path, s: &HostSettings) {
    let result = std::fs::create_dir_all(config_dir)
        .and_then(|_| std::fs::write(path(config_dir), serde_json::to_vec_pretty(s)?));
    if let Err(e) = result {
        tracing::error!(error = %e, "failed to save host settings");
    }
}
