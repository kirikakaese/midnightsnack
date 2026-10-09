// SPDX-License-Identifier: GPL-3.0-or-later
//! The host's link to a relay. The host keeps one WebSocket to the relay; every remote on the
//! relay is a channel on it. Each channel is a Noise NK session (the host is the responder)
//! carrying the tunnel framing of [`crate::tunnel`]: the WebSocket session on stream 0 and HTTP
//! exchanges, answered by the host's own router with a synthetic, non-loopback address.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderName, HeaderValue, Method, Request, Uri};
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use midnightsnack_protocol::{
    tunnel, ConnectionPath, RelayError, RelaySettings, RelayState, NOISE_MAX_MESSAGE,
    RELAY_NOISE_PATTERN, RELAY_PROLOGUE, TUNNEL_MAX_PAYLOAD,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{self, Message as WsMessage};
use tower::ServiceExt;

use crate::state::{lock, AppState, Event};
use crate::tunnel::{split, Frame, Reassembler, RequestHead, ResponseHead};
use crate::ws::{self, ClientSocket, Incoming, Peer, SocketError};

const HOST_ID_CONTEXT: &[u8] = b"midnightsnack relay host id";
/// Remotes served at once through the relay.
const MAX_CHANNELS: usize = 64;
/// Concurrent HTTP exchanges per remote.
const MAX_STREAMS: usize = 32;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const RECONNECT_MIN: Duration = Duration::from_secs(1);
const RECONNECT_MAX: Duration = Duration::from_secs(30);
/// Queued Noise messages per channel before a channel that cannot keep up is dropped.
const CHANNEL_QUEUE: usize = 128;
/// Response headers passed through the tunnel.
const RESPONSE_HEADERS: &[&str] = &[
    "content-type",
    "content-length",
    "cache-control",
    "etag",
    "last-modified",
];

/// The host's keys on relays. The id is derived from the secret, so nobody without it can
/// claim the id; the static key authenticates the host to remotes.
#[derive(Clone, Serialize, Deserialize)]
pub struct RelayIdentity {
    #[serde(with = "b64_32")]
    private_key: [u8; 32],
    #[serde(with = "b64_32")]
    public_key: [u8; 32],
    #[serde(with = "b64_32")]
    secret: [u8; 32],
}

impl std::fmt::Debug for RelayIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayIdentity")
            .field("host_id", &self.host_id())
            .finish_non_exhaustive()
    }
}

mod b64_32 {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&B64.encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let s = String::deserialize(d)?;
        B64.decode(s)
            .ok()
            .and_then(|v| v.try_into().ok())
            .ok_or_else(|| serde::de::Error::custom("expected 32 bytes"))
    }
}

fn noise_params() -> snow::params::NoiseParams {
    RELAY_NOISE_PATTERN.parse().expect("valid noise pattern")
}

impl RelayIdentity {
    pub fn generate() -> Self {
        let keypair = snow::Builder::new(noise_params())
            .generate_keypair()
            .expect("key generation");
        let mut secret = [0u8; 32];
        rand::rng().fill_bytes(&mut secret);
        RelayIdentity {
            private_key: keypair.private.try_into().expect("32-byte key"),
            public_key: keypair.public.try_into().expect("32-byte key"),
            secret,
        }
    }

    /// Reads `relay-identity.json` from the data directory, creating it on first use.
    pub fn load_or_create(data_dir: Option<&Path>) -> Self {
        let Some(dir) = data_dir else {
            return Self::generate();
        };
        let path = dir.join("relay-identity.json");
        if let Some(id) = crate::util::read_json::<RelayIdentity>(&path) {
            return id;
        }
        let id = Self::generate();
        id.save(dir);
        id
    }

    pub fn save(&self, dir: &Path) {
        let path = dir.join("relay-identity.json");
        if let Err(e) = crate::util::write_secret_json(&path, self) {
            tracing::error!(error = %e, "failed to save relay identity");
        }
    }

    /// `base64url(SHA-256(context ‖ secret)[..16])` — what the relay computes from the secret.
    pub fn host_id(&self) -> String {
        host_id_for_secret(&self.secret)
    }

    pub fn public_key_b64(&self) -> String {
        B64.encode(self.public_key)
    }

    pub fn secret_b64(&self) -> String {
        B64.encode(self.secret)
    }

    pub fn private_key(&self) -> &[u8; 32] {
        &self.private_key
    }
}

/// The host id a relay assigns to a host secret.
pub fn host_id_for_secret(secret: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(HOST_ID_CONTEXT);
    h.update(secret);
    B64.encode(&h.finalize()[..16])
}

/// Relay channels are addressed from the IPv6 discard prefix `100::/64`: never loopback, so
/// every local-only rule refuses them, and distinct per channel for per-client lockouts.
pub fn relay_addr(channel: u32) -> SocketAddr {
    let ip = Ipv6Addr::new(0x100, 0, 0, 0, 0, 0, (channel >> 16) as u16, channel as u16);
    SocketAddr::new(IpAddr::V6(ip), 0)
}

pub fn is_relay_ip(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V6(v6) if v6.segments()[..4] == [0x100, 0, 0, 0])
}

/// The relay's base URL in canonical form (`https://host[:port][/path]`, no trailing slash).
pub fn normalize_url(url: &str) -> Result<String, RelayError> {
    let url = url.trim().trim_end_matches('/');
    let uri: Uri = url.parse().map_err(|_| RelayError::InvalidUrl)?;
    match uri.scheme_str() {
        Some("https") | Some("http") => {}
        _ => return Err(RelayError::InvalidUrl),
    }
    if uri.host().is_none_or(str::is_empty) || uri.query().is_some() {
        return Err(RelayError::InvalidUrl);
    }
    Ok(url.to_owned())
}

/// Keeps the relay link up while it is enabled, reconnecting with backoff.
pub async fn run(state: Arc<AppState>, router: Router) {
    let mut backoff = RECONNECT_MIN;
    loop {
        let settings = lock(&state.settings).relay.clone();
        if !settings.enabled || settings.url.trim().is_empty() {
            set_status(&state, RelayState::Off, None, 0);
            state.relay_restart.notified().await;
            continue;
        }
        set_status(&state, RelayState::Connecting, None, 0);
        let identity = lock(&state.relay_identity).clone();
        let attempt = tokio::select! {
            r = connect(&settings, &identity) => r,
            _ = state.relay_restart.notified() => continue,
        };
        match attempt {
            Ok(link) => {
                backoff = RECONNECT_MIN;
                tracing::info!(url = %settings.url, host_id = %identity.host_id(), "connected to relay");
                set_status(&state, RelayState::Connected, None, 0);
                tokio::select! {
                    _ = serve_link(&state, &router, &identity, link) => {
                        tracing::info!("relay link closed");
                    }
                    _ = state.relay_restart.notified() => continue,
                }
                set_status(&state, RelayState::Connecting, None, 0);
                // Reconnect quickly after a working link dropped.
                tokio::select! {
                    _ = tokio::time::sleep(RECONNECT_MIN) => {}
                    _ = state.relay_restart.notified() => {}
                }
            }
            Err(e) => {
                tracing::warn!(url = %settings.url, error = ?e, "cannot connect to relay");
                set_status(&state, RelayState::Failed, Some(e), 0);
                tokio::select! {
                    _ = tokio::time::sleep(backoff) => {}
                    _ = state.relay_restart.notified() => {}
                }
                backoff = (backoff * 2).min(RECONNECT_MAX);
            }
        }
    }
}

fn set_status(state: &AppState, s: RelayState, error: Option<RelayError>, remotes: u32) {
    let changed = {
        let mut r = lock(&state.relay_runtime);
        let changed = r.state != s || r.error != error || r.remotes != remotes;
        r.state = s;
        r.error = error;
        r.remotes = remotes;
        changed
    };
    if changed {
        state.emit(Event::Connectivity);
    }
}

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}
type Link = tokio_tungstenite::WebSocketStream<Box<dyn Io>>;

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = rustls::RootCertStore::empty();
            let native = rustls_native_certs::load_native_certs();
            for e in &native.errors {
                tracing::debug!(error = %e, "skipping a system certificate store");
            }
            let (added, ignored) = roots.add_parsable_certificates(native.certs);
            tracing::debug!(added, ignored, "loaded system root certificates");
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            Arc::new(
                rustls::ClientConfig::builder_with_provider(provider)
                    .with_safe_default_protocol_versions()
                    .expect("ring supports the default protocol versions")
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            )
        })
        .clone()
}

async fn connect(settings: &RelaySettings, identity: &RelayIdentity) -> Result<Link, RelayError> {
    let base = normalize_url(&settings.url)?;
    let secure = base.starts_with("https://");
    let ws_url = format!(
        "{}{}/relay/v1/host",
        if secure { "wss://" } else { "ws://" },
        base.split_once("://")
            .map(|(_, rest)| rest)
            .unwrap_or_default()
    );
    let mut request = ws_url
        .as_str()
        .into_client_request()
        .map_err(|_| RelayError::InvalidUrl)?;
    let headers = request.headers_mut();
    let bearer = format!("Bearer {}", identity.secret_b64());
    headers.insert(
        "authorization",
        HeaderValue::from_str(&bearer).map_err(|_| RelayError::InvalidUrl)?,
    );
    if let Some(token) = settings.access_token.as_deref().filter(|t| !t.is_empty()) {
        headers.insert(
            "x-relay-token",
            HeaderValue::from_str(token).map_err(|_| RelayError::Unauthorized)?,
        );
    }
    let uri = request.uri().clone();
    let host = uri.host().ok_or(RelayError::InvalidUrl)?.to_owned();
    let port = uri.port_u16().unwrap_or(if secure { 443 } else { 80 });
    let tcp = tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio::net::TcpStream::connect((host.as_str(), port)),
    )
    .await
    .map_err(|_| RelayError::Unreachable)?
    .map_err(|_| RelayError::Unreachable)?;
    let _ = tcp.set_nodelay(true);
    let io: Box<dyn Io> = if secure {
        let name =
            rustls::pki_types::ServerName::try_from(host.trim_matches(['[', ']']).to_owned())
                .map_err(|_| RelayError::InvalidUrl)?;
        let tls = tokio::time::timeout(
            CONNECT_TIMEOUT,
            tokio_rustls::TlsConnector::from(tls_config()).connect(name, tcp),
        )
        .await
        .map_err(|_| RelayError::Unreachable)?
        .map_err(|e| {
            tracing::warn!(error = %e, "TLS handshake with the relay failed");
            RelayError::Unreachable
        })?;
        Box::new(tls)
    } else {
        Box::new(tcp)
    };
    let config = tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(NOISE_MAX_MESSAGE + 16))
        .max_frame_size(Some(NOISE_MAX_MESSAGE + 16));
    let (link, _) = tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio_tungstenite::client_async_with_config(request, io, Some(config)),
    )
    .await
    .map_err(|_| RelayError::Unreachable)?
    .map_err(|e| match e {
        tungstenite::Error::Http(res) => match res.status().as_u16() {
            401 | 403 => RelayError::Unauthorized,
            429 | 503 => RelayError::Full,
            _ => RelayError::Incompatible,
        },
        _ => RelayError::Unreachable,
    })?;
    Ok(link)
}

fn link_message(op: u8, channel: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + payload.len());
    out.push(op);
    out.extend_from_slice(&channel.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

async fn serve_link(state: &Arc<AppState>, router: &Router, identity: &RelayIdentity, link: Link) {
    let (mut sink, mut stream) = link.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Vec<u8>>(512);
    let writer = async move {
        while let Some(m) = out_rx.recv().await {
            if sink.send(WsMessage::Binary(m.into())).await.is_err() {
                break;
            }
        }
    };
    let reader = async {
        let mut channels: HashMap<u32, mpsc::Sender<Vec<u8>>> = HashMap::new();
        while let Some(msg) = stream.next().await {
            let bytes = match msg {
                Ok(WsMessage::Binary(b)) => b,
                Ok(WsMessage::Close(_)) | Err(_) => break,
                Ok(_) => continue,
            };
            if bytes.len() < 5 {
                continue;
            }
            let op = bytes[0];
            let channel = u32::from_be_bytes(bytes[1..5].try_into().expect("4 bytes"));
            let payload = &bytes[5..];
            channels.retain(|_, tx| !tx.is_closed());
            match op {
                tunnel::link::OPEN => {
                    if channels.len() >= MAX_CHANNELS || channels.contains_key(&channel) {
                        let _ = out_tx.try_send(link_message(tunnel::link::CLOSE, channel, &[]));
                        continue;
                    }
                    let (tx, rx) = mpsc::channel(CHANNEL_QUEUE);
                    channels.insert(channel, tx);
                    tokio::spawn(channel_task(
                        state.clone(),
                        router.clone(),
                        channel,
                        rx,
                        out_tx.clone(),
                        identity.clone(),
                    ));
                }
                tunnel::link::DATA => {
                    if let Some(tx) = channels.get(&channel) {
                        if tx.try_send(payload.to_vec()).is_err() {
                            tracing::debug!(channel, "relay channel cannot keep up, closing");
                            channels.remove(&channel);
                            let _ =
                                out_tx.try_send(link_message(tunnel::link::CLOSE, channel, &[]));
                        }
                    }
                }
                tunnel::link::CLOSE => {
                    channels.remove(&channel);
                }
                _ => {}
            }
            set_status(state, RelayState::Connected, None, channels.len() as u32);
        }
    };
    tokio::select! {
        _ = writer => {}
        _ = reader => {}
    }
}

/// The WebSocket session (stream 0) of a relay channel.
struct TunnelSocket {
    rx: mpsc::Receiver<Incoming>,
    tx: mpsc::Sender<Frame>,
}

fn closed() -> SocketError {
    "relay channel closed".into()
}

impl ClientSocket for TunnelSocket {
    async fn recv(&mut self) -> Option<Result<Incoming, SocketError>> {
        self.rx.recv().await.map(Ok)
    }

    async fn send_text(&mut self, text: String) -> Result<(), SocketError> {
        for f in split(tunnel::WS_MESSAGE, 0, text.as_bytes(), true) {
            self.tx.send(f).await.map_err(|_| closed())?;
        }
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), SocketError> {
        self.tx
            .send(Frame::new(tunnel::PING, 0, true, Vec::new()))
            .await
            .map_err(|_| closed())
    }

    async fn close(&mut self) -> Result<(), SocketError> {
        // The channel task sends WS_CLOSE once the session ends.
        Ok(())
    }
}

/// Body chunks of a tunneled request.
type BodyTx = mpsc::Sender<Result<bytes::Bytes, std::io::Error>>;

struct Channel {
    state: Arc<AppState>,
    router: Router,
    addr: SocketAddr,
    /// Frames to encrypt and send (from the session and HTTP exchanges).
    plain_tx: mpsc::Sender<Frame>,
    ws: Option<mpsc::Sender<Incoming>>,
    ws_message: Reassembler,
    /// Open HTTP exchanges; `Some` while the request body is still arriving.
    streams: HashMap<u32, Option<BodyTx>>,
}

async fn channel_task(
    state: Arc<AppState>,
    router: Router,
    channel: u32,
    mut inbound: mpsc::Receiver<Vec<u8>>,
    link: mpsc::Sender<Vec<u8>>,
    identity: RelayIdentity,
) {
    let prologue = format!("{RELAY_PROLOGUE}{}", identity.host_id());
    let mut buf = vec![0u8; NOISE_MAX_MESSAGE];
    let mut out = vec![0u8; NOISE_MAX_MESSAGE];
    let handshake = async {
        let first = tokio::time::timeout(HANDSHAKE_TIMEOUT, inbound.recv())
            .await
            .ok()
            .flatten()?;
        let mut hs = snow::Builder::new(noise_params())
            .local_private_key(identity.private_key())
            .ok()?
            .prologue(prologue.as_bytes())
            .ok()?
            .build_responder()
            .ok()?;
        hs.read_message(&first, &mut buf).ok()?;
        let n = hs.write_message(&[], &mut out).ok()?;
        link.send(link_message(tunnel::link::DATA, channel, &out[..n]))
            .await
            .ok()?;
        hs.into_transport_mode().ok()
    };
    let Some(mut transport) = handshake.await else {
        tracing::debug!(channel, "relay channel handshake failed");
        let _ = link.try_send(link_message(tunnel::link::CLOSE, channel, &[]));
        return;
    };
    tracing::debug!(channel, "relay channel open");

    let (plain_tx, mut plain_rx) = mpsc::channel::<Frame>(64);
    let mut ch = Channel {
        state,
        router,
        addr: relay_addr(channel),
        plain_tx,
        ws: None,
        ws_message: Reassembler::default(),
        streams: HashMap::new(),
    };
    loop {
        let frame = tokio::select! {
            m = inbound.recv() => {
                let Some(m) = m else { break };
                let Ok(n) = transport.read_message(&m, &mut buf) else {
                    tracing::debug!(channel, "undecryptable relay message");
                    break;
                };
                let Some(frame) = Frame::decode(&buf[..n]) else { break };
                match ch.handle(frame) {
                    Some(reply) => reply,
                    None => continue,
                }
            }
            f = plain_rx.recv() => {
                let Some(f) = f else { break };
                ch.outgoing(&f);
                f
            }
        };
        let Ok(n) = transport.write_message(&frame.encode(), &mut out) else {
            break;
        };
        if link
            .send(link_message(tunnel::link::DATA, channel, &out[..n]))
            .await
            .is_err()
        {
            break;
        }
    }
    let _ = link.try_send(link_message(tunnel::link::CLOSE, channel, &[]));
    tracing::debug!(channel, "relay channel closed");
}

impl Channel {
    /// Bookkeeping for frames leaving the host.
    fn outgoing(&mut self, f: &Frame) {
        match f.kind {
            tunnel::WS_CLOSE => self.ws = None,
            tunnel::RESET => {
                self.streams.remove(&f.stream);
            }
            tunnel::RESPONSE_HEAD | tunnel::RESPONSE_BODY if f.fin => {
                self.streams.remove(&f.stream);
            }
            _ => {}
        }
    }

    /// Handles a frame from the remote; returns a frame to send back right away.
    fn handle(&mut self, f: Frame) -> Option<Frame> {
        match (f.kind, f.stream) {
            (tunnel::PING, _) => Some(Frame::new(tunnel::PONG, f.stream, true, f.payload)),
            (tunnel::PONG, 0) => {
                if let Some(ws) = &self.ws {
                    let _ = ws.try_send(Incoming::Pong);
                }
                None
            }
            (tunnel::WS_MESSAGE, 0) => {
                let text = match self.ws_message.push(&f.payload, f.fin, ws::MAX_MESSAGE) {
                    Ok(Some(m)) => String::from_utf8(m).ok()?,
                    Ok(None) => return None,
                    Err(()) => {
                        self.ws = None;
                        return Some(Frame::new(tunnel::WS_CLOSE, 0, true, Vec::new()));
                    }
                };
                let ws = self.ws.get_or_insert_with(|| {
                    let (tx, rx) = mpsc::channel(256);
                    let socket = TunnelSocket {
                        rx,
                        tx: self.plain_tx.clone(),
                    };
                    let peer = Peer {
                        addr: self.addr,
                        path: ConnectionPath::Relay,
                    };
                    let state = self.state.clone();
                    let done = self.plain_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = ws::run(socket, peer, state).await {
                            tracing::debug!(error = %e, "relay session ended");
                        }
                        let _ = done
                            .send(Frame::new(tunnel::WS_CLOSE, 0, true, Vec::new()))
                            .await;
                    });
                    tx
                });
                if ws.try_send(Incoming::Text(text)).is_err() {
                    self.ws = None;
                    return Some(Frame::new(tunnel::WS_CLOSE, 0, true, Vec::new()));
                }
                None
            }
            (tunnel::WS_CLOSE, 0) => {
                self.ws = None;
                None
            }
            (_, 0) => None,
            (tunnel::REQUEST_HEAD, stream) => {
                if self.streams.len() >= MAX_STREAMS || self.streams.contains_key(&stream) {
                    return Some(Frame::new(tunnel::RESET, stream, true, Vec::new()));
                }
                let Ok(head) = serde_json::from_slice::<RequestHead>(&f.payload) else {
                    return Some(Frame::new(tunnel::RESET, stream, true, Vec::new()));
                };
                let (body_tx, body) = if f.fin {
                    (None, Body::empty())
                } else {
                    let (tx, rx) = mpsc::channel(64);
                    (Some(tx), Body::from_stream(body_stream(rx)))
                };
                self.streams.insert(stream, body_tx);
                tokio::spawn(exchange(
                    self.router.clone(),
                    self.addr,
                    stream,
                    head,
                    body,
                    self.plain_tx.clone(),
                ));
                None
            }
            (tunnel::REQUEST_BODY, stream) => {
                let slot = self.streams.get_mut(&stream)?;
                let Some(tx) = slot else { return None };
                if tx.try_send(Ok(f.payload.into())).is_err() {
                    self.streams.remove(&stream);
                    return Some(Frame::new(tunnel::RESET, stream, true, Vec::new()));
                }
                if f.fin {
                    *slot = None;
                }
                None
            }
            (tunnel::RESET, stream) => {
                if let Some(Some(tx)) = self.streams.remove(&stream) {
                    let _ = tx.try_send(Err(std::io::Error::other("reset by remote")));
                }
                None
            }
            _ => None,
        }
    }
}

fn body_stream(
    rx: mpsc::Receiver<Result<bytes::Bytes, std::io::Error>>,
) -> impl futures_util::Stream<Item = Result<bytes::Bytes, std::io::Error>> {
    futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    })
}

/// Paths a remote may request through the relay. Video/audio files and capture streams are not
/// offered over the relay.
fn tunnel_allowed(path: &str) -> bool {
    let path = path.split('?').next().unwrap_or_default();
    path.starts_with("/api/v1/")
        && path != "/api/v1/ws"
        && !path.starts_with("/api/v1/media/file/")
        && !path.starts_with("/api/v1/media/capture/")
}

/// Answers one tunneled HTTP request with the host's router.
async fn exchange(
    router: Router,
    addr: SocketAddr,
    stream: u32,
    head: RequestHead,
    body: Body,
    out: mpsc::Sender<Frame>,
) {
    let response = if tunnel_allowed(&head.path) {
        match build_request(&head, body) {
            Some(mut req) => {
                req.extensions_mut().insert(ConnectInfo(addr));
                match router.oneshot(req).await {
                    Ok(r) => r,
                    Err(never) => match never {},
                }
            }
            None => status_response(400),
        }
    } else {
        status_response(404)
    };
    let (parts, body) = response.into_parts();
    let head = ResponseHead {
        status: parts.status.as_u16(),
        headers: parts
            .headers
            .iter()
            .filter(|(k, _)| RESPONSE_HEADERS.contains(&k.as_str()))
            .filter_map(|(k, v)| Some((k.as_str().to_owned(), v.to_str().ok()?.to_owned())))
            .collect(),
    };
    let head = serde_json::to_vec(&head).expect("response head serializes");
    if out
        .send(Frame::new(tunnel::RESPONSE_HEAD, stream, false, head))
        .await
        .is_err()
    {
        return;
    }
    let mut data = body.into_data_stream();
    let mut pending: Vec<u8> = Vec::new();
    while let Some(chunk) = data.next().await {
        let Ok(chunk) = chunk else {
            let _ = out
                .send(Frame::new(tunnel::RESET, stream, true, Vec::new()))
                .await;
            return;
        };
        pending.extend_from_slice(&chunk);
        while pending.len() >= TUNNEL_MAX_PAYLOAD {
            let rest = pending.split_off(TUNNEL_MAX_PAYLOAD);
            let full = std::mem::replace(&mut pending, rest);
            if out
                .send(Frame::new(tunnel::RESPONSE_BODY, stream, false, full))
                .await
                .is_err()
            {
                return;
            }
        }
    }
    let _ = out
        .send(Frame::new(tunnel::RESPONSE_BODY, stream, true, pending))
        .await;
}

fn build_request(head: &RequestHead, body: Body) -> Option<Request<Body>> {
    let method = Method::from_bytes(head.method.as_bytes()).ok()?;
    if method != Method::GET && method != Method::POST {
        return None;
    }
    let uri: Uri = head.path.parse().ok()?;
    let mut req = Request::builder().method(method).uri(uri);
    for (k, v) in &head.headers {
        let name = HeaderName::from_bytes(k.as_bytes()).ok()?;
        // Hop-by-hop and host headers make no sense in the tunnel.
        if matches!(
            name.as_str(),
            "host" | "connection" | "upgrade" | "transfer-encoding"
        ) {
            continue;
        }
        req = req.header(name, HeaderValue::from_str(v).ok()?);
    }
    req.body(body).ok()
}

fn status_response(status: u16) -> axum::response::Response {
    let mut r = axum::response::Response::new(Body::empty());
    *r.status_mut() = axum::http::StatusCode::from_u16(status).expect("valid status");
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_addresses_are_never_local() {
        let a = relay_addr(0x0001_0002);
        assert!(!a.ip().is_loopback());
        assert!(is_relay_ip(a.ip()));
        assert_ne!(relay_addr(1), relay_addr(2));
        assert!(!is_relay_ip("192.168.1.2".parse().unwrap()));
        assert!(!is_relay_ip("::1".parse().unwrap()));
    }

    #[test]
    fn host_id_is_derived_from_the_secret() {
        let id = RelayIdentity::generate();
        assert_eq!(id.host_id(), host_id_for_secret(&id.secret));
        assert_eq!(id.host_id().len(), 22);
        assert_ne!(id.host_id(), RelayIdentity::generate().host_id());
    }

    #[test]
    fn identity_persists_with_private_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let a = RelayIdentity::load_or_create(Some(dir.path()));
        let b = RelayIdentity::load_or_create(Some(dir.path()));
        assert_eq!(a.host_id(), b.host_id());
        assert_eq!(a.public_key_b64(), b.public_key_b64());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join("relay-identity.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "identity must not be readable by others");
        }
    }

    #[test]
    fn urls_are_validated() {
        assert_eq!(
            normalize_url(" https://relay.example.org/ ").unwrap(),
            "https://relay.example.org"
        );
        assert_eq!(
            normalize_url("http://127.0.0.1:8080/sub").unwrap(),
            "http://127.0.0.1:8080/sub"
        );
        for bad in [
            "",
            "relay.example.org",
            "ftp://x",
            "https://",
            "https://x/?a=1",
        ] {
            assert_eq!(normalize_url(bad), Err(RelayError::InvalidUrl), "{bad}");
        }
    }

    #[test]
    fn only_api_paths_go_through_the_tunnel() {
        assert!(tunnel_allowed("/api/v1/pair"));
        assert!(tunnel_allowed("/api/v1/media/slide/c/0?k=x"));
        assert!(tunnel_allowed("/api/v1/upload?name=a.pdf"));
        assert!(!tunnel_allowed("/api/v1/ws"));
        assert!(!tunnel_allowed("/api/v1/media/file/c?k=x"));
        assert!(!tunnel_allowed("/api/v1/media/capture/c"));
        assert!(!tunnel_allowed("/index.html"));
    }
}
