// SPDX-License-Identifier: GPL-3.0-or-later
//! The host behind a real relay: a Noise client plays the phone, tunnels HTTP and the
//! WebSocket session, and checks that local-only rules hold through the relay.

use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use midnightsnack_core::Origin;
use midnightsnack_protocol::*;
use midnightsnack_server::{start, state::lock, ServerConfig, ServerHandle};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Setup {
    handle: ServerHandle,
    relay_url: String,
    _dir: tempfile::TempDir,
}

async fn setup() -> Setup {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_url = format!("http://{}", listener.local_addr().unwrap());
    let relay = midnightsnack_relay::Relay::new(Default::default());
    tokio::spawn(midnightsnack_relay::serve(listener, relay));

    let dir = tempfile::tempdir().unwrap();
    let mut config = ServerConfig::new(dir.path().join("cache"));
    config.bind = "127.0.0.1:0".parse().unwrap();
    config.mdns = false;
    config.data_dir = Some(dir.path().join("data"));
    let handle = start(config).await.unwrap();
    handle
        .state
        .perform(
            Origin::LOCAL_ADMIN,
            Action::ConfigureRelay {
                enabled: true,
                url: relay_url.clone(),
                access_token: None,
            },
        )
        .await
        .unwrap();
    wait_until(|| lock(&handle.state.relay_runtime).state == RelayState::Connected).await;
    Setup {
        handle,
        relay_url,
        _dir: dir,
    }
}

async fn wait_until(cond: impl Fn() -> bool) {
    for _ in 0..100 {
        if cond() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("condition not reached");
}

/// The relay join link: `<relay>/r/<host id>#t=<join token>&k=<key>`.
fn relay_link(s: &Setup) -> (String, String, Vec<u8>) {
    let pairing = s.handle.state.pairing_info();
    let link = pairing
        .links
        .iter()
        .find(|l| l.kind == JoinKind::Relay)
        .expect("relay link while connected")
        .url
        .clone();
    let (page, fragment) = link.split_once('#').unwrap();
    let host_id = page.rsplit('/').next().unwrap().to_owned();
    let mut token = String::new();
    let mut key = Vec::new();
    for part in fragment.split('&') {
        if let Some(t) = part.strip_prefix("t=") {
            token = t.to_owned();
        }
        if let Some(k) = part.strip_prefix("k=") {
            key = B64.decode(k).unwrap();
        }
    }
    (host_id, token, key)
}

struct Remote {
    ws: Ws,
    noise: snow::TransportState,
    next_stream: u32,
}

fn frame(kind: u8, stream: u32, fin: bool, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![kind];
    out.extend_from_slice(&stream.to_be_bytes());
    out.push(u8::from(fin));
    out.extend_from_slice(payload);
    out
}

impl Remote {
    async fn connect(relay_url: &str, host_id: &str, key: &[u8]) -> Result<Remote, Option<u16>> {
        let url = format!(
            "{}/relay/v1/remote/{host_id}",
            relay_url.replace("http://", "ws://")
        );
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|_| None)?;
        let prologue = format!("{RELAY_PROLOGUE}{host_id}");
        let mut hs = snow::Builder::new(RELAY_NOISE_PATTERN.parse().unwrap())
            .remote_public_key(key)
            .unwrap()
            .prologue(prologue.as_bytes())
            .unwrap()
            .build_initiator()
            .unwrap();
        let mut buf = vec![0u8; NOISE_MAX_MESSAGE];
        let n = hs.write_message(&[], &mut buf).unwrap();
        ws.send(Message::Binary(buf[..n].to_vec().into()))
            .await
            .map_err(|_| None)?;
        loop {
            match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
                Ok(Some(Ok(Message::Binary(b)))) => {
                    hs.read_message(&b, &mut buf).map_err(|_| None)?;
                    break;
                }
                Ok(Some(Ok(Message::Close(c)))) => return Err(c.map(|c| c.code.into())),
                Ok(Some(Ok(_))) => continue,
                _ => return Err(None),
            }
        }
        Ok(Remote {
            ws,
            noise: hs.into_transport_mode().unwrap(),
            next_stream: 1,
        })
    }

    async fn send_frame(&mut self, f: Vec<u8>) {
        let mut buf = vec![0u8; NOISE_MAX_MESSAGE];
        let n = self.noise.write_message(&f, &mut buf).unwrap();
        self.ws
            .send(Message::Binary(buf[..n].to_vec().into()))
            .await
            .unwrap();
    }

    /// Next decrypted frame: (kind, stream, fin, payload).
    async fn recv_frame(&mut self) -> Option<(u8, u32, bool, Vec<u8>)> {
        let mut buf = vec![0u8; NOISE_MAX_MESSAGE];
        loop {
            let msg = tokio::time::timeout(Duration::from_secs(10), self.ws.next())
                .await
                .ok()??
                .ok()?;
            let Message::Binary(b) = msg else {
                if matches!(msg, Message::Close(_)) {
                    return None;
                }
                continue;
            };
            let n = self.noise.read_message(&b, &mut buf).ok()?;
            let p = &buf[..n];
            let stream = u32::from_be_bytes(p[1..5].try_into().unwrap());
            return Some((p[0], stream, p[5] & 1 != 0, p[6..].to_vec()));
        }
    }

    /// One HTTP exchange through the tunnel: (status, body).
    async fn http(&mut self, method: &str, path: &str, body: Option<&[u8]>) -> (u16, Vec<u8>) {
        let stream = self.next_stream;
        self.next_stream += 1;
        let head = serde_json::json!({
            "method": method,
            "path": path,
            "headers": [["content-type", "application/json"]],
        });
        let head = serde_json::to_vec(&head).unwrap();
        self.send_frame(frame(tunnel::REQUEST_HEAD, stream, body.is_none(), &head))
            .await;
        if let Some(b) = body {
            self.send_frame(frame(tunnel::REQUEST_BODY, stream, true, b))
                .await;
        }
        let mut status = 0;
        let mut out = Vec::new();
        loop {
            let (kind, s, fin, payload) = self.recv_frame().await.expect("response");
            if s != stream {
                continue;
            }
            match kind {
                tunnel::RESPONSE_HEAD => {
                    let v: serde_json::Value = serde_json::from_slice(&payload).unwrap();
                    status = v["status"].as_u64().unwrap() as u16;
                    if fin {
                        return (status, out);
                    }
                }
                tunnel::RESPONSE_BODY => {
                    out.extend_from_slice(&payload);
                    if fin {
                        return (status, out);
                    }
                }
                tunnel::RESET => return (0, out),
                _ => {}
            }
        }
    }

    async fn ws_send(&mut self, msg: &ClientMessage) {
        let text = serde_json::to_vec(msg).unwrap();
        self.send_frame(frame(tunnel::WS_MESSAGE, 0, true, &text))
            .await;
    }

    /// Next server message on stream 0 (answers host pings); `None` once the session closed.
    async fn ws_recv(&mut self) -> Option<ServerMessage> {
        let mut message = Vec::new();
        loop {
            let (kind, stream, fin, payload) = self.recv_frame().await?;
            match (kind, stream) {
                (tunnel::PING, _) => {
                    self.send_frame(frame(tunnel::PONG, stream, true, &payload))
                        .await
                }
                (tunnel::WS_CLOSE, 0) => return None,
                (tunnel::WS_MESSAGE, 0) => {
                    message.extend_from_slice(&payload);
                    if fin {
                        return Some(serde_json::from_slice(&message).unwrap());
                    }
                }
                _ => {}
            }
        }
    }

    async fn ws_wait(&mut self, pred: impl Fn(&ServerMessage) -> bool) -> ServerMessage {
        loop {
            let m = self.ws_recv().await.expect("session open");
            if pred(&m) {
                return m;
            }
        }
    }
}

#[tokio::test]
async fn relay_host_id_matches_the_relay() {
    let s = setup().await;
    let identity = lock(&s.handle.state.relay_identity).clone();
    let secret = B64.decode(identity.secret_b64()).unwrap();
    assert_eq!(
        identity.host_id(),
        midnightsnack_relay::host_id_for_secret(&secret)
    );
}

#[tokio::test]
async fn phone_pairs_and_runs_the_show_through_the_relay() {
    let s = setup().await;
    let (host_id, join_token, key) = relay_link(&s);
    let mut phone = Remote::connect(&s.relay_url, &host_id, &key).await.unwrap();

    let (status, body) = phone.http("GET", "/api/v1/info", None).await;
    assert_eq!(status, 200);
    let info: HostInfo = serde_json::from_slice(&body).unwrap();
    assert_eq!(info.protocol_version, PROTOCOL_VERSION);

    let pin = s.handle.state.pairing_info().pin;
    let req = serde_json::to_vec(&PairRequest {
        join_token,
        pin,
        device_name: "Phone on 5G".into(),
    })
    .unwrap();
    let (status, body) = phone.http("POST", "/api/v1/pair", Some(&req)).await;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let request_id = serde_json::from_slice::<PairResponse>(&body)
        .unwrap()
        .request_id;
    let pending = lock(&s.handle.state.pairing).pending();
    assert_eq!(pending.len(), 1);
    assert!(pending[0].via_relay, "requests over the relay are labeled");
    s.handle
        .state
        .perform(
            Origin::LOCAL_ADMIN,
            Action::ApprovePairing {
                request_id: request_id.clone(),
                role: Role::Operator,
            },
        )
        .await
        .unwrap();
    let (_, body) = phone
        .http("GET", &format!("/api/v1/pair/{request_id}"), None)
        .await;
    let PairStatus::Approved {
        token, device_id, ..
    } = serde_json::from_slice(&body).unwrap()
    else {
        panic!("not approved");
    };

    phone
        .ws_send(&ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            token,
        })
        .await;
    let welcome = phone
        .ws_wait(|m| matches!(m, ServerMessage::Welcome { .. }))
        .await;
    let ServerMessage::Welcome { session, .. } = welcome else {
        unreachable!()
    };
    assert_eq!(session.role, Role::Operator);
    assert!(!session.local);
    let routes = phone
        .ws_wait(|m| matches!(m, ServerMessage::Routes { .. }))
        .await;
    let ServerMessage::Routes { routes } = routes else {
        unreachable!()
    };
    let route = routes.relay.expect("relay route");
    assert!(route.url.ends_with(&format!("/r/{host_id}")));
    assert_eq!(B64.decode(route.key).unwrap(), key);

    phone
        .ws_send(&ClientMessage::Action {
            request_id: 1,
            action: Action::SetBlackout { on: true },
        })
        .await;
    let result = phone
        .ws_wait(|m| matches!(m, ServerMessage::ActionResult { .. }))
        .await;
    assert_eq!(
        result,
        ServerMessage::ActionResult {
            request_id: 1,
            error: None
        }
    );
    assert!(
        s.handle
            .state
            .engine
            .lock()
            .unwrap()
            .live_state(0)
            .masters
            .blackout
    );

    // Host-only actions stay refused for a paired operator over the relay.
    phone
        .ws_send(&ClientMessage::Action {
            request_id: 2,
            action: Action::OpenShow {
                path: "/etc/passwd".into(),
            },
        })
        .await;
    let result = phone
        .ws_wait(|m| matches!(m, ServerMessage::ActionResult { request_id: 2, .. }))
        .await;
    assert!(matches!(
        result,
        ServerMessage::ActionResult { error: Some(_), .. }
    ));

    let devices = lock(&s.handle.state.devices).list();
    let me = devices.iter().find(|d| d.id == device_id).unwrap();
    assert!(me.connected);
    assert_eq!(me.path, Some(ConnectionPath::Relay));
    assert_eq!(me.address, None);
    assert_eq!(lock(&s.handle.state.relay_runtime).remotes, 1);
}

#[tokio::test]
async fn host_tokens_and_local_api_keys_are_refused_through_the_relay() {
    let s = setup().await;
    let (host_id, _, key) = relay_link(&s);
    let (_, api_key) = lock(&s.handle.state.devices).add_api_key("Deck", Role::Operator);
    for token in [s.handle.operator_token.clone(), api_key] {
        let mut phone = Remote::connect(&s.relay_url, &host_id, &key).await.unwrap();
        phone
            .ws_send(&ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                token,
            })
            .await;
        let m = phone.ws_recv().await;
        assert_eq!(
            m,
            Some(ServerMessage::Error {
                code: ErrorCode::Unauthorized
            })
        );
        assert_eq!(phone.ws_recv().await, None, "session closed");
    }
    // Video files and capture streams are not served through the relay.
    let mut phone = Remote::connect(&s.relay_url, &host_id, &key).await.unwrap();
    let (status, _) = phone.http("GET", "/api/v1/media/file/x?k=y", None).await;
    assert_eq!(status, 404);
    let (status, _) = phone.http("GET", "/", None).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn wrong_key_and_offline_hosts_fail_cleanly() {
    let s = setup().await;
    let (host_id, _, _) = relay_link(&s);
    let impostor = snow::Builder::new(RELAY_NOISE_PATTERN.parse().unwrap())
        .generate_keypair()
        .unwrap();
    assert!(Remote::connect(&s.relay_url, &host_id, &impostor.public)
        .await
        .is_err());

    let err = Remote::connect(&s.relay_url, "nobody-is-here", &impostor.public).await;
    assert_eq!(err.err(), Some(Some(relay_close::HOST_OFFLINE)));

    // Turning the relay off disconnects the host from it.
    s.handle
        .state
        .perform(
            Origin::LOCAL_ADMIN,
            Action::ConfigureRelay {
                enabled: false,
                url: s.relay_url.clone(),
                access_token: None,
            },
        )
        .await
        .unwrap();
    wait_until(|| lock(&s.handle.state.relay_runtime).state == RelayState::Off).await;
    assert!(!s
        .handle
        .state
        .pairing_info()
        .links
        .iter()
        .any(|l| l.kind == JoinKind::Relay));
    let key = B64
        .decode(lock(&s.handle.state.relay_identity).public_key_b64())
        .unwrap();
    let mut gone = None;
    for _ in 0..40 {
        match Remote::connect(&s.relay_url, &host_id, &key).await {
            Err(code) => {
                gone = code;
                break;
            }
            Ok(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
    assert_eq!(gone, Some(relay_close::HOST_OFFLINE));
}

#[tokio::test]
async fn relays_with_an_access_token_refuse_other_hosts() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_url = format!("http://{}", listener.local_addr().unwrap());
    let relay = midnightsnack_relay::Relay::new(midnightsnack_relay::RelayConfig {
        access_token: Some("letmein".into()),
        ..Default::default()
    });
    tokio::spawn(midnightsnack_relay::serve(listener, relay.clone()));
    let dir = tempfile::tempdir().unwrap();
    let mut config = ServerConfig::new(dir.path().join("cache"));
    config.bind = "127.0.0.1:0".parse().unwrap();
    config.mdns = false;
    let handle = start(config).await.unwrap();
    let configure = |token: Option<&str>| Action::ConfigureRelay {
        enabled: true,
        url: relay_url.clone(),
        access_token: token.map(Into::into),
    };
    handle
        .state
        .perform(Origin::LOCAL_ADMIN, configure(None))
        .await
        .unwrap();
    wait_until(|| lock(&handle.state.relay_runtime).error == Some(RelayError::Unauthorized)).await;
    assert_eq!(relay.host_count(), 0);
    handle
        .state
        .perform(Origin::LOCAL_ADMIN, configure(Some("letmein")))
        .await
        .unwrap();
    wait_until(|| lock(&handle.state.relay_runtime).state == RelayState::Connected).await;
    assert_eq!(relay.host_count(), 1);
    let status = handle.state.connectivity().relay;
    assert!(status.has_access_token);
}
