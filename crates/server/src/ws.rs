// SPDX-License-Identifier: GPL-3.0-or-later
//! WebSocket connections: authentication, state fan-out and action requests. The session runs
//! over [`ClientSocket`], so real WebSockets and the relay tunnel share the same code.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::response::Response;
use axum::Extension;
use midnightsnack_core::{now_ms, Origin};
use midnightsnack_protocol::{
    ClientMessage, ConnectionPath, ErrorCode, Role, ServerMessage, SessionInfo, PROTOCOL_VERSION,
};
use midnightsnack_render::TargetSize;
use tokio::sync::broadcast::error::RecvError;

use crate::devices::Device;
use crate::state::{lock, AppState, Event, PointerUpdate};
use crate::util::random_token;

const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(5);
pub(crate) const MAX_MESSAGE: usize = 1 << 20;
/// Pointer updates accepted per device and second (a finger reports at up to 60 Hz).
const POINTER_RATE: u32 = 60;

/// Marks requests that arrived over the HTTPS listener.
#[derive(Debug, Clone, Copy)]
pub struct Secure;

pub async fn handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    secure: Option<Extension<Secure>>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let path = if addr.ip().is_loopback() {
        ConnectionPath::Local
    } else if secure.is_some() {
        ConnectionPath::Https
    } else {
        ConnectionPath::Lan
    };
    ws.max_message_size(MAX_MESSAGE)
        .on_upgrade(move |socket| async move {
            let peer = Peer { addr, path };
            if let Err(e) = run(socket, peer, state).await {
                tracing::debug!(%addr, error = %e, "connection closed");
            }
        })
}

pub(crate) type SocketError = Box<dyn std::error::Error + Send + Sync>;
type WsResult = Result<(), SocketError>;

/// What a session receives from its client.
pub(crate) enum Incoming {
    Text(String),
    Pong,
    Close,
    /// Anything the protocol does not use (binary messages).
    Other,
    /// Transport-level traffic (pings), answered by the transport itself.
    Ignored,
}

/// A client connection carrying protocol messages.
pub(crate) trait ClientSocket: Send {
    fn recv(&mut self) -> impl Future<Output = Option<Result<Incoming, SocketError>>> + Send;
    fn send_text(&mut self, text: String) -> impl Future<Output = WsResult> + Send;
    fn ping(&mut self) -> impl Future<Output = WsResult> + Send;
    fn close(&mut self) -> impl Future<Output = WsResult> + Send;
}

impl ClientSocket for WebSocket {
    async fn recv(&mut self) -> Option<Result<Incoming, SocketError>> {
        let msg = WebSocket::recv(self).await?;
        Some(
            msg.map(|m| match m {
                Message::Text(t) => Incoming::Text(t.to_string()),
                Message::Pong(_) => Incoming::Pong,
                Message::Close(_) => Incoming::Close,
                Message::Binary(_) => Incoming::Other,
                Message::Ping(_) => Incoming::Ignored,
            })
            .map_err(Into::into),
        )
    }

    async fn send_text(&mut self, text: String) -> WsResult {
        Ok(self.send(Message::Text(text.into())).await?)
    }

    async fn ping(&mut self) -> WsResult {
        Ok(self.send(Message::Ping(Vec::new().into())).await?)
    }

    async fn close(&mut self) -> WsResult {
        Ok(self.send(Message::Close(None)).await?)
    }
}

/// Where a connection comes from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Peer {
    /// Network address; a synthetic one for relay channels (see `relay_link`).
    pub addr: SocketAddr,
    pub path: ConnectionPath,
}

async fn send<S: ClientSocket>(socket: &mut S, msg: &ServerMessage) -> WsResult {
    let text = serde_json::to_string(msg).expect("server messages serialize");
    socket.send_text(text).await
}

async fn reject<S: ClientSocket>(mut socket: S, code: ErrorCode) -> WsResult {
    send(&mut socket, &ServerMessage::Error { code }).await?;
    socket.close().await
}

/// Waits for `hello` and authenticates it.
async fn handshake<S: ClientSocket>(
    socket: &mut S,
    addr: SocketAddr,
    state: &AppState,
) -> Result<Device, ErrorCode> {
    let first = tokio::time::timeout(HELLO_TIMEOUT, socket.recv())
        .await
        .map_err(|_| ErrorCode::Unauthorized)?;
    let Some(Ok(Incoming::Text(text))) = first else {
        return Err(ErrorCode::MalformedMessage);
    };
    let Ok(ClientMessage::Hello {
        protocol_version,
        token,
    }) = serde_json::from_str(&text)
    else {
        return Err(ErrorCode::MalformedMessage);
    };
    if protocol_version != PROTOCOL_VERSION {
        return Err(ErrorCode::ProtocolMismatch);
    }
    let device = lock(&state.devices)
        .authenticate(&token)
        .ok_or(ErrorCode::Unauthorized)?;
    // Host-window tokens (and API keys, if restricted) are only valid on the host itself.
    if !state.device_allowed_from(&device, addr) {
        tracing::warn!(%addr, device = %device.name, "token not allowed from this address");
        return Err(ErrorCode::Unauthorized);
    }
    Ok(device)
}

struct Conn {
    device_id: String,
    local: bool,
    api_key: bool,
    loopback: bool,
    media_key: String,
    /// Start of the current one-second window and pointer updates in it.
    pointer_window: std::sync::Mutex<(Instant, u32)>,
}

impl Conn {
    fn pointer_allowed(&self) -> bool {
        let mut w = self
            .pointer_window
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if w.0.elapsed() >= Duration::from_secs(1) {
            *w = (Instant::now(), 0);
        }
        w.1 += 1;
        w.1 <= POINTER_RATE
    }

    /// Paired remotes learn the host's other routes; host windows and API keys do not need them.
    fn wants_routes(&self) -> bool {
        !self.local && !self.api_key
    }

    fn role(&self, state: &AppState) -> Option<Role> {
        lock(&state.devices).get(&self.device_id).map(|d| d.role)
    }

    fn session(&self, state: &AppState) -> Option<SessionInfo> {
        let d = lock(&state.devices).get(&self.device_id)?;
        Some(SessionInfo {
            device_id: d.id,
            device_name: d.name,
            role: d.role,
            local: self.local,
            media_key: self.media_key.clone(),
        })
    }
}

pub(crate) async fn run<S: ClientSocket>(
    mut socket: S,
    peer: Peer,
    state: Arc<AppState>,
) -> WsResult {
    let addr = peer.addr;
    // Subscribe before the handshake so no event between snapshot and loop is lost.
    let mut events = state.events.subscribe();
    let device = match handshake(&mut socket, addr, &state).await {
        Ok(d) => d,
        Err(code) => return reject(socket, code).await,
    };
    let conn = Conn {
        device_id: device.id.clone(),
        local: device.local,
        api_key: device.api_key,
        loopback: addr.ip().is_loopback(),
        media_key: random_token(),
        pointer_window: std::sync::Mutex::new((Instant::now(), 0)),
    };
    lock(&state.media_keys).insert(conn.media_key.clone(), conn.device_id.clone());
    {
        let mut devices = lock(&state.devices);
        devices.mark_connected(&conn.device_id, 1);
        let address = matches!(peer.path, ConnectionPath::Lan | ConnectionPath::Https)
            .then(|| addr.ip().to_string());
        devices.set_path(&conn.device_id, peer.path, address);
    }
    state.emit(Event::Devices);
    tracing::info!(%addr, device = %device.name, role = ?device.role, path = ?peer.path, "client connected");

    let result = session(&mut socket, &state, &conn, &mut events).await;

    lock(&state.media_keys).remove(&conn.media_key);
    lock(&state.devices).mark_connected(&conn.device_id, -1);
    state.emit(Event::Devices);
    tracing::info!(%addr, device = %device.name, "client disconnected");
    result
}

async fn send_full_state<S: ClientSocket>(
    socket: &mut S,
    state: &AppState,
    conn: &Conn,
    role: Role,
) -> WsResult {
    send(
        socket,
        &ServerMessage::Show {
            show: state.show_snapshot(role),
        },
    )
    .await?;
    let live = lock(&state.engine).live_state(now_ms());
    send(socket, &ServerMessage::Live { live }).await?;
    if role == Role::Admin {
        send_devices(socket, state).await?;
        send_inbox(socket, state).await?;
        send(
            socket,
            &ServerMessage::Pairing {
                pairing: state.pairing_info(),
            },
        )
        .await?;
        send(
            socket,
            &ServerMessage::Connectivity {
                connectivity: state.connectivity(),
            },
        )
        .await?;
    }
    if conn.wants_routes() {
        send(
            socket,
            &ServerMessage::Routes {
                routes: state.routes(),
            },
        )
        .await?;
    }
    send_openslides(socket, state, role).await?;
    let queued = *state.render.subscribe_queued().borrow();
    send(socket, &ServerMessage::RenderProgress { queued }).await
}

async fn send_openslides<S: ClientSocket>(
    socket: &mut S,
    state: &AppState,
    role: Role,
) -> WsResult {
    let data = lock(&state.openslides_data).as_deref().cloned();
    send(socket, &ServerMessage::OpenSlides { data }).await?;
    if role == Role::Admin {
        send(
            socket,
            &ServerMessage::OpenSlidesStatus {
                status: state.openslides_status(),
            },
        )
        .await?;
    }
    Ok(())
}

async fn send_inbox<S: ClientSocket>(socket: &mut S, state: &AppState) -> WsResult {
    let items = lock(&state.inbox).items();
    let auto_accept = lock(&state.settings).auto_accept_uploads;
    send(socket, &ServerMessage::Inbox { items, auto_accept }).await
}

async fn send_devices<S: ClientSocket>(socket: &mut S, state: &AppState) -> WsResult {
    let devices = lock(&state.devices).list();
    let pending = lock(&state.pairing).pending();
    let control = state.control_settings();
    send(
        socket,
        &ServerMessage::Devices {
            devices,
            pending,
            control,
        },
    )
    .await
}

async fn session<S: ClientSocket>(
    socket: &mut S,
    state: &Arc<AppState>,
    conn: &Conn,
    events: &mut tokio::sync::broadcast::Receiver<Event>,
) -> WsResult {
    let Some(session) = conn.session(state) else {
        return Ok(());
    };
    let role = session.role;
    send(
        socket,
        &ServerMessage::Welcome {
            host: state.host.clone(),
            session,
        },
    )
    .await?;
    send_full_state(socket, state, conn, role).await?;

    let mut ping = tokio::time::interval(PING_INTERVAL);
    let mut ping_sent: Option<Instant> = None;
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(msg) = incoming else { return Ok(()) };
                match msg? {
                    Incoming::Text(text) => {
                        let reply = handle_message(state, conn, &text).await;
                        if let Some(reply) = reply {
                            send(socket, &reply).await?;
                        }
                    }
                    Incoming::Pong => {
                        if let Some(sent) = ping_sent.take() {
                            let ms = sent.elapsed().as_millis().min(u32::MAX as u128) as u32;
                            lock(&state.devices).set_latency(&conn.device_id, ms);
                        }
                    }
                    Incoming::Close => return Ok(()),
                    Incoming::Ignored => {}
                    Incoming::Other => {
                        send(socket, &ServerMessage::Error { code: ErrorCode::MalformedMessage }).await?;
                    }
                }
            }
            event = events.recv() => {
                let Some(role) = conn.role(state) else {
                    // Revoked.
                    return socket.close().await;
                };
                match event {
                    Ok(Event::Show) => {
                        send(socket, &ServerMessage::Show { show: state.show_snapshot(role) }).await?;
                    }
                    Ok(Event::Live) => {
                        let live = lock(&state.engine).live_state(now_ms());
                        send(socket, &ServerMessage::Live { live }).await?;
                    }
                    Ok(Event::Devices) if role == Role::Admin => send_devices(socket, state).await?,
                    Ok(Event::Inbox) if role == Role::Admin => send_inbox(socket, state).await?,
                    Ok(Event::ApiLocalOnly) if conn.api_key && !conn.loopback => {
                        send(socket, &ServerMessage::Error { code: ErrorCode::Unauthorized }).await?;
                        return socket.close().await;
                    }
                    Ok(Event::Pairing) if role == Role::Admin => {
                        send(socket, &ServerMessage::Pairing { pairing: state.pairing_info() }).await?;
                    }
                    Ok(Event::OpenSlides) => {
                        let data = lock(&state.openslides_data).as_deref().cloned();
                        send(socket, &ServerMessage::OpenSlides { data }).await?;
                    }
                    Ok(Event::OpenSlidesStatus) if role == Role::Admin => {
                        send(socket, &ServerMessage::OpenSlidesStatus { status: state.openslides_status() }).await?;
                    }
                    Ok(Event::Connectivity) => {
                        if role == Role::Admin {
                            send(socket, &ServerMessage::Pairing { pairing: state.pairing_info() }).await?;
                            send(socket, &ServerMessage::Connectivity { connectivity: state.connectivity() }).await?;
                        }
                        if conn.wants_routes() {
                            send(socket, &ServerMessage::Routes { routes: state.routes() }).await?;
                        }
                    }
                    Ok(Event::RenderProgress(queued)) => {
                        send(socket, &ServerMessage::RenderProgress { queued }).await?;
                    }
                    Ok(Event::Kick(target)) => {
                        let me = match &target {
                            Some(id) => *id == conn.device_id,
                            None => !conn.local,
                        };
                        if me {
                            send(socket, &ServerMessage::Error { code: ErrorCode::Unauthorized }).await?;
                            return socket.close().await;
                        }
                    }
                    Ok(Event::Pointer(p)) if p.device_id != conn.device_id => {
                        send(socket, &ServerMessage::Pointer {
                            device_id: p.device_id.clone(),
                            pos: p.pos,
                            mode: p.mode,
                            color: p.color.clone(),
                        }).await?;
                    }
                    Ok(Event::Session(id)) if id == conn.device_id => {
                        if let Some(session) = conn.session(state) {
                            send(socket, &ServerMessage::Session { session }).await?;
                            // Role-dependent data may now differ.
                            send_full_state(socket, state, conn, role).await?;
                        }
                    }
                    Ok(_) => {}
                    Err(RecvError::Lagged(_)) => send_full_state(socket, state, conn, role).await?,
                    Err(RecvError::Closed) => return Ok(()),
                }
            }
            _ = ping.tick() => {
                ping_sent = Some(Instant::now());
                socket.ping().await?;
            }
        }
    }
}

async fn handle_message(state: &Arc<AppState>, conn: &Conn, text: &str) -> Option<ServerMessage> {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(_) => {
            return Some(ServerMessage::Error {
                code: ErrorCode::MalformedMessage,
            })
        }
    };
    match msg {
        ClientMessage::Hello { .. } => None,
        ClientMessage::Ping { nonce } => Some(ServerMessage::Pong { nonce }),
        ClientMessage::ListCaptureTargets => {
            if conn.role(state) != Some(Role::Admin) {
                return Some(ServerMessage::Error {
                    code: ErrorCode::Forbidden,
                });
            }
            let result = tokio::task::spawn_blocking(midnightsnack_capture::list_targets).await;
            Some(match result {
                Ok(Ok(targets)) => ServerMessage::CaptureTargets { targets },
                Ok(Err(midnightsnack_capture::CaptureError::Permission)) => ServerMessage::Error {
                    code: ErrorCode::CapturePermission,
                },
                _ => ServerMessage::CaptureTargets {
                    targets: Vec::new(),
                },
            })
        }
        ClientMessage::CreateApiKey { name, role } => {
            if conn.role(state) != Some(Role::Admin) {
                return Some(ServerMessage::Error {
                    code: ErrorCode::Forbidden,
                });
            }
            let name: String = name.trim().chars().take(60).collect();
            if name.is_empty() {
                return Some(ServerMessage::Error {
                    code: ErrorCode::InvalidState,
                });
            }
            let (device_id, token) = lock(&state.devices).add_api_key(&name, role);
            state.emit(Event::Devices);
            Some(ServerMessage::ApiKey {
                device_id,
                name,
                token,
            })
        }
        ClientMessage::Pointer { pos, mode, color } => {
            if conn.role(state).is_none_or(|r| r < Role::Presenter) {
                return Some(ServerMessage::Error {
                    code: ErrorCode::Forbidden,
                });
            }
            let valid = midnightsnack_core::model::is_valid_color(&color)
                && pos.is_none_or(|p| p.iter().all(|v| v.is_finite()));
            if !valid {
                return Some(ServerMessage::Error {
                    code: ErrorCode::MalformedMessage,
                });
            }
            // Hiding is always delivered; movement beyond the rate is dropped.
            if pos.is_some() && !conn.pointer_allowed() {
                return None;
            }
            state.emit(Event::Pointer(Arc::new(PointerUpdate {
                device_id: conn.device_id.clone(),
                pos: pos.map(|[x, y]| [x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)]),
                mode,
                color,
            })));
            None
        }
        ClientMessage::Viewport { width, height } => {
            // Only host output windows decide the render resolution.
            if conn.local && width > 0 && height > 0 {
                let size = TargetSize::new(width, height);
                let changed = {
                    let mut s = lock(&state.output_size);
                    let changed = *s != size;
                    *s = size;
                    changed
                };
                if changed {
                    state.prefetch();
                }
            }
            None
        }
        ClientMessage::Action { request_id, action } => {
            let error = match conn.role(state) {
                None => Some(ErrorCode::Unauthorized),
                Some(role) => {
                    let origin = Origin {
                        role,
                        local: conn.local,
                    };
                    state.perform(origin, action).await.err()
                }
            };
            Some(ServerMessage::ActionResult { request_id, error })
        }
    }
}
