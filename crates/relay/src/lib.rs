// SPDX-License-Identifier: GPL-3.0-or-later
//! The midnightsnack relay: a dumb, self-hostable pipe between hosts and remotes that cannot
//! reach each other directly.
//!
//! A host keeps one WebSocket at `/relay/v1/host` (authenticated with its secret, and with the
//! relay's access token if one is configured). Each remote opens `/relay/v1/remote/{host_id}`;
//! the relay numbers it as a channel and forwards its binary messages over the host's link as
//! `[op][channel u32 BE][payload]`. Payloads are Noise ciphertext the relay cannot read. The
//! relay also serves the web remote at `/r/{host_id}`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use midnightsnack_protocol::{relay_close, tunnel, NOISE_MAX_MESSAGE};
use rust_embed::RustEmbed;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tower_http::set_header::SetResponseHeaderLayer;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Version of the relay protocol (`/relay/v1`).
pub const RELAY_PROTOCOL: u32 = 1;

const HOST_ID_CONTEXT: &[u8] = b"midnightsnack relay host id";
const PING_INTERVAL: Duration = Duration::from_secs(20);
/// Connections that stay silent this long (no messages, no pongs) are dropped.
const IDLE_TIMEOUT: Duration = Duration::from_secs(65);
/// Messages waiting for a remote before it counts as too slow and is dropped.
const REMOTE_QUEUE: usize = 512;
/// Messages waiting for the host link.
const HOST_QUEUE: usize = 2048;
/// Largest WebSocket message accepted (a Noise message plus the link header).
const MAX_MESSAGE: usize = NOISE_MAX_MESSAGE + 5;

#[derive(RustEmbed)]
#[folder = "../../apps/remote/dist"]
#[allow_missing = true]
struct RemoteAssets;

const CSP: &str = "default-src 'self'; img-src 'self' data: blob:; connect-src 'self' ws: wss:; \
                   style-src 'self' 'unsafe-inline'; frame-ancestors 'none'";

#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Hosts must present this token (`X-Relay-Token`) when set.
    pub access_token: Option<String>,
    pub max_hosts: usize,
    pub max_remotes_per_host: usize,
    /// Sustained messages per second a remote may send (bursts up to twice that).
    pub remote_rate: u32,
}

impl Default for RelayConfig {
    fn default() -> Self {
        RelayConfig {
            access_token: None,
            max_hosts: 200,
            max_remotes_per_host: 64,
            remote_rate: 400,
        }
    }
}

struct HostEntry {
    generation: u64,
    to_host: mpsc::Sender<Vec<u8>>,
    channels: Mutex<HashMap<u32, mpsc::Sender<Vec<u8>>>>,
    cancel: CancellationToken,
}

pub struct Relay {
    config: RelayConfig,
    hosts: Mutex<HashMap<String, Arc<HostEntry>>>,
    next_channel: AtomicU32,
    generation: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Relay {
    pub fn new(config: RelayConfig) -> Arc<Self> {
        Arc::new(Relay {
            config,
            hosts: Mutex::new(HashMap::new()),
            next_channel: AtomicU32::new(1),
            generation: AtomicU64::new(1),
        })
    }

    /// Hosts currently connected.
    pub fn host_count(&self) -> usize {
        lock(&self.hosts).len()
    }

    fn host(&self, id: &str) -> Option<Arc<HostEntry>> {
        lock(&self.hosts).get(id).cloned()
    }
}

/// The host id for a host secret: `base64url(SHA-256(context ‖ secret)[..16])`.
pub fn host_id_for_secret(secret: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(HOST_ID_CONTEXT);
    h.update(secret);
    B64.encode(&h.finalize()[..16])
}

/// Short form of a host id for logs.
fn short(id: &str) -> &str {
    &id[..id.len().min(6)]
}

pub fn router(relay: Arc<Relay>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/relay/v1/info", get(info))
        .route("/relay/v1/host", get(host_handler))
        .route("/relay/v1/remote/{host_id}", get(remote_handler))
        .fallback(assets)
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .with_state(relay)
}

/// Serves until the listener fails.
pub async fn serve(listener: tokio::net::TcpListener, relay: Arc<Relay>) -> std::io::Result<()> {
    axum::serve(
        listener,
        router(relay).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}

async fn info(State(relay): State<Arc<Relay>>) -> Response {
    axum::Json(serde_json::json!({
        "name": "midnightsnack-relay",
        "version": VERSION,
        "protocol": RELAY_PROTOCOL,
        "access_token_required": relay.config.access_token.is_some(),
    }))
    .into_response()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn host_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(relay): State<Arc<Relay>>,
) -> Response {
    let secret = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|v| B64.decode(v.trim()).ok())
        .filter(|s| s.len() == 32);
    let Some(secret) = secret else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if let Some(expected) = &relay.config.access_token {
        let given = headers
            .get("x-relay-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if !constant_time_eq(given.as_bytes(), expected.as_bytes()) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    let id = host_id_for_secret(&secret);
    {
        let hosts = lock(&relay.hosts);
        if hosts.len() >= relay.config.max_hosts && !hosts.contains_key(&id) {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    }
    ws.max_message_size(MAX_MESSAGE)
        .max_frame_size(MAX_MESSAGE)
        .on_upgrade(move |socket| run_host(relay, id, socket))
}

fn link_message(op: u8, channel: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + payload.len());
    out.push(op);
    out.extend_from_slice(&channel.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

async fn run_host(relay: Arc<Relay>, id: String, mut socket: WebSocket) {
    let (to_host, mut from_remotes) = mpsc::channel::<Vec<u8>>(HOST_QUEUE);
    let generation = relay.generation.fetch_add(1, Ordering::Relaxed);
    let entry = Arc::new(HostEntry {
        generation,
        to_host,
        channels: Mutex::new(HashMap::new()),
        cancel: CancellationToken::new(),
    });
    // A reconnecting host (same secret) replaces its old link.
    if let Some(old) = lock(&relay.hosts).insert(id.clone(), entry.clone()) {
        old.cancel.cancel();
    }
    tracing::info!(host = short(&id), "host connected");

    let mut ping = tokio::time::interval(PING_INTERVAL);
    let mut last_heard = Instant::now();
    loop {
        tokio::select! {
            _ = entry.cancel.cancelled() => break,
            msg = socket.recv() => {
                let Some(Ok(msg)) = msg else { break };
                last_heard = Instant::now();
                let Message::Binary(bytes) = msg else {
                    if matches!(msg, Message::Close(_)) { break }
                    continue;
                };
                if bytes.len() < 5 {
                    continue;
                }
                let channel = u32::from_be_bytes(bytes[1..5].try_into().expect("4 bytes"));
                match bytes[0] {
                    tunnel::link::DATA => {
                        let tx = lock(&entry.channels).get(&channel).cloned();
                        if let Some(tx) = tx {
                            if tx.try_send(bytes[5..].to_vec()).is_err() {
                                // The remote does not keep up (or is gone): drop it.
                                lock(&entry.channels).remove(&channel);
                                let _ = entry
                                    .to_host
                                    .try_send(link_message(tunnel::link::CLOSE, channel, &[]));
                            }
                        }
                    }
                    tunnel::link::CLOSE => {
                        lock(&entry.channels).remove(&channel);
                    }
                    _ => {}
                }
            }
            out = from_remotes.recv() => {
                let Some(out) = out else { break };
                if socket.send(Message::Binary(out.into())).await.is_err() {
                    break;
                }
            }
            _ = ping.tick() => {
                if last_heard.elapsed() > IDLE_TIMEOUT {
                    tracing::info!(host = short(&id), "host link timed out");
                    break;
                }
                if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }
    {
        let mut hosts = lock(&relay.hosts);
        if hosts.get(&id).is_some_and(|e| e.generation == generation) {
            hosts.remove(&id);
        }
    }
    // Dropping the senders ends every remote of this link.
    lock(&entry.channels).clear();
    let _ = socket.send(Message::Close(None)).await;
    tracing::info!(host = short(&id), "host disconnected");
}

async fn remote_handler(
    ws: WebSocketUpgrade,
    Path(host_id): Path<String>,
    State(relay): State<Arc<Relay>>,
) -> Response {
    ws.max_message_size(MAX_MESSAGE)
        .max_frame_size(MAX_MESSAGE)
        .on_upgrade(move |socket| run_remote(relay, host_id, socket))
}

async fn close_with(mut socket: WebSocket, code: u16, reason: &'static str) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}

/// Token bucket for remote messages; waits instead of disconnecting so uploads slow down.
struct Bucket {
    tokens: f64,
    rate: f64,
    last: Instant,
}

impl Bucket {
    fn new(rate: u32) -> Self {
        Bucket {
            tokens: f64::from(rate) * 2.0,
            rate: f64::from(rate),
            last: Instant::now(),
        }
    }

    /// How long to wait before the next message may pass.
    fn take(&mut self) -> Option<Duration> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.rate).min(self.rate * 2.0);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            return None;
        }
        let wait = (1.0 - self.tokens) / self.rate;
        self.tokens -= 1.0;
        Some(Duration::from_secs_f64(wait))
    }
}

async fn run_remote(relay: Arc<Relay>, host_id: String, mut socket: WebSocket) {
    let Some(host) = relay.host(&host_id) else {
        return close_with(socket, relay_close::HOST_OFFLINE, "host offline").await;
    };
    let channel = loop {
        let c = relay.next_channel.fetch_add(1, Ordering::Relaxed);
        if c != 0 {
            break c;
        }
    };
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(REMOTE_QUEUE);
    let admitted = {
        let mut channels = lock(&host.channels);
        let ok = channels.len() < relay.config.max_remotes_per_host;
        if ok {
            channels.insert(channel, tx);
        }
        ok
    };
    if !admitted {
        return close_with(socket, relay_close::HOST_FULL, "host full").await;
    }
    if host
        .to_host
        .send(link_message(tunnel::link::OPEN, channel, &[]))
        .await
        .is_err()
    {
        return close_with(socket, relay_close::HOST_OFFLINE, "host offline").await;
    }
    tracing::debug!(host = short(&host_id), channel, "remote connected");

    let mut bucket = Bucket::new(relay.config.remote_rate);
    let mut ping = tokio::time::interval(PING_INTERVAL);
    let mut last_heard = Instant::now();
    let close = loop {
        tokio::select! {
            msg = socket.recv() => {
                let Some(Ok(msg)) = msg else { break None };
                last_heard = Instant::now();
                match msg {
                    Message::Binary(bytes) => {
                        if let Some(wait) = bucket.take() {
                            tokio::time::sleep(wait).await;
                        }
                        let m = link_message(tunnel::link::DATA, channel, &bytes);
                        if host.to_host.send(m).await.is_err() {
                            break Some(relay_close::HOST_OFFLINE);
                        }
                    }
                    Message::Close(_) => break None,
                    _ => {}
                }
            }
            out = rx.recv() => {
                let Some(out) = out else {
                    // The host closed the channel, or its link is gone.
                    let online = relay
                        .host(&host_id)
                        .is_some_and(|h| h.generation == host.generation);
                    break Some(if online { relay_close::CLOSED_BY_HOST } else { relay_close::HOST_OFFLINE });
                };
                if socket.send(Message::Binary(out.into())).await.is_err() {
                    break None;
                }
            }
            _ = ping.tick() => {
                if last_heard.elapsed() > IDLE_TIMEOUT {
                    break None;
                }
                if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break None;
                }
            }
        }
    };
    let removed = lock(&host.channels).remove(&channel).is_some();
    if removed {
        let _ = host
            .to_host
            .try_send(link_message(tunnel::link::CLOSE, channel, &[]));
    }
    tracing::debug!(host = short(&host_id), channel, "remote disconnected");
    if let Some(code) = close {
        close_with(socket, code, "").await;
    }
}

async fn assets(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    // The remote page of a host; the host id is read by the page itself.
    let is_page = path.is_empty()
        || path == "index.html"
        || path
            .strip_prefix("r/")
            .is_some_and(|id| !id.trim_end_matches('/').contains('/'));
    let file_path = if is_page { "index.html" } else { path };
    let Some(file) = RemoteAssets::get(file_path) else {
        if is_page {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "web remote not built (pnpm build:remote)",
            )
                .into_response();
        }
        return StatusCode::NOT_FOUND.into_response();
    };
    let mime = mime_guess::from_path(file_path).first_or_octet_stream();
    let cache = if file_path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, mime.as_ref().to_owned()),
            (header::CACHE_CONTROL, cache.to_owned()),
            (header::CONTENT_SECURITY_POLICY, CSP.to_owned()),
        ],
        file.data,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_ids_are_stable_and_unguessable_lengths() {
        let a = host_id_for_secret(&[1; 32]);
        assert_eq!(a, host_id_for_secret(&[1; 32]));
        assert_ne!(a, host_id_for_secret(&[2; 32]));
        assert_eq!(a.len(), 22);
    }

    #[test]
    fn bucket_allows_bursts_then_paces() {
        let mut b = Bucket::new(10);
        for _ in 0..20 {
            assert!(b.take().is_none());
        }
        let wait = b.take().expect("must wait after the burst");
        assert!(wait <= Duration::from_millis(110), "{wait:?}");
    }

    #[test]
    fn tokens_compare_in_constant_time() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
