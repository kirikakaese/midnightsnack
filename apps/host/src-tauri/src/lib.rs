// SPDX-License-Identifier: GPL-3.0-or-later
//! Tauri desktop host: window management and the bridge between the UI and the core.

use midnightsnack_core::APP_VERSION;
use midnightsnack_protocol::{HostInfo, PROTOCOL_VERSION};

#[tauri::command]
fn host_info() -> HostInfo {
    HostInfo {
        name: hostname(),
        app_version: APP_VERSION.to_owned(),
        protocol_version: PROTOCOL_VERSION,
    }
}

/// Called by the operator window once it has rendered. In smoke-test mode
/// (`MIDNIGHTSNACK_SMOKE_TEST=1`, used by CI) the app exits successfully.
#[tauri::command]
fn ui_ready(app: tauri::AppHandle) {
    if std::env::var_os("MIDNIGHTSNACK_SMOKE_TEST").is_some() {
        tracing::info!("smoke test: operator UI ready, exiting");
        app.exit(0);
    }
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "midnightsnack".to_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![host_info, ui_ready])
        .run(tauri::generate_context!())
        .expect("error while running midnightsnack");
}
