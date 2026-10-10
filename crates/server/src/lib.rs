// SPDX-License-Identifier: GPL-3.0-or-later
//! The midnightsnack host's embedded server: serves the web remote, the pairing API, slide
//! images and the realtime WebSocket. Every action from any client goes through
//! [`AppState::perform`], i.e. the core dispatcher.

mod api;
mod assets;
mod autosave;
mod control_api;
mod devices;
pub mod discovery;
mod host_actions;
pub mod https;
mod inbox;
mod listen;
mod openslides_service;
mod osc_service;
mod pairing;
pub mod relay_link;
mod scheduler;
pub mod state;
mod tunnel;
mod util;

pub use util::write_json_atomic;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::{header, HeaderValue};
use axum::routing::{get, post};
use axum::Router;
use midnightsnack_core::APP_VERSION;
use midnightsnack_protocol::{HostInfo, Role, PROTOCOL_VERSION};
use midnightsnack_render::{RenderCache, RenderService};
use tokio::net::TcpListener;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::RequestBodyTimeoutLayer;

pub use state::{AppState, Event};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address to listen on. Port 0 picks a free port.
    pub bind: SocketAddr,
    /// If the port is taken, fall back to a free one instead of failing.
    pub fallback_to_free_port: bool,
    pub host_name: String,
    /// Persistent data (devices, settings, autosave, extracted shows). `None` keeps
    /// everything in memory (tests).
    pub data_dir: Option<PathBuf>,
    pub cache_dir: PathBuf,
    /// Extra directories to search for the PDFium library.
    pub pdfium_dirs: Vec<PathBuf>,
    pub mdns: bool,
    pub restore_autosave: bool,
    /// Largest accepted upload.
    pub max_upload_bytes: u64,
}

impl ServerConfig {
    pub fn new(cache_dir: PathBuf) -> Self {
        ServerConfig {
            bind: SocketAddr::from(([0, 0, 0, 0], midnightsnack_protocol::DEFAULT_PORT)),
            fallback_to_free_port: true,
            host_name: default_host_name(),
            data_dir: None,
            cache_dir,
            pdfium_dirs: Vec::new(),
            mdns: true,
            restore_autosave: true,
            max_upload_bytes: inbox::DEFAULT_MAX_UPLOAD_BYTES,
        }
    }
}

pub fn default_host_name() -> String {
    std::env::var("MIDNIGHTSNACK_HOST_NAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "midnightsnack".to_owned())
}

/// A running server.
pub struct ServerHandle {
    pub addr: SocketAddr,
    pub state: Arc<AppState>,
    /// Admin token for the host's operator window (loopback only).
    pub operator_token: String,
    /// Operator token for the host's output windows (loopback only).
    pub output_token: String,
    /// Read-only token for the host's stage display windows (loopback only).
    pub stage_token: String,
    _mdns: Option<discovery::Advertisement>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl ServerHandle {
    pub fn local_ws_url(&self) -> String {
        format!("ws://127.0.0.1:{}/api/v1/ws", self.addr.port())
    }

    pub fn local_http_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/info", get(api::info))
        .route(
            "/api/v1/pair",
            post(api::pair).layer(axum::extract::DefaultBodyLimit::max(16 * 1024)),
        )
        .route("/api/v1/pair/{request_id}", get(api::pair_status))
        .route("/api/v1/ws", get(ws::handler))
        .route("/api/v1/action", post(control_api::action))
        .route("/api/v1/state", get(control_api::summary))
        .route("/api/v1/media/slide/{cue_id}/{slide}", get(api::slide))
        .route("/api/v1/media/file/{cue_id}", get(api::media_file))
        .route("/api/v1/media/asset/{asset_id}", get(api::asset))
        .route("/api/v1/media/capture/{cue_id}", get(api::capture))
        .route(
            "/api/v1/upload",
            post(inbox::upload).layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .fallback(assets::serve)
        // Request bodies must keep arriving (slow uploads are fine, stalled ones are not).
        .layer(RequestBodyTimeoutLayer::new(
            std::time::Duration::from_secs(30),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .with_state(state)
}

/// Keeps `LiveState::capture_lost` in sync with the capture workers.
async fn capture_status(state: Arc<AppState>) {
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(500));
    loop {
        tick.tick().await;
        let lost_sources = state.capture.lost_sources();
        let change = {
            let mut engine = state::lock(&state.engine);
            let lost: Vec<String> = engine
                .show()
                .cues
                .iter()
                .filter(|c| match &c.content {
                    midnightsnack_core::CueContent::Capture { capture } => {
                        lost_sources.contains(&capture.source)
                    }
                    _ => false,
                })
                .map(|c| c.id.clone())
                .collect();
            engine.set_capture_lost(lost)
        };
        // Not an edit: only broadcast.
        if change.live {
            state.emit(Event::Live);
        }
    }
}

fn current_interfaces(loopback: bool) -> Vec<midnightsnack_protocol::NetworkInterface> {
    let loopback_only = || {
        vec![midnightsnack_protocol::NetworkInterface {
            name: "loopback".into(),
            address: "127.0.0.1".into(),
            hotspot: false,
        }]
    };
    if loopback {
        return loopback_only();
    }
    let v = discovery::interfaces();
    if v.is_empty() {
        loopback_only()
    } else {
        v
    }
}

/// Notices new interfaces (a hotspot was started, Wi-Fi changed) and updates join links.
async fn watch_interfaces(state: Arc<AppState>) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(3));
    loop {
        tick.tick().await;
        let now = tokio::task::spawn_blocking(|| current_interfaces(false))
            .await
            .unwrap_or_default();
        let changed = {
            let mut cur = state::lock(&state.interfaces);
            let changed = *cur != now && !now.is_empty();
            if changed {
                *cur = now;
            }
            changed
        };
        if changed {
            tracing::info!("network interfaces changed");
            state::refresh_base_urls(&state);
            state.emit(Event::Connectivity);
        }
    }
}

/// Starts the server on the current Tokio runtime.
pub async fn start(config: ServerConfig) -> std::io::Result<ServerHandle> {
    if let Some(d) = &config.data_dir {
        std::fs::create_dir_all(d)?;
    }
    let render = RenderService::start(
        RenderCache::new(&config.cache_dir),
        config.pdfium_dirs.clone(),
    );
    let engine = config
        .data_dir
        .as_deref()
        .filter(|_| config.restore_autosave)
        .and_then(autosave::restore)
        .unwrap_or_default();
    let host = HostInfo {
        name: config.host_name.clone(),
        app_version: APP_VERSION.to_owned(),
        protocol_version: PROTOCOL_VERSION,
    };
    let state = AppState::new(host, render, config.data_dir.clone(), engine);
    state.max_upload.store(
        config.max_upload_bytes,
        std::sync::atomic::Ordering::Relaxed,
    );
    inbox::clear_pending(&state);

    let listener = match TcpListener::bind(config.bind).await {
        Ok(l) => l,
        Err(e) if config.fallback_to_free_port && config.bind.port() != 0 => {
            tracing::warn!(error = %e, port = config.bind.port(), "port busy, using a free port");
            TcpListener::bind(SocketAddr::new(config.bind.ip(), 0)).await?
        }
        Err(e) => return Err(e),
    };
    let addr = listener.local_addr()?;

    state
        .http_port
        .store(addr.port(), std::sync::atomic::Ordering::Relaxed);
    let loopback = config.bind.ip().is_loopback();
    *state::lock(&state.interfaces) = current_interfaces(loopback);
    state::refresh_base_urls(&state);

    let (operator_token, output_token, stage_token) = {
        let mut d = state::lock(&state.devices);
        (
            d.add_local("Operator", Role::Admin),
            d.add_local("Output", Role::Operator),
            d.add_local("Stage display", Role::StageViewer),
        )
    };

    // Forward render queue length to clients.
    {
        let state = state.clone();
        let mut rx = state.render.subscribe_queued();
        tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                let n = *rx.borrow_and_update();
                state.emit(Event::RenderProgress(n));
            }
        });
    }
    tokio::spawn(autosave::run(state.clone()));
    tokio::spawn(scheduler::run(state.clone()));
    tokio::spawn(capture_status(state.clone()));
    tokio::spawn(osc_service::run(state.clone()));
    tokio::spawn(host_actions::refresh_conversions(state.clone()));
    tokio::spawn(https::run(
        state.clone(),
        router(state.clone()),
        config.bind.ip(),
        https::default_port(config.bind.port()),
    ));
    tokio::spawn(relay_link::run(state.clone(), router(state.clone())));
    tokio::spawn(openslides_service::run(state.clone()));
    if !loopback {
        tokio::spawn(watch_interfaces(state.clone()));
    }
    state.prefetch();

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let app = router(state.clone());
    tokio::spawn(listen::serve_plain(listener, app, async {
        let _ = rx.await;
    }));
    tracing::info!(%addr, "server listening");

    let mdns = if config.mdns && !config.bind.ip().is_loopback() {
        discovery::advertise(&config.host_name, addr.port(), APP_VERSION)
    } else {
        None
    };

    Ok(ServerHandle {
        addr,
        state,
        operator_token,
        output_token,
        stage_token,
        _mdns: mdns,
        shutdown: Some(tx),
    })
}
