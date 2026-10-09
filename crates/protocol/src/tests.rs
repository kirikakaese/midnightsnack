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
        ClientMessage::Pointer {
            pos: None,
            mode: PointerMode::Point,
            color: "#ff0000".into(),
        },
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
                    transition: Some(Transition {
                        kind: TransitionKind::Fade,
                        duration_ms: 300,
                    }),
                    auto_advance_ms: Some(5000),
                    media: Some(MediaInfo {
                        options: MediaOptions::default(),
                        duration_ms: Some(1),
                    }),
                    text: Some(TextInfo {
                        source: "a\n\nb".into(),
                        lyrics: true,
                        slides: vec!["a".into(), "b".into()],
                        theme: Some(TextTheme::default()),
                    }),
                    timer: Some(TimerCue {
                        mode: TimerMode::CountdownTo {
                            time: "19:30".into(),
                        },
                        label: "Doors".into(),
                        overtime_color: "#ef4444".into(),
                        theme: None,
                    }),
                    web: Some(WebInfo {
                        url: "https://example.org".into(),
                        zoom: 100,
                        block_navigation: true,
                        forward_keys: true,
                        persist_session: false,
                        openslides: false,
                    }),
                    capture: Some(CaptureInfo {
                        source: CaptureSource::Window {
                            app: "Firefox".into(),
                            title: "".into(),
                        },
                        fps: 30,
                    }),
                    openslides: Some(OpenSlidesCue {
                        slide: OpenSlidesSlide::Speakers { list_id: Some(2) },
                        theme: None,
                    }),
                    targets: Some(vec!["main".into()]),
                    converted_from: Some("talk.pptx".into()),
                }],
                path: None,
                dirty: true,
                revision: 3,
                default_transition: Transition::default(),
                default_theme: TextTheme::default(),
                overlays: vec![Overlay {
                    id: "o".into(),
                    name: "Name".into(),
                    kind: OverlayKind::Ticker {
                        text: "News".into(),
                        speed: 10,
                    },
                    position: OverlayPosition::TopRight,
                    color: "#fff".into(),
                    background: "#000".into(),
                    scale: 100,
                }],
                logo: Some("asset".into()),
                outputs: vec![OutputDef::main()],
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
                media: Some(MediaPlayback {
                    cue_id: "c1".into(),
                    position: Stopwatch {
                        accumulated_ms: 500,
                        running_since_ms: None,
                    },
                    ended: false,
                }),
                countdown: Countdown::default(),
                overlays_visible: vec!["o".into()],
                stage_message: Some("2 min".into()),
                auto_advance_at_ms: Some(3000),
                outputs: vec![OutputLive {
                    output_id: "main".into(),
                    position: Some(pos(0)),
                }],
                test_pattern: Some(TestPattern::Bars),
                capture_lost: vec![],
                web_nav: Some(WebNav {
                    cue_id: "c1".into(),
                    forward: true,
                    seq: 4,
                }),
                drawing: Some(Drawing {
                    position: pos(0),
                    strokes: vec![Stroke {
                        color: "#ff0000".into(),
                        width: 0.01,
                        points: vec![[0.1, 0.2], [0.3, 0.4]],
                    }],
                }),
                host_time_ms: 2000,
                revision: 9,
            },
        },
        ServerMessage::ActionResult {
            request_id: 1,
            error: Some(ErrorCode::Forbidden),
        },
        ServerMessage::Pointer {
            device_id: "d".into(),
            pos: Some([0.5, 0.25]),
            mode: PointerMode::Draw,
            color: "#00ff00".into(),
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

    // `loop` is a keyword in Rust but the natural name on the wire.
    let json = serde_json::to_value(MediaOptions::default()).unwrap();
    assert_eq!(json["loop"], serde_json::json!(false));

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
