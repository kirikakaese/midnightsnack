// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end tests of the embedded server over real HTTP and WebSocket connections.

use std::path::PathBuf;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use midnightsnack_protocol::*;
use midnightsnack_render::test_support::{build_pdf, TestPage};
use midnightsnack_server::{start, ServerConfig, ServerHandle};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Fixture {
    handle: ServerHandle,
    dir: tempfile::TempDir,
    http: reqwest::Client,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let mut config = ServerConfig::new(dir.path().join("cache"));
    config.bind = "127.0.0.1:0".parse().unwrap();
    config.mdns = false;
    config.data_dir = Some(dir.path().join("data"));
    config.pdfium_dirs = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/host/src-tauri/resources/pdfium")];
    let handle = start(config).await.unwrap();
    Fixture {
        handle,
        dir,
        http: reqwest::Client::new(),
    }
}

impl Fixture {
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.handle.local_http_url())
    }

    async fn connect(&self, token: &str) -> Client {
        let (ws, _) = tokio_tungstenite::connect_async(self.handle.local_ws_url())
            .await
            .unwrap();
        let mut c = Client { ws, next_id: 0 };
        c.send(&ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            token: token.into(),
        })
        .await;
        c
    }

    fn pdf(&self, name: &str, pages: usize) -> String {
        let p = self.dir.path().join(name);
        let pages: Vec<_> = (0..pages)
            .map(|i| TestPage::landscape([i as f32 / 4.0, 0.2, 0.5]))
            .collect();
        std::fs::write(&p, build_pdf(&pages)).unwrap();
        p.to_string_lossy().into_owned()
    }

    fn png(&self, name: &str) -> String {
        let p = self.dir.path().join(name);
        image::RgbImage::from_pixel(64, 36, image::Rgb([10, 200, 30]))
            .save(&p)
            .unwrap();
        p.to_string_lossy().into_owned()
    }

    async fn pair(&self, operator: &mut Client, name: &str, role: Role) -> String {
        let info = self.handle.state.pairing_info();
        let token = info.join_urls[0].split("#t=").nth(1).unwrap().to_owned();
        let res: PairResponse = self
            .http
            .post(self.url("/api/v1/pair"))
            .json(&PairRequest {
                join_token: token,
                pin: info.pin,
                device_name: name.into(),
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        // Operator sees the request.
        let pending = operator
            .wait(|m| match m {
                ServerMessage::Devices { pending, .. } if !pending.is_empty() => Some(pending),
                _ => None,
            })
            .await;
        assert_eq!(pending[0].device_name, name);
        let status: PairStatus = self.status(&res.request_id).await;
        assert_eq!(status, PairStatus::Pending);
        operator
            .action(Action::ApprovePairing {
                request_id: res.request_id.clone(),
                role,
            })
            .await
            .unwrap();
        match self.status(&res.request_id).await {
            PairStatus::Approved { token, role: r, .. } => {
                assert_eq!(r, role);
                token
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    async fn status(&self, id: &str) -> PairStatus {
        self.http
            .get(self.url(&format!("/api/v1/pair/{id}")))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}

struct Client {
    ws: Ws,
    next_id: u32,
}

impl Client {
    async fn send(&mut self, m: &ClientMessage) {
        self.ws
            .send(Message::Text(serde_json::to_string(m).unwrap().into()))
            .await
            .unwrap();
    }

    async fn recv(&mut self) -> Option<ServerMessage> {
        loop {
            let msg = tokio::time::timeout(Duration::from_secs(10), self.ws.next())
                .await
                .expect("timed out waiting for message")?;
            match msg.ok()? {
                Message::Text(t) => return Some(serde_json::from_str(&t).unwrap()),
                Message::Close(_) => return None,
                _ => continue,
            }
        }
    }

    /// Waits for a matching message; fails after 15 s even if unrelated messages keep coming.
    async fn wait<T>(&mut self, mut f: impl FnMut(ServerMessage) -> Option<T>) -> T {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            let m = tokio::time::timeout_at(deadline, self.recv())
                .await
                .expect("timed out waiting for a matching message")
                .expect("connection closed");
            if let Some(t) = f(m) {
                return t;
            }
        }
    }

    async fn action(&mut self, action: Action) -> Result<(), ErrorCode> {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&ClientMessage::Action {
            request_id: id,
            action,
        })
        .await;
        self.wait(|m| match m {
            ServerMessage::ActionResult { request_id, error } if request_id == id => {
                Some(error.map_or(Ok(()), Err))
            }
            _ => None,
        })
        .await
    }

    async fn welcome(&mut self) -> SessionInfo {
        self.wait(|m| match m {
            ServerMessage::Welcome { session, .. } => Some(session),
            ServerMessage::Error { code } => panic!("rejected: {code:?}"),
            _ => None,
        })
        .await
    }

    async fn show(&mut self) -> ShowSnapshot {
        self.wait(|m| match m {
            ServerMessage::Show { show } => Some(show),
            _ => None,
        })
        .await
    }

    async fn live(&mut self) -> LiveState {
        self.wait(|m| match m {
            ServerMessage::Live { live } => Some(live),
            _ => None,
        })
        .await
    }
}

#[tokio::test]
async fn info_endpoint() {
    let f = fixture().await;
    let info: HostInfo = f
        .http
        .get(f.url("/api/v1/info"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(info.protocol_version, PROTOCOL_VERSION);
}

#[tokio::test]
async fn rejects_bad_tokens_and_versions() {
    let f = fixture().await;
    let mut c = f.connect("nope").await;
    assert_eq!(
        c.recv().await,
        Some(ServerMessage::Error {
            code: ErrorCode::Unauthorized
        })
    );

    let (ws, _) = tokio_tungstenite::connect_async(f.handle.local_ws_url())
        .await
        .unwrap();
    let mut c = Client { ws, next_id: 0 };
    c.send(&ClientMessage::Hello {
        protocol_version: 999,
        token: f.handle.operator_token.clone(),
    })
    .await;
    assert_eq!(
        c.recv().await,
        Some(ServerMessage::Error {
            code: ErrorCode::ProtocolMismatch
        })
    );
}

#[tokio::test]
async fn operator_receives_full_state() {
    let f = fixture().await;
    let mut op = f.connect(&f.handle.operator_token).await;
    let session = op.welcome().await;
    assert_eq!(session.role, Role::Admin);
    assert!(session.local);
    op.show().await;
    op.live().await;
    let pairing = op
        .wait(|m| match m {
            ServerMessage::Pairing { pairing } => Some(pairing),
            _ => None,
        })
        .await;
    assert_eq!(pairing.pin.len(), 6);
    assert!(pairing.join_urls[0].contains("/join#t="));
}

#[tokio::test]
async fn full_pairing_and_control_flow() {
    let f = fixture().await;
    let mut op = f.connect(&f.handle.operator_token).await;
    op.welcome().await;

    // Build a show: PDF (3 pages) + image.
    let pdf = f.pdf("deck.pdf", 3);
    let png = f.png("pic.png");
    let add = op
        .action(Action::AddFiles {
            paths: vec![pdf, png],
            at_index: None,
        })
        .await;
    let pdf_ok = add.is_ok();
    if !pdf_ok {
        assert_eq!(
            add,
            Err(ErrorCode::PdfEngineMissing),
            "only PDFium may be missing"
        );
    }
    let show = op
        .wait(|m| match m {
            ServerMessage::Show { show } if !show.cues.is_empty() => Some(show),
            _ => None,
        })
        .await;
    assert!(show.dirty);

    // Pair a presenter phone.
    let token = f.pair(&mut op, "Phone", Role::Presenter).await;
    let mut phone = f.connect(&token).await;
    let session = phone.welcome().await;
    assert_eq!(session.role, Role::Presenter);
    assert!(!session.local);
    let phone_show = phone.show().await;
    assert_eq!(phone_show.path, None, "paths are only sent to admins");

    // Presenter cannot start the show or blackout.
    assert_eq!(phone.action(Action::Next).await, Err(ErrorCode::Forbidden));
    assert_eq!(
        phone.action(Action::ToggleBlackout).await,
        Err(ErrorCode::Forbidden)
    );
    // Remote devices can never send host paths.
    assert_eq!(
        phone
            .action(Action::AddFiles {
                paths: vec!["/etc".into()],
                at_index: None
            })
            .await,
        Err(ErrorCode::Forbidden)
    );

    // Operator starts; presenter moves within the cue.
    op.action(Action::Go).await.unwrap();
    let live = phone
        .wait(|m| match m {
            ServerMessage::Live { live } if live.program.is_some() => Some(live),
            _ => None,
        })
        .await;
    assert!(live.show_timer.is_running());
    if pdf_ok {
        phone.action(Action::Next).await.unwrap();
        phone.action(Action::Next).await.unwrap();
        // Third slide is the last of the PDF; the next would leave the cue.
        assert_eq!(phone.action(Action::Next).await, Err(ErrorCode::Forbidden));
    }

    // Slide image with the session's media key.
    // `action()` consumes intermediate live updates, so read the position from the host.
    let program = f
        .handle
        .state
        .engine
        .lock()
        .unwrap()
        .program()
        .cloned()
        .unwrap();
    let url = f.url(&format!(
        "/api/v1/media/slide/{}/{}?k={}",
        program.cue_id, program.slide, session.media_key
    ));
    let res = f.http.get(&url).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let etag = res.headers()["etag"].to_str().unwrap().to_owned();
    assert!(res.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("image/"));
    let res = f
        .http
        .get(&url)
        .header("if-none-match", etag)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 304);
    let bad = f.url(&format!("/api/v1/media/slide/{}/0?k=wrong", program.cue_id));
    assert_eq!(f.http.get(bad).send().await.unwrap().status(), 401);

    // Promote the phone; it is told about its new role.
    let devices = f.handle.state.devices.lock().unwrap().list();
    let phone_id = devices
        .iter()
        .find(|d| d.name == "Phone")
        .unwrap()
        .id
        .clone();
    op.action(Action::SetDeviceRole {
        device_id: phone_id.clone(),
        role: Role::Operator,
    })
    .await
    .unwrap();
    let s = phone
        .wait(|m| match m {
            ServerMessage::Session { session } => Some(session),
            _ => None,
        })
        .await;
    assert_eq!(s.role, Role::Operator);
    phone.action(Action::ToggleBlackout).await.unwrap();
    let live = op
        .wait(|m| match m {
            ServerMessage::Live { live } if live.masters.blackout => Some(live),
            _ => None,
        })
        .await;
    assert!(live.masters.blackout);

    // Revoking disconnects the phone and its token stops working.
    op.action(Action::RevokeDevice {
        device_id: phone_id,
    })
    .await
    .unwrap();
    loop {
        match phone.recv().await {
            None => break,
            Some(ServerMessage::Error { code }) => assert_eq!(code, ErrorCode::Unauthorized),
            Some(_) => {}
        }
    }
    let mut again = f.connect(&token).await;
    assert_eq!(
        again.recv().await,
        Some(ServerMessage::Error {
            code: ErrorCode::Unauthorized
        })
    );
}

#[tokio::test]
async fn wrong_pins_lock_out() {
    let f = fixture().await;
    let info = f.handle.state.pairing_info();
    let token = info.join_urls[0].split("#t=").nth(1).unwrap().to_owned();
    let wrong = if info.pin == "000000" {
        "111111"
    } else {
        "000000"
    };
    let mut statuses = vec![];
    for _ in 0..6 {
        let res = f
            .http
            .post(f.url("/api/v1/pair"))
            .json(&PairRequest {
                join_token: token.clone(),
                pin: wrong.into(),
                device_name: "x".into(),
            })
            .send()
            .await
            .unwrap();
        statuses.push(res.status().as_u16());
    }
    assert_eq!(statuses, vec![401, 401, 401, 401, 429, 429]);
    let res = f
        .http
        .post(f.url("/api/v1/pair"))
        .json(&PairRequest {
            join_token: token,
            pin: info.pin,
            device_name: "x".into(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 429, "locked even with the right PIN");
}

#[tokio::test]
async fn auto_approve_and_disconnect_all() {
    let f = fixture().await;
    let mut op = f.connect(&f.handle.operator_token).await;
    op.welcome().await;
    op.action(Action::SetAutoApprove {
        role: Some(Role::StageViewer),
    })
    .await
    .unwrap();

    let info = f.handle.state.pairing_info();
    assert_eq!(info.auto_approve, Some(Role::StageViewer));
    let token = info.join_urls[0].split("#t=").nth(1).unwrap().to_owned();
    let res: PairResponse = f
        .http
        .post(f.url("/api/v1/pair"))
        .json(&PairRequest {
            join_token: token,
            pin: info.pin,
            device_name: "Stage".into(),
        })
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let PairStatus::Approved { token, role, .. } = f.status(&res.request_id).await else {
        panic!("expected auto approval")
    };
    assert_eq!(role, Role::StageViewer);
    let mut stage = f.connect(&token).await;
    stage.welcome().await;
    assert_eq!(stage.action(Action::Next).await, Err(ErrorCode::Forbidden));

    op.action(Action::DisconnectAll).await.unwrap();
    while stage.recv().await.is_some() {}
    // The operator window stays connected.
    op.action(Action::ToggleLogo).await.unwrap();
}

#[tokio::test]
async fn save_and_open_show() {
    let f = fixture().await;
    let mut op = f.connect(&f.handle.operator_token).await;
    op.welcome().await;
    let png = f.png("a.png");
    op.action(Action::AddFiles {
        paths: vec![png.clone()],
        at_index: None,
    })
    .await
    .unwrap();
    op.action(Action::RenameShow {
        title: "Gala".into(),
    })
    .await
    .unwrap();
    let file = f.dir.path().join("gala");
    op.action(Action::SaveShow {
        path: Some(file.to_string_lossy().into()),
        embed_media: true,
    })
    .await
    .unwrap();
    let saved = f.dir.path().join("gala.msnack");
    assert!(saved.is_file(), "extension is added");
    let show = op
        .wait(|m| match m {
            ServerMessage::Show { show } if !show.dirty => Some(show),
            _ => None,
        })
        .await;
    assert_eq!(show.path.as_deref(), Some(saved.to_string_lossy().as_ref()));

    std::fs::remove_file(png).unwrap();
    op.action(Action::NewShow).await.unwrap();
    op.action(Action::OpenShow {
        path: saved.to_string_lossy().into(),
    })
    .await
    .unwrap();
    let show = op
        .wait(|m| match m {
            ServerMessage::Show { show } if show.title == "Gala" => Some(show),
            _ => None,
        })
        .await;
    assert_eq!(show.cues.len(), 1);
    // Embedded media still renders although the original is gone.
    let source = f.handle.state.slide_source(&show.cues[0].id, 0).unwrap();
    let rendered = f
        .handle
        .state
        .render
        .render(source, midnightsnack_render::TargetSize::new(320, 180))
        .await;
    assert!(rendered.is_ok());

    assert_eq!(
        op.action(Action::OpenShow {
            path: "/does/not/exist.msnack".into()
        })
        .await,
        Err(ErrorCode::Io)
    );
}

#[tokio::test]
async fn unsupported_files_are_reported() {
    let f = fixture().await;
    let mut op = f.connect(&f.handle.operator_token).await;
    op.welcome().await;
    let txt = f.dir.path().join("notes.txt");
    std::fs::write(&txt, "x").unwrap();
    let png = f.png("ok.png");
    let r = op
        .action(Action::AddFiles {
            paths: vec![txt.to_string_lossy().into(), png],
            at_index: None,
        })
        .await;
    assert_eq!(r, Err(ErrorCode::UnsupportedFile));
    // The valid file was still added.
    assert_eq!(f.handle.state.show_snapshot(Role::Admin).cues.len(), 1);
}

#[tokio::test]
async fn serves_remote_or_placeholder() {
    let f = fixture().await;
    let res = f.http.get(f.url("/join")).send().await.unwrap();
    // 200 when apps/remote/dist is built, otherwise a clear 404.
    assert!(res.status() == 200 || res.status() == 404);
    assert_eq!(res.headers()["x-content-type-options"], "nosniff");
}

#[tokio::test]
async fn media_files_support_range_requests() {
    let f = fixture().await;
    let mut op = f.connect(&f.handle.operator_token).await;
    let session = op.welcome().await;
    let clip = f.dir.path().join("clip.mp4");
    std::fs::write(&clip, (0u8..=255).cycle().take(10_000).collect::<Vec<u8>>()).unwrap();
    op.action(Action::AddFiles {
        paths: vec![clip.to_string_lossy().into()],
        at_index: None,
    })
    .await
    .unwrap();
    let show = f.handle.state.show_snapshot(Role::Admin);
    assert_eq!(show.cues[0].kind, CueKind::Video);
    let url = f.url(&format!(
        "/api/v1/media/file/{}?k={}",
        show.cues[0].id, session.media_key
    ));

    let res = f
        .http
        .get(&url)
        .header("range", "bytes=100-109")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 206);
    assert_eq!(res.headers()["content-type"], "video/mp4");
    let body = res.bytes().await.unwrap();
    assert_eq!(body.as_ref(), &(100u8..110).collect::<Vec<u8>>()[..]);

    let res = f.http.get(&url).send().await.unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.bytes().await.unwrap().len(), 10_000);

    let no_key = f.url(&format!("/api/v1/media/file/{}?k=nope", show.cues[0].id));
    assert_eq!(f.http.get(no_key).send().await.unwrap().status(), 401);
}

#[tokio::test]
async fn scheduler_auto_advances() {
    let f = fixture().await;
    let mut op = f.connect(&f.handle.operator_token).await;
    op.welcome().await;
    let (a, b) = (f.png("a.png"), f.png("b.png"));
    op.action(Action::AddFiles {
        paths: vec![a, b],
        at_index: None,
    })
    .await
    .unwrap();
    let first = f.handle.state.show_snapshot(Role::Admin).cues[0].id.clone();
    let second = f.handle.state.show_snapshot(Role::Admin).cues[1].id.clone();
    op.action(Action::SetCueAutoAdvance {
        cue_id: first,
        after_ms: Some(300),
    })
    .await
    .unwrap();
    op.action(Action::Go).await.unwrap();
    let live = op
        .wait(|m| match m {
            ServerMessage::Live { live }
                if live.program.as_ref().is_some_and(|p| p.cue_id == second) =>
            {
                Some(live)
            }
            _ => None,
        })
        .await;
    assert!(live.auto_advance_at_ms.is_none());
}

#[tokio::test]
async fn logo_assets_and_stage_messages() {
    let f = fixture().await;
    let mut op = f.connect(&f.handle.operator_token).await;
    let session = op.welcome().await;
    let logo = f.png("logo.png");
    op.action(Action::SetLogoImage { path: Some(logo) })
        .await
        .unwrap();
    let show = f.handle.state.show_snapshot(Role::Admin);
    let asset = show.logo.expect("logo asset");
    let res = f
        .http
        .get(f.url(&format!(
            "/api/v1/media/asset/{asset}?k={}",
            session.media_key
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["content-type"], "image/png");
    let txt = f.dir.path().join("x.txt");
    std::fs::write(&txt, "x").unwrap();
    assert_eq!(
        op.action(Action::SetLogoImage {
            path: Some(txt.to_string_lossy().into())
        })
        .await,
        Err(ErrorCode::UnsupportedFile)
    );

    // A stage viewer receives the operator's message.
    op.action(Action::SetAutoApprove {
        role: Some(Role::StageViewer),
    })
    .await
    .unwrap();
    let info = f.handle.state.pairing_info();
    let token = info.join_urls[0].split("#t=").nth(1).unwrap().to_owned();
    let res: PairResponse = f
        .http
        .post(f.url("/api/v1/pair"))
        .json(&PairRequest {
            join_token: token,
            pin: info.pin,
            device_name: "Stage".into(),
        })
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let PairStatus::Approved { token, .. } = f.status(&res.request_id).await else {
        panic!()
    };
    let mut stage = f.connect(&token).await;
    stage.welcome().await;
    op.action(Action::SetStageMessage {
        text: Some("Wrap up".into()),
    })
    .await
    .unwrap();
    let msg = stage
        .wait(|m| match m {
            ServerMessage::Live { live } => live.stage_message,
            _ => None,
        })
        .await;
    assert_eq!(msg, "Wrap up");
}
