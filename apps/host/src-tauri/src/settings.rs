// SPDX-License-Identifier: GPL-3.0-or-later
//! Host-only settings (display assignment, keyboard mapping). Stored as JSON in the app config
//! dir. Which outputs exist is part of the show; *where* they appear is a property of this host.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Where an output's window goes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Placement {
    /// Display name, remembered so the output returns there when re-plugged.
    pub display: Option<String>,
    /// A normal window instead of covering a display (rehearsal on one screen).
    pub windowed: bool,
    /// Reopen this output on launch.
    pub open: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HostSettings {
    /// Output id -> placement.
    pub outputs: BTreeMap<String, Placement>,
    /// Key (as `KeyboardEvent.key`, or `Shift+F5` style) -> action name. Overrides defaults.
    pub keymap: BTreeMap<String, String>,
    /// Port of the embedded server.
    pub port: Option<u16>,
    pub midi: crate::midi::MidiSettings,
    // Phase 1/2 fields, migrated into `outputs["main"]`.
    #[serde(skip_serializing)]
    output_display: Option<String>,
    #[serde(skip_serializing)]
    output_windowed: bool,
}

pub fn path(config_dir: &Path) -> PathBuf {
    config_dir.join("host-settings.json")
}

pub fn load(config_dir: &Path) -> HostSettings {
    let mut s: HostSettings = std::fs::read(path(config_dir))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if let Some(display) = s.output_display.take() {
        s.outputs.entry("main".into()).or_insert(Placement {
            display: Some(display),
            windowed: s.output_windowed,
            open: true,
        });
    }
    s
}

pub fn save(config_dir: &Path, s: &HostSettings) {
    let result = std::fs::create_dir_all(config_dir)
        .and_then(|_| std::fs::write(path(config_dir), serde_json::to_vec_pretty(s)?));
    if let Err(e) = result {
        tracing::error!(error = %e, "failed to save host settings");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_single_output_settings() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            path(dir.path()),
            r#"{"output_display":"HDMI-1","output_windowed":false,"keymap":{"x":"next"}}"#,
        )
        .unwrap();
        let s = load(dir.path());
        assert_eq!(
            s.outputs["main"],
            Placement {
                display: Some("HDMI-1".into()),
                windowed: false,
                open: true
            }
        );
        save(dir.path(), &s);
        let raw = std::fs::read_to_string(path(dir.path())).unwrap();
        assert!(!raw.contains("output_display"));
        assert_eq!(load(dir.path()).outputs.len(), 1);
    }
}
