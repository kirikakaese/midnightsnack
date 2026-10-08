// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use serde::{Deserialize, Serialize};

fn round_trip<T>(value: &T) -> T
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    let json = serde_json::to_string(value).expect("serialize");
    serde_json::from_str(&json).expect("deserialize")
}

fn pos(slide: u32) -> Position {
    Position {
        cue_id: "c1".into(),
        slide,
    }
}

#[test]
fn client_messages_round_trip() {
    let msgs = [
        ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            token: "t".into(),
        },
        ClientMessage::Ping { nonce: 7 },
        ClientMessage::Viewport {
            width: 1920,
            height: 1080,
        },
        ClientMessage::Action {
            request_id: 1,
            action: Action::Next,
        },
        ClientMessage::Action {
            request_id: 2,
            action: Action::GoTo { position: pos(3) },
        },
        ClientMessage::Action {
            request_id: 3,
            action: Action::SaveShow {
                path: Some("/tmp/a.msnack".into()),
                embed_media: true,
            },
        },
        ClientMessage::Action {
            request_id: 4,
            action: Action::ApprovePairing {
                request_id: "r".into(),
                role: Role::Presenter,
            },
        },
    ];
    for m in msgs {
        assert_eq!(round_trip(&m), m);
    }
}

#[test]
fn server_messages_round_trip() {
    let msgs = [
        ServerMessage::Welcome {
            host: HostInfo {
                name: "Stage left".into(),
                app_version: "0.0.1".into(),
                protocol_version: PROTOCOL_VERSION,
            },
            session: SessionInfo {
                device_id: "d".into(),
                device_name: "Phone".into(),
                role: Role::Operator,
                local: false,
                media_key: "k".into(),
            },
        },
        ServerMessage::Show {
            show: ShowSnapshot {
                id: "s".into(),
                title: "Gala".into(),
                cues: vec![CueSummary {
                    id: "c1".into(),
                    name: "Intro".into(),
                    kind: CueKind::Pdf,
                    slide_count: 4,
                    color: Some("#ef4444".into()),
                    notes: String::new(),
                    slide_notes: vec!["hello".into()],
                    background: None,
                }],
                path: None,
                dirty: true,
                revision: 3,
            },
        },
        ServerMessage::Live {
            live: LiveState {
                program: Some(pos(1)),
                output: Some(pos(0)),
                next: Some(pos(2)),
                prev: Some(pos(0)),
                masters: Masters {
                    blackout: false,
                    freeze: true,
                    logo: false,
                },
                show_timer: Stopwatch {
                    accumulated_ms: 10,
                    running_since_ms: Some(1000),
                },
                slide_timer: Stopwatch::default(),
                host_time_ms: 2000,
                revision: 9,
            },
        },
        ServerMessage::ActionResult {
            request_id: 1,
            error: Some(ErrorCode::Forbidden),
        },
        ServerMessage::Pong { nonce: 1 },
        ServerMessage::Error {
            code: ErrorCode::Forbidden,
        },
    ];
    for m in msgs {
        assert_eq!(round_trip(&m), m);
    }
}

#[test]
fn wire_format_is_tagged_snake_case() {
    let json = serde_json::to_value(ClientMessage::Ping { nonce: 3 }).unwrap();
    assert_eq!(json, serde_json::json!({ "type": "ping", "nonce": 3 }));

    let json = serde_json::to_value(ClientMessage::Action {
        request_id: 1,
        action: Action::SetBlackout { on: true },
    })
    .unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "type": "action",
            "request_id": 1,
            "action": { "action": "set_blackout", "on": true }
        })
    );

    let json = serde_json::to_value(PairStatus::Pending).unwrap();
    assert_eq!(json, serde_json::json!({ "status": "pending" }));
}

#[test]
fn roles_are_ordered_by_privilege() {
    assert!(Role::Admin > Role::Operator);
    assert!(Role::Operator > Role::Presenter);
    assert!(Role::Presenter > Role::StageViewer);
}

#[test]
fn stopwatch_elapsed() {
    let sw = Stopwatch {
        accumulated_ms: 500,
        running_since_ms: Some(1_000),
    };
    assert_eq!(sw.elapsed_ms(1_250), 750);
    assert!(sw.is_running());
    let stopped = Stopwatch {
        accumulated_ms: 500,
        running_since_ms: None,
    };
    assert_eq!(stopped.elapsed_ms(99_999), 500);
}
