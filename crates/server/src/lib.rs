// SPDX-License-Identifier: GPL-3.0-or-later
//! The midnightsnack host's embedded server: serves the web remote, the pairing API, slide
//! images and the realtime WebSocket. Every action from any client goes through
//! [`AppState::perform`], i.e. the core dispatcher.

mod api;
mod assets;
mod autosave;
mod devices;
pub mod discovery;
mod host_actions;
mod pairing;
mod scheduler;
pub mod state;
mod util;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::{header, HeaderValue};
use axum::routing::{get, post};
use axum::serve::ListenerExt;
use axum::Router;
use midnightsnack_core::APP_VERSION;
use midnightsnack_protocol::{HostInfo, Role, PROTOCOL_VERSION};
use midnightsnack_render::{RenderCache, RenderService};
use tokio::net::TcpListener;
use tower_http::set_header::SetResponseHeaderLayer;

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
        .route("/api/v1/pair", post(api::pair))
        .route("/api/v1/pair/{request_id}", get(api::pair_status))
        .route("/api/v1/ws", get(ws::handler))
        .route("/api/v1/media/slide/{cue_id}/{slide}", get(api::slide))
        .route("/api/v1/media/file/{cue_id}", get(api::media_file))
        .route("/api/v1/media/asset/{asset_id}", get(api::asset))
        .fallback(assets::serve)
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

    let listener = match TcpListener::bind(config.bind).await {
        Ok(l) => l,
        Err(e) if config.fallback_to_free_port && config.bind.port() != 0 => {
            tracing::warn!(error = %e, port = config.bind.port(), "port busy, using a free port");
            TcpListener::bind(SocketAddr::new(config.bind.ip(), 0)).await?
        }
        Err(e) => return Err(e),
    };
    let addr = listener.local_addr()?;

    let base_urls: Vec<String> = if config.bind.ip().is_loopback() {
        vec![format!("http://127.0.0.1:{}", addr.port())]
    } else {
        let mut v: Vec<String> = discovery::lan_addresses()
            .into_iter()
            .map(|ip| format!("http://{ip}:{}", addr.port()))
            .collect();
        if v.is_empty() {
            v.push(format!("http://127.0.0.1:{}", addr.port()));
        }
        v
    };
    *state::lock(&state.base_urls) = base_urls;

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
    state.prefetch();

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let app = router(state.clone()).into_make_service_with_connect_info::<SocketAddr>();
    // Live control sends many tiny messages; never let Nagle's algorithm delay them.
    let listener = listener.tap_io(|tcp| {
        if let Err(e) = tcp.set_nodelay(true) {
            tracing::debug!(error = %e, "could not set TCP_NODELAY");
        }
    });
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = rx.await;
        });
        if let Err(e) = server.await {
            tracing::error!(error = %e, "server stopped");
        }
    });
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
