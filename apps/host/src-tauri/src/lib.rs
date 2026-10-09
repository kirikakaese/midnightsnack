// SPDX-License-Identifier: GPL-3.0-or-later
//! Tauri desktop host: starts the embedded server, manages output windows and displays.
//!
//! The operator and output windows talk to the core through the same WebSocket protocol as
//! remote devices (with loopback-only tokens), so every control path shares one dispatcher.

mod controller;
mod hotplug;
mod midi;
mod output;
mod settings;
mod web;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use midnightsnack_core::APP_VERSION;
use midnightsnack_protocol::{HostInfo, MidiTrigger, OutputFeed, DEFAULT_PORT, PROTOCOL_VERSION};
use midnightsnack_server::{start, state::lock, ServerConfig, ServerHandle};
use serde::Serialize;
use tauri::{AppHandle, Manager, State, Webview};

use crate::output::DisplayInfo;
use crate::settings::{HostSettings, Placement};

pub(crate) struct HostState {
    server: ServerHandle,
    pub(crate) settings: Mutex<HostSettings>,
    config_dir: PathBuf,
    keep_awake: Mutex<Option<keepawake::KeepAwake>>,
}

impl HostState {
    fn save_settings(&self) {
        let s = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        settings::save(&self.config_dir, &s);
    }
}

#[derive(Serialize)]
struct ConnectionInfo {
    ws_url: String,
    http_base: String,
    token: String,
    /// For output windows: the output this window shows.
    output_id: Option<String>,
    /// This window controls another host (controller mode).
    controller: bool,
}

#[tauri::command]
fn host_info(state: State<'_, HostState>) -> HostInfo {
    state.server.state.host.clone()
}

/// Token and URLs for the calling webview: program outputs get an operator token, stage
/// outputs a read-only one, the operator window an admin token. All only work from this machine.
#[tauri::command]
fn connection_info(webview: Webview, state: State<'_, HostState>) -> ConnectionInfo {
    if let Some(remote) =
        controller::remote_id(webview.label()).and_then(|id| controller::remote(&state, id))
    {
        return ConnectionInfo {
            ws_url: remote.ws_url(),
            http_base: remote.base_url.clone(),
            token: remote.token,
            output_id: None,
            controller: true,
        };
    }
    let s = &state.server;
    let output_id = output::output_id(webview.label()).map(str::to_owned);
    let token = match &output_id {
        None => s.operator_token.clone(),
        Some(id) => {
            let engine = lock(&s.state.engine);
            let stage = engine
                .show()
                .outputs
                .iter()
                .any(|o| o.id == *id && o.feed == OutputFeed::Stage);
            if stage {
                s.stage_token.clone()
            } else {
                s.output_token.clone()
            }
        }
    };
    ConnectionInfo {
        ws_url: s.local_ws_url(),
        http_base: s.local_http_url(),
        token,
        output_id,
        controller: false,
    }
}

#[tauri::command]
fn list_displays(app: AppHandle) -> Vec<DisplayInfo> {
    output::list_displays(&app)
}

#[tauri::command]
fn display_status(app: AppHandle) -> hotplug::DisplayStatus {
    hotplug::status(&app)
}

#[derive(Serialize)]
struct OutputWindowState {
    #[serde(flatten)]
    placement: Placement,
    /// The window exists.
    window_open: bool,
}

/// Placement and window state of every output configured on this host.
#[tauri::command]
fn output_windows(
    app: AppHandle,
    state: State<'_, HostState>,
) -> BTreeMap<String, OutputWindowState> {
    let s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
    s.outputs
        .iter()
        .map(|(id, p)| {
            (
                id.clone(),
                OutputWindowState {
                    placement: p.clone(),
                    window_open: output::window(&app, id).is_some(),
                },
            )
        })
        .collect()
}

#[tauri::command]
fn open_output(
    app: AppHandle,
    state: State<'_, HostState>,
    output_id: String,
    display: Option<String>,
    windowed: bool,
) -> Result<(), String> {
    let exists = lock(&state.server.state.engine)
        .show()
        .outputs
        .iter()
        .any(|o| o.id == output_id);
    if !exists {
        return Err(format!("unknown output {output_id}"));
    }
    output::open(&app, &output_id, display.as_deref(), windowed).map_err(|e| e.to_string())?;
    state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .outputs
        .insert(
            output_id,
            Placement {
                display,
                windowed,
                open: true,
            },
        );
    state.save_settings();
    update_keep_awake(&app, &state);
    web::WINDOWS_CHANGED.notify_one();
    Ok(())
}

#[tauri::command]
fn close_output(
    app: AppHandle,
    state: State<'_, HostState>,
    output_id: String,
) -> Result<(), String> {
    output::close(&app, &output_id).map_err(|e| e.to_string())?;
    if let Some(p) = state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .outputs
        .get_mut(&output_id)
    {
        // Closing on purpose: do not reopen on the next launch.
        p.open = false;
    }
    state.save_settings();
    update_keep_awake(&app, &state);
    web::WINDOWS_CHANGED.notify_one();
    Ok(())
}

/// Screen sleep is prevented while any output window is open.
fn update_keep_awake(app: &AppHandle, state: &HostState) {
    let any_open = app.windows().keys().any(|l| output::output_id(l).is_some());
    let mut guard = state.keep_awake.lock().unwrap_or_else(|e| e.into_inner());
    if !any_open {
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
fn keymap(state: State<'_, HostState>) -> BTreeMap<String, String> {
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

/// Opens the macOS privacy settings for Screen Recording.
#[tauri::command]
fn midi_ports() -> Vec<String> {
    midnightsnack_control::midi::port_names()
}

#[tauri::command]
fn midi_settings(state: State<'_, HostState>) -> midi::MidiSettings {
    state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .midi
        .clone()
}

#[tauri::command]
fn set_midi_settings(state: State<'_, HostState>, settings: midi::MidiSettings) {
    state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .midi = settings;
    state.save_settings();
}

/// Waits up to 10 s for the next MIDI press; `null` on timeout.
#[tauri::command]
async fn midi_learn(learn: State<'_, midi::Learn>) -> Result<Option<MidiTrigger>, ()> {
    Ok(learn.next(std::time::Duration::from_secs(10)).await)
}

#[tauri::command]
fn open_capture_settings() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn();
    }
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
    config.data_dir = Some(data_dir.clone());
    config.pdfium_dirs = pdfium_dirs(&handle);
    let smoke = std::env::var_os("MIDNIGHTSNACK_SMOKE_TEST").is_some();
    if smoke {
        config.mdns = false;
        config.restore_autosave = false;
        config.bind.set_port(0);
    }
    let server = tauri::async_runtime::block_on(start(config))?;
    let server_state = server.state.clone();

    let reopen: Vec<(String, Placement)> = host_settings
        .outputs
        .iter()
        .filter(|(_, p)| p.open)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    app.manage(HostState {
        server,
        settings: Mutex::new(host_settings),
        config_dir,
        keep_awake: Mutex::new(None),
    });

    // Restore outputs that were open, on their displays if connected (the watcher brings the
    // others back when their display appears).
    let known: Vec<String> = lock(&server_state.engine)
        .show()
        .outputs
        .iter()
        .map(|o| o.id.clone())
        .collect();
    let displays = output::list_displays(&handle);
    for (id, p) in reopen {
        let present = p.windowed
            || p.display
                .as_ref()
                .is_none_or(|d| displays.iter().any(|x| x.name == *d));
        if !known.contains(&id) || !present || smoke {
            continue;
        }
        if let Err(e) = output::open(&handle, &id, p.display.as_deref(), p.windowed) {
            tracing::warn!(error = %e, output = %id, "could not restore output window");
        }
    }
    update_keep_awake(&handle, &app.state::<HostState>());

    hotplug::spawn(handle.clone());
    app.manage(midi::Learn::default());
    if !smoke {
        midi::spawn(handle.clone(), server_state.clone());
    }
    tauri::async_runtime::spawn(web::run(handle, server_state, data_dir.join("web")));
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
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Closing the operator window quits the app (including outputs).
                if window.label() == "operator" {
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            host_info,
            connection_info,
            list_displays,
            display_status,
            output_windows,
            open_output,
            close_output,
            keymap,
            qr_svg,
            open_capture_settings,
            midi_ports,
            midi_settings,
            set_midi_settings,
            midi_learn,
            controller::discover_hosts,
            controller::remote_hosts,
            controller::forget_remote,
            controller::pair_remote,
            controller::open_controller,
            ui_ready
        ])
        .run(tauri::generate_context!())
        .expect("error while running midnightsnack");
}
