// SPDX-License-Identifier: GPL-3.0-or-later
//! The laptop as its own Wi-Fi access point. NetworkManager (`nmcli`) can create one without
//! extra privileges for the logged-in user; Windows and macOS only offer it in their settings,
//! so the operator gets a shortcut there and step-by-step instructions instead.

use std::process::Command;

use serde::Serialize;

/// NetworkManager connection created for the hotspot.
const CONNECTION: &str = "midnightsnack-hotspot";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HotspotStatus {
    /// `linux`, `windows`, `macos` or another `std::env::consts::OS`.
    pub os: &'static str,
    /// This computer can start a hotspot from the app (NetworkManager present).
    pub can_start: bool,
    /// The app's hotspot is running.
    pub active: bool,
    pub ssid: Option<String>,
}

fn nmcli(args: &[&str]) -> Result<String, String> {
    let out = Command::new("nmcli")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        Err(if err.is_empty() {
            format!("nmcli exited with {}", out.status)
        } else {
            err
        })
    }
}

fn nmcli_available() -> bool {
    cfg!(target_os = "linux") && nmcli(&["--version"]).is_ok()
}

/// Whether `nmcli -t -f NAME connection show --active` lists the hotspot connection.
fn active_in(terse: &str) -> bool {
    terse
        .lines()
        .any(|l| l.split(':').next() == Some(CONNECTION))
}

pub fn status() -> HotspotStatus {
    let can_start = nmcli_available();
    let active = can_start
        && nmcli(&["-t", "-f", "NAME", "connection", "show", "--active"])
            .is_ok_and(|o| active_in(&o));
    let ssid = if active {
        nmcli(&[
            "-t",
            "-g",
            "802-11-wireless.ssid",
            "connection",
            "show",
            CONNECTION,
        ])
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
    } else {
        None
    };
    HotspotStatus {
        os: std::env::consts::OS,
        can_start,
        active,
        ssid,
    }
}

/// SSIDs are 1–32 bytes; WPA2 passwords 8–63 printable ASCII characters.
pub fn validate(ssid: &str, password: &str) -> Result<(), String> {
    if ssid.is_empty() || ssid.len() > 32 || ssid.chars().any(char::is_control) {
        return Err("invalid_ssid".into());
    }
    if !(8..=63).contains(&password.len()) || !password.chars().all(|c| c.is_ascii_graphic()) {
        return Err("invalid_password".into());
    }
    Ok(())
}

#[tauri::command]
pub fn hotspot_status() -> HotspotStatus {
    status()
}

#[tauri::command]
pub async fn hotspot_start(ssid: String, password: String) -> Result<HotspotStatus, String> {
    validate(&ssid, &password)?;
    if !nmcli_available() {
        return Err("unsupported".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        nmcli(&[
            "device", "wifi", "hotspot", "con-name", CONNECTION, "ssid", &ssid, "password",
            &password,
        ])?;
        Ok(status())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn hotspot_stop() -> Result<HotspotStatus, String> {
    tauri::async_runtime::spawn_blocking(|| {
        nmcli(&["connection", "down", CONNECTION])?;
        Ok(status())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Opens the operating system's hotspot settings.
#[tauri::command]
pub fn open_hotspot_settings() {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("explorer.exe")
            .arg("ms-settings:network-mobilehotspot")
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.Sharing-Settings.extension")
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        // GNOME; other desktops simply ignore it.
        let _ = Command::new("gnome-control-center").arg("wifi").spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_active_hotspot_connection() {
        assert!(active_in("Wired connection 1\nmidnightsnack-hotspot\n"));
        assert!(!active_in(
            "Wired connection 1\nmidnightsnack-hotspot-old\n"
        ));
        assert!(!active_in(""));
    }

    #[test]
    fn validates_ssid_and_password() {
        assert!(validate("midnightsnack", "snacks-at-9").is_ok());
        assert!(validate("", "snacks-at-9").is_err());
        assert!(validate(&"x".repeat(33), "snacks-at-9").is_err());
        assert!(validate("ok", "short").is_err());
        assert!(validate("ok", "has space in it").is_err());
    }
}
