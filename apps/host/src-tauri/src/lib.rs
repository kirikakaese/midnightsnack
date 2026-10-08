// SPDX-License-Identifier: GPL-3.0-or-later
//! Tauri desktop host: starts the embedded server, manages windows and displays.
//!
//! The operator and output windows talk to the core through the same WebSocket protocol as
//! remote devices (with loopback-only tokens), so every control path shares one dispatcher.

mod output;
mod settings;

use std::path::PathBuf;
use std::sync::Mutex;

use midnightsnack_core::APP_VERSION;
use midnightsnack_protocol::{HostInfo, DEFAULT_PORT, PROTOCOL_VERSION};
use midnightsnack_server::{start, ServerConfig, ServerHandle};
use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::output::{DisplayInfo, DisplayWindow};
use crate::settings::HostSettings;

struct HostState {
    server: ServerHandle,
    settings: Mutex<HostSettings>,
    config_dir: PathBuf,
    keep_awake: Mutex<Option<keepawake::KeepAwake>>,
}

#[derive(Serialize)]
struct ConnectionInfo {
    ws_url: String,
    http_base: String,
    token: String,
}

#[tauri::command]
fn host_info(state: State<'_, HostState>) -> HostInfo {
    state.server.state.host.clone()
}

/// Token and URLs for the calling window. Output windows get an operator token, the operator
/// window an admin token; both only work from this machine.
#[tauri::command]
fn connection_info(window: WebviewWindow, state: State<'_, HostState>) -> ConnectionInfo {
    let s = &state.server;
    let label = window.label();
    let token = if label.starts_with("output-") {
        s.output_token.clone()
    } else if label.starts_with("stage-") {
        s.stage_token.clone()
    } else {
        s.operator_token.clone()
    };
    ConnectionInfo {
        ws_url: s.local_ws_url(),
        http_base: s.local_http_url(),
        token,
    }
}

#[tauri::command]
fn list_displays(app: AppHandle) -> Vec<DisplayInfo> {
    output::list_displays(&app)
}

#[derive(Serialize)]
struct WindowState {
    open: bool,
    display: Option<String>,
    windowed: bool,
}

fn parse_kind(kind: &str) -> Result<DisplayWindow, String> {
    match kind {
        "output" => Ok(DisplayWindow::Output),
        "stage" => Ok(DisplayWindow::Stage),
        other => Err(format!("unknown window kind {other}")),
    }
}

/// State of the output (`kind = "output"`) or stage display (`kind = "stage"`) window.
#[tauri::command]
fn window_state(
    app: AppHandle,
    state: State<'_, HostState>,
    kind: String,
) -> Result<WindowState, String> {
    let kind = parse_kind(&kind)?;
    let s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
    let (display, windowed) = match kind {
        DisplayWindow::Output => (s.output_display.clone(), s.output_windowed),
        DisplayWindow::Stage => (s.stage_display.clone(), s.stage_windowed),
    };
    Ok(WindowState {
        open: output::is_open(&app, kind),
        display,
        windowed,
    })
}

#[tauri::command]
fn open_window(
    app: AppHandle,
    state: State<'_, HostState>,
    kind: String,
    display: Option<String>,
    windowed: bool,
) -> Result<(), String> {
    let kind = parse_kind(&kind)?;
    output::open(&app, kind, display.as_deref(), windowed).map_err(|e| e.to_string())?;
    {
        let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
        match kind {
            DisplayWindow::Output => {
                s.output_display = display;
                s.output_windowed = windowed;
            }
            DisplayWindow::Stage => {
                s.stage_display = display;
                s.stage_windowed = windowed;
            }
        }
        settings::save(&state.config_dir, &s);
    }
    if kind == DisplayWindow::Output {
        set_keep_awake(&state, true);
    }
    Ok(())
}

#[tauri::command]
fn close_window(app: AppHandle, state: State<'_, HostState>, kind: String) -> Result<(), String> {
    let kind = parse_kind(&kind)?;
    output::close(&app, kind).map_err(|e| e.to_string())?;
    {
        // Closing on purpose means: do not reopen on the next launch.
        let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
        match kind {
            DisplayWindow::Output => s.output_display = None,
            DisplayWindow::Stage => s.stage_display = None,
        }
        settings::save(&state.config_dir, &s);
    }
    if kind == DisplayWindow::Output {
        set_keep_awake(&state, false);
    }
    Ok(())
}

fn set_keep_awake(state: &HostState, on: bool) {
    let mut guard = state.keep_awake.lock().unwrap_or_else(|e| e.into_inner());
    if !on {
        *guard = None;
        return;
    }
    if guard.is_none() {
        match keepawake::Builder::default()
            .display(true)
            .idle(true)
            .reason("Presentation output is open")
            .app_name("midnightsnack")
            .app_reverse_domain("io.github.kirikakaese.midnightsnack")
            .create()
        {
            Ok(k) => *guard = Some(k),
            Err(e) => tracing::warn!(error = %e, "could not prevent screen sleep"),
        }
    }
}

/// Keyboard overrides from the host settings file.
#[tauri::command]
fn keymap(state: State<'_, HostState>) -> std::collections::BTreeMap<String, String> {
    state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .keymap
        .clone()
}

/// QR code for a join URL, as an SVG document.
#[tauri::command]
fn qr_svg(text: String) -> Result<String, String> {
    let code = qrcode::QrCode::new(text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(code
        .render::<qrcode::render::svg::Color<'_>>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build())
}

/// Called by the operator window once it has rendered. In smoke-test mode
/// (`MIDNIGHTSNACK_SMOKE_TEST=1`, used by CI) the app exits successfully.
#[tauri::command]
fn ui_ready(app: AppHandle) {
    if std::env::var_os("MIDNIGHTSNACK_SMOKE_TEST").is_some() {
        tracing::info!("smoke test: operator UI ready, exiting");
        app.exit(0);
    }
}

fn pdfium_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        dirs.push(res.join("resources").join("pdfium"));
        dirs.push(res.join("pdfium"));
        dirs.push(res);
    }
    // Development builds: the repository's download location.
    dirs.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("pdfium"),
    );
    dirs
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let paths = app.path();
    let config_dir = paths.app_config_dir()?;
    let data_dir = paths.app_data_dir()?;
    let cache_dir = paths.app_cache_dir()?.join("render");
    let host_settings = settings::load(&config_dir);

    let mut config = ServerConfig::new(cache_dir);
    config
        .bind
        .set_port(host_settings.port.unwrap_or(DEFAULT_PORT));
    config.data_dir = Some(data_dir);
    config.pdfium_dirs = pdfium_dirs(&handle);
    if std::env::var_os("MIDNIGHTSNACK_SMOKE_TEST").is_some() {
        config.mdns = false;
        config.restore_autosave = false;
        config.bind.set_port(0);
    }
    let server = tauri::async_runtime::block_on(start(config))?;

    let reopen = [
        (
            DisplayWindow::Output,
            host_settings.output_display.clone(),
            host_settings.output_windowed,
        ),
        (
            DisplayWindow::Stage,
            host_settings.stage_display.clone(),
            host_settings.stage_windowed,
        ),
    ];
    app.manage(HostState {
        server,
        settings: Mutex::new(host_settings),
        config_dir,
        keep_awake: Mutex::new(None),
    });

    // Restore display windows on the displays they were on last time, if present.
    let displays = output::list_displays(&handle);
    for (kind, name, windowed) in reopen {
        let Some(name) = name else { continue };
        if !displays.iter().any(|d| d.name == name) {
            continue;
        }
        if let Err(e) = output::open(&handle, kind, Some(&name), windowed) {
            tracing::warn!(error = %e, ?kind, "could not restore window");
        } else if kind == DisplayWindow::Output {
            set_keep_awake(&app.state::<HostState>(), true);
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    tracing::info!(
        version = APP_VERSION,
        protocol = PROTOCOL_VERSION,
        "starting midnightsnack"
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(setup)
        .on_window_event(|window, event| {
            // Closing the operator window quits the app (including outputs).
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() == "operator" {
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            host_info,
            connection_info,
            list_displays,
            window_state,
            open_window,
            close_window,
            keymap,
            qr_svg,
            ui_ready
        ])
        .run(tauri::generate_context!())
        .expect("error while running midnightsnack");
}
