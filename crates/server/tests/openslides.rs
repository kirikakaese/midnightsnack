// SPDX-License-Identifier: GPL-3.0-or-later
//! The host connected to the mock OpenSlides server: data reaches clients, OpenSlides cues get
//! their pages from the data, credentials stay on the host.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use midnightsnack_core::Origin;
use midnightsnack_openslides::mock::{MockOpenSlides, MockOptions};
use midnightsnack_protocol::*;
use midnightsnack_server::{start, state::lock, ServerConfig, ServerHandle};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

async fn host(dir: &std::path::Path) -> ServerHandle {
    let mut config = ServerConfig::new(dir.join("cache"));
    config.bind = "127.0.0.1:0".parse().unwrap();
    config.mdns = false;
    config.data_dir = Some(dir.join("data"));
    config.restore_autosave = false;
    start(config).await.unwrap()
}

async fn wait_until(cond: impl Fn() -> bool) {
    for _ in 0..200 {
        if cond() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("condition not reached");
}

fn configure(url: &str, password: Option<&str>, meeting: Option<u32>) -> Action {
    Action::ConfigureOpenSlides {
        enabled: true,
        url: url.into(),
        username: "admin".into(),
        password: password.map(Into::into),
        meeting_id: meeting,
    }
}

#[tokio::test]
async fn meeting_data_and_pages_follow_openslides() {
    let mock = MockOpenSlides::start(MockOptions::default()).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let h = host(dir.path()).await;
    h.state
        .perform(
            Origin::LOCAL_ADMIN,
            configure(&mock.url, Some("admin"), None),
        )
        .await
        .unwrap();
    wait_until(|| h.state.openslides_status().state == OpenSlidesState::SelectMeeting).await;
    let status = h.state.openslides_status();
    assert_eq!(status.meetings.len(), 1);
    assert!(status.has_password);
    assert!(lock(&h.state.openslides_data).is_none());

    // Picking the meeting keeps the stored password.
    h.state
        .perform(Origin::LOCAL_ADMIN, configure(&mock.url, None, Some(1)))
        .await
        .unwrap();
    wait_until(|| lock(&h.state.openslides_data).is_some()).await;
    assert_eq!(
        h.state.openslides_status().state,
        OpenSlidesState::Connected
    );

    // A phone-like client (stage display window token) receives the data, not the status.
    let (mut ws, _) = tokio_tungstenite::connect_async(h.local_ws_url())
        .await
        .unwrap();
    let hello = ClientMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        token: h.stage_token.clone(),
    };
    ws.send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
        .await
        .unwrap();
    let mut got_data = None;
    let mut got_status = false;
    while let Ok(Some(Ok(Message::Text(t)))) =
        tokio::time::timeout(Duration::from_secs(2), ws.next()).await
    {
        match serde_json::from_str::<ServerMessage>(&t).unwrap() {
            ServerMessage::OpenSlides { data } => got_data = data,
            ServerMessage::OpenSlidesStatus { .. } => got_status = true,
            ServerMessage::RenderProgress { .. } => break,
            _ => {}
        }
    }
    let data = got_data.expect("OpenSlides data for every client");
    assert_eq!(data.name, "Delegates assembly 2026");
    assert!(!got_status, "the connection status is for admins only");

    // An agenda cue and a motion cue.
    for slide in [
        OpenSlidesSlide::Agenda,
        OpenSlidesSlide::Motion { motion_id: 1 },
        OpenSlidesSlide::Follow { projector_id: None },
    ] {
        h.state
            .perform(
                Origin::LOCAL_ADMIN,
                Action::AddOpenSlides {
                    name: String::new(),
                    slide,
                    at_index: None,
                },
            )
            .await
            .unwrap();
    }
    let show = h.state.show_snapshot(Role::Admin);
    let names: Vec<&str> = show.cues.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Agenda", "Motion 1", "OpenSlides projector"]);
    assert!(show.cues.iter().all(|c| c.kind == CueKind::OpenSlides));
    lock(&h.state.engine).mark_saved(dir.path().join("show.msnack"));

    // The motion text grows in OpenSlides: the motion cue gets more pages, the show stays
    // unchanged for the user.
    let long: String = (0..10)
        .map(|i| format!("<p>{} {i}</p>", "word ".repeat(50)))
        .collect();
    mock.set(json!({ "motion/1/text": long }));
    wait_until(|| h.state.show_snapshot(Role::Admin).cues[1].slide_count > 2).await;
    let show = h.state.show_snapshot(Role::Admin);
    assert_eq!(show.cues[0].slide_count, 1);
    assert!(!show.dirty, "page counts from OpenSlides are not an edit");

    // Admin-only, and the password never leaves the host.
    let err = h
        .state
        .perform(
            Origin {
                role: Role::Operator,
                local: false,
            },
            configure(&mock.url, Some("x"), Some(1)),
        )
        .await;
    assert_eq!(err, Err(ErrorCode::Forbidden));
    let status_json = serde_json::to_string(&h.state.openslides_status()).unwrap();
    assert!(!status_json.contains("\"password\""), "{status_json}");
    let creds = dir.path().join("data/openslides-credentials.json");
    assert!(std::fs::read_to_string(&creds).unwrap().contains("admin"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&creds).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0);
    }
    let settings = std::fs::read_to_string(dir.path().join("data/settings.json")).unwrap();
    assert!(settings.contains("openslides"));
    assert!(
        !settings.contains("\"password\""),
        "no password in settings.json"
    );

    // Turning it off clears the data.
    h.state
        .perform(
            Origin::LOCAL_ADMIN,
            Action::ConfigureOpenSlides {
                enabled: false,
                url: mock.url.clone(),
                username: "admin".into(),
                password: None,
                meeting_id: Some(1),
            },
        )
        .await
        .unwrap();
    wait_until(|| lock(&h.state.openslides_data).is_none()).await;
    assert_eq!(h.state.openslides_status().state, OpenSlidesState::Off);
}

#[tokio::test]
async fn invalid_settings_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let h = host(dir.path()).await;
    for url in ["", "openslides.example.org", "ftp://x"] {
        let r = h
            .state
            .perform(Origin::LOCAL_ADMIN, configure(url, None, None))
            .await;
        assert_eq!(r, Err(ErrorCode::InvalidState), "{url}");
    }
}
