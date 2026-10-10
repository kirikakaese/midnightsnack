// SPDX-License-Identifier: GPL-3.0-or-later
//! OSC over UDP. Bound to this computer only while API keys are restricted to it; otherwise
//! on all interfaces, where a sender must first `/midnightsnack/auth <token>` with a device
//! token or API key. Subscribers get `/midnightsnack/state/...` feedback on every change.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use midnightsnack_control::osc::{self, Command, OscError};
use midnightsnack_core::{now_ms, Origin};
use midnightsnack_protocol::{Action, Position, Role, StateSummary};
use tokio::net::UdpSocket;

use crate::state::{lock, AppState, Event};

/// Authenticated senders are forgotten after this long without messages.
const SESSION_IDLE: Duration = Duration::from_secs(12 * 3600);
/// Subscriptions end unless renewed.
const SUBSCRIPTION: Duration = Duration::from_secs(3600);
/// Failed `/auth` attempts per address before it is ignored for a minute.
const MAX_AUTH_FAILURES: u32 = 5;

#[derive(Default)]
struct Sessions {
    /// Sender address → device id and last activity. The device is looked up on every
    /// message, so revoking it or changing its role applies at once.
    authed: HashMap<SocketAddr, (String, Instant)>,
    /// Feedback target → expiry.
    subscribers: HashMap<SocketAddr, Instant>,
    /// Address → (failures, window start).
    failures: HashMap<IpAddr, (u32, Instant)>,
}

/// Runs for the lifetime of the server, (re)binding when the OSC settings change.
pub async fn run(state: Arc<AppState>) {
    loop {
        let (settings, local_only) = {
            let s = lock(&state.settings);
            (s.osc, s.api_local_only)
        };
        let socket = if settings.enabled {
            let ip: IpAddr = if local_only {
                [127, 0, 0, 1].into()
            } else {
                [0, 0, 0, 0].into()
            };
            match UdpSocket::bind(SocketAddr::new(ip, settings.port)).await {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!(port = settings.port, error = %e, "cannot start OSC");
                    None
                }
            }
        } else {
            None
        };
        let port = socket
            .as_ref()
            .and_then(|s| s.local_addr().ok())
            .map(|a| a.port());
        *lock(&state.osc_listening) = port;
        state.emit(Event::Devices);
        match socket {
            Some(socket) => {
                tracing::info!(?port, local_only, "OSC listening");
                tokio::select! {
                    _ = serve(&state, socket) => {}
                    _ = state.osc_restart.notified() => {}
                }
            }
            None => state.osc_restart.notified().await,
        }
    }
}

async fn serve(state: &Arc<AppState>, socket: UdpSocket) {
    let mut sessions = Sessions::default();
    let mut events = state.events.subscribe();
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let mut last_sent: Option<StateSummary> = None;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        tokio::select! {
            received = socket.recv_from(&mut buf) => {
                let Ok((n, from)) = received else { continue };
                let subscribed = handle_datagram(state, &mut sessions, &buf[..n], from).await;
                if subscribed {
                    // New subscribers get the full state at once.
                    last_sent = None;
                }
            }
            event = events.recv() => {
                if !matches!(event, Ok(Event::Live | Event::Show) | Err(_)) {
                    continue;
                }
            }
            _ = tick.tick() => {}
        }
        if sessions.subscribers.is_empty() {
            continue;
        }
        // Timers change every second; everything else on events.
        let summary = lock(&state.engine).summary(now_ms());
        if last_sent.as_ref() == Some(&summary) {
            continue;
        }
        let now = Instant::now();
        sessions.subscribers.retain(|_, expiry| *expiry > now);
        let bytes = osc::feedback(&summary);
        for target in sessions.subscribers.keys() {
            let _ = socket.send_to(&bytes, target).await;
        }
        last_sent = Some(summary);
    }
}

/// Returns whether a subscription was added.
async fn handle_datagram(
    state: &Arc<AppState>,
    sessions: &mut Sessions,
    datagram: &[u8],
    from: SocketAddr,
) -> bool {
    let Ok(commands) = osc::decode(datagram) else {
        return false;
    };
    let mut subscribed = false;
    for command in commands {
        let command = match command {
            Ok(c) => c,
            Err(OscError::UnknownAddress(a)) => {
                tracing::debug!(%from, address = %a, "unknown OSC address");
                continue;
            }
            Err(e) => {
                tracing::debug!(%from, ?e, "bad OSC message");
                continue;
            }
        };
        if let Command::Auth(token) = &command {
            authenticate(state, sessions, from, token);
            continue;
        }
        // Senders on this computer act as operators; others must authenticate.
        let role = if from.ip().is_loopback() {
            Some(Role::Operator)
        } else {
            session_role(state, sessions, from)
        };
        let Some(role) = role else {
            tracing::debug!(%from, "OSC from unauthenticated sender ignored");
            continue;
        };
        match command {
            Command::Subscribe(port) => {
                let target = SocketAddr::new(from.ip(), port.unwrap_or(from.port()));
                sessions
                    .subscribers
                    .insert(target, Instant::now() + SUBSCRIPTION);
                subscribed = true;
            }
            Command::Unsubscribe => {
                sessions.subscribers.retain(|t, _| t.ip() != from.ip());
            }
            other => {
                let Some(action) = to_action(state, other) else {
                    continue;
                };
                let origin = Origin { role, local: false };
                if let Err(e) = state.perform(origin, action).await {
                    tracing::debug!(%from, ?e, "OSC action refused");
                }
            }
        }
    }
    subscribed
}

/// The current role of the device a sender authenticated as; ends the session when the
/// device is gone (revoked, forgotten, disconnected) or it went idle.
fn session_role(state: &AppState, sessions: &mut Sessions, from: SocketAddr) -> Option<Role> {
    let (id, seen) = sessions.authed.get_mut(&from)?;
    let device = (seen.elapsed() < SESSION_IDLE)
        .then(|| lock(&state.devices).get(id))
        .flatten()
        .filter(|d| state.device_allowed_from(d, from));
    match device {
        Some(d) => {
            *seen = Instant::now();
            Some(d.role)
        }
        None => {
            sessions.authed.remove(&from);
            None
        }
    }
}

fn authenticate(state: &AppState, sessions: &mut Sessions, from: SocketAddr, token: &str) {
    let now = Instant::now();
    if sessions.failures.len() > 1024 {
        // Spoofed source addresses must not grow this forever.
        sessions
            .failures
            .retain(|_, (_, start)| start.elapsed() <= Duration::from_secs(60));
    }
    let entry = sessions.failures.entry(from.ip()).or_insert((0, now));
    if entry.1.elapsed() > Duration::from_secs(60) {
        *entry = (0, now);
    }
    if entry.0 >= MAX_AUTH_FAILURES {
        return;
    }
    let device = lock(&state.devices).authenticate(token);
    match device.filter(|d| state.device_allowed_from(d, from)) {
        Some(d) => {
            tracing::info!(%from, device = %d.name, "OSC sender authenticated");
            sessions.authed.insert(from, (d.id, now));
        }
        None => {
            entry.0 += 1;
            tracing::warn!(%from, "OSC authentication failed");
        }
    }
}

/// Resolves cue numbers and keeps the countdown label.
fn to_action(state: &AppState, command: Command) -> Option<Action> {
    match command {
        Command::Action(a) => Some(a),
        Command::GoToCue { cue, slide } => {
            let engine = lock(&state.engine);
            let c = engine.show().cues.get(cue.checked_sub(1)? as usize)?;
            Some(Action::GoTo {
                position: Position {
                    cue_id: c.id.clone(),
                    slide: (slide - 1).min(c.slide_count().saturating_sub(1)),
                },
            })
        }
        Command::CountdownDuration(duration_ms) => {
            let label = lock(&state.engine).live_state(0).countdown.label;
            Some(Action::CountdownSet { duration_ms, label })
        }
        Command::Auth(_) | Command::Subscribe(_) | Command::Unsubscribe => None,
    }
}
