// SPDX-License-Identifier: GPL-3.0-or-later
//! WebSocket connections: authentication, state fan-out and action requests.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::response::Response;
use midnightsnack_core::{now_ms, Origin};
use midnightsnack_protocol::{
    ClientMessage, ErrorCode, Role, ServerMessage, SessionInfo, PROTOCOL_VERSION,
};
use midnightsnack_render::TargetSize;
use tokio::sync::broadcast::error::RecvError;

use crate::devices::Device;
use crate::state::{lock, AppState, Event, PointerUpdate};
use crate::util::random_token;

const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(5);
const MAX_MESSAGE: usize = 1 << 20;
/// Pointer updates accepted per device and second (a finger reports at up to 60 Hz).
const POINTER_RATE: u32 = 60;

pub async fn handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.max_message_size(MAX_MESSAGE)
        .on_upgrade(move |socket| async move {
            if let Err(e) = run(socket, addr, state).await {
                tracing::debug!(%addr, error = %e, "connection closed");
            }
        })
}

type WsResult = Result<(), axum::Error>;

async fn send(socket: &mut WebSocket, msg: &ServerMessage) -> WsResult {
    let text = serde_json::to_string(msg).expect("server messages serialize");
    socket.send(Message::Text(text.into())).await
}

async fn reject(mut socket: WebSocket, code: ErrorCode) -> WsResult {
    send(&mut socket, &ServerMessage::Error { code }).await?;
    socket.send(Message::Close(None)).await
}

/// Waits for `hello` and authenticates it.
async fn handshake(
    socket: &mut WebSocket,
    addr: SocketAddr,
    state: &AppState,
) -> Result<Device, ErrorCode> {
    let first = tokio::time::timeout(HELLO_TIMEOUT, socket.recv())
        .await
        .map_err(|_| ErrorCode::Unauthorized)?;
    let Some(Ok(Message::Text(text))) = first else {
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
    // Host-window tokens are only valid on the host itself.
    if device.local && !addr.ip().is_loopback() {
        tracing::warn!(%addr, "local token used from remote address");
        return Err(ErrorCode::Unauthorized);
    }
    Ok(device)
}

struct Conn {
    device_id: String,
    local: bool,
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

async fn run(mut socket: WebSocket, addr: SocketAddr, state: Arc<AppState>) -> WsResult {
    // Subscribe before the handshake so no event between snapshot and loop is lost.
    let mut events = state.events.subscribe();
    let device = match handshake(&mut socket, addr, &state).await {
        Ok(d) => d,
        Err(code) => return reject(socket, code).await,
    };
    let conn = Conn {
        device_id: device.id.clone(),
        local: device.local,
        media_key: random_token(),
        pointer_window: std::sync::Mutex::new((Instant::now(), 0)),
    };
    lock(&state.media_keys).insert(conn.media_key.clone(), conn.device_id.clone());
    lock(&state.devices).mark_connected(&conn.device_id, 1);
    state.emit(Event::Devices);
    tracing::info!(%addr, device = %device.name, role = ?device.role, "client connected");

    let result = session(&mut socket, &state, &conn, &mut events).await;

    lock(&state.media_keys).remove(&conn.media_key);
    lock(&state.devices).mark_connected(&conn.device_id, -1);
    state.emit(Event::Devices);
    tracing::info!(%addr, device = %device.name, "client disconnected");
    result
}

async fn send_full_state(socket: &mut WebSocket, state: &AppState, role: Role) -> WsResult {
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
    }
    let queued = *state.render.subscribe_queued().borrow();
    send(socket, &ServerMessage::RenderProgress { queued }).await
}

async fn send_inbox(socket: &mut WebSocket, state: &AppState) -> WsResult {
    let items = lock(&state.inbox).items();
    let auto_accept = lock(&state.settings).auto_accept_uploads;
    send(socket, &ServerMessage::Inbox { items, auto_accept }).await
}

async fn send_devices(socket: &mut WebSocket, state: &AppState) -> WsResult {
    let devices = lock(&state.devices).list();
    let pending = lock(&state.pairing).pending();
    send(socket, &ServerMessage::Devices { devices, pending }).await
}

async fn session(
    socket: &mut WebSocket,
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
    send_full_state(socket, state, role).await?;

    let mut ping = tokio::time::interval(PING_INTERVAL);
    let mut ping_sent: Option<Instant> = None;
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(msg) = incoming else { return Ok(()) };
                match msg? {
                    Message::Text(text) => {
                        let reply = handle_message(state, conn, &text).await;
                        if let Some(reply) = reply {
                            send(socket, &reply).await?;
                        }
                    }
                    Message::Pong(_) => {
                        if let Some(sent) = ping_sent.take() {
                            let ms = sent.elapsed().as_millis().min(u32::MAX as u128) as u32;
                            lock(&state.devices).set_latency(&conn.device_id, ms);
                        }
                    }
                    Message::Close(_) => return Ok(()),
                    Message::Binary(_) => {
                        send(socket, &ServerMessage::Error { code: ErrorCode::MalformedMessage }).await?;
                    }
                    Message::Ping(_) => {}
                }
            }
            event = events.recv() => {
                let Some(role) = conn.role(state) else {
                    // Revoked.
                    return socket.send(Message::Close(None)).await;
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
                    Ok(Event::Pairing) if role == Role::Admin => {
                        send(socket, &ServerMessage::Pairing { pairing: state.pairing_info() }).await?;
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
                            return socket.send(Message::Close(None)).await;
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
                            send_full_state(socket, state, role).await?;
                        }
                    }
                    Ok(_) => {}
                    Err(RecvError::Lagged(_)) => send_full_state(socket, state, role).await?,
                    Err(RecvError::Closed) => return Ok(()),
                }
            }
            _ = ping.tick() => {
                ping_sent = Some(Instant::now());
                socket.send(Message::Ping(Vec::new().into())).await?;
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
