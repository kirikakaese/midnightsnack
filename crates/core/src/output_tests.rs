// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for multiple outputs, cue targets, test patterns, web and capture cues.

use crate::engine::tests::pdf;
use crate::model::Show;
use crate::protocol::*;
use crate::Engine;

fn room(id: &str, feed: OutputFeed) -> OutputDef {
    OutputDef {
        id: id.into(),
        name: id.into(),
        feed,
        ..OutputDef::main()
    }
}

/// Outputs: main, room2 (program), stage (stage feed). Cues: a → main, b → room2, c → all.
fn engine() -> Engine {
    let mut a = pdf("a", 1);
    a.targets = Some(vec!["main".into()]);
    let mut b = pdf("b", 1);
    b.targets = Some(vec!["room2".into()]);
    let c = pdf("c", 1);
    let show = Show {
        cues: vec![a, b, c],
        outputs: vec![
            OutputDef::main(),
            room("room2", OutputFeed::Program),
            room("stage", OutputFeed::Stage),
        ],
        ..Show::default()
    };
    Engine::new(show)
}

fn shows(e: &Engine, output: &str) -> Option<String> {
    let live = e.live_state(0);
    let o = live
        .outputs
        .iter()
        .find(|o| o.output_id == output)
        .expect("program output");
    o.position
        .as_ref()
        .map(|p| e.show().cue(&p.cue_id).unwrap().name.clone())
}

#[test]
fn outputs_keep_their_last_targeted_cue() {
    let mut e = engine();
    e.apply(&Action::Next, 0).unwrap();
    assert_eq!(
        (shows(&e, "main"), shows(&e, "room2")),
        (Some("a".into()), None)
    );
    e.apply(&Action::Next, 0).unwrap();
    assert_eq!(
        (shows(&e, "main"), shows(&e, "room2")),
        (Some("a".into()), Some("b".into()))
    );
    e.apply(&Action::Next, 0).unwrap();
    assert_eq!(
        (shows(&e, "main"), shows(&e, "room2")),
        (Some("c".into()), Some("c".into()))
    );
    // `output` mirrors the main output.
    assert_eq!(e.live_state(0).output.unwrap().cue_id, e.show().cues[2].id);
    // Stage-feed outputs have no program position.
    assert_eq!(e.live_state(0).outputs.len(), 2);
}

#[test]
fn freeze_holds_every_output_and_release_catches_up() {
    let mut e = engine();
    e.apply(&Action::Next, 0).unwrap();
    e.apply(&Action::SetFreeze { on: true }, 0).unwrap();
    e.apply(&Action::Next, 0).unwrap();
    e.apply(&Action::Next, 0).unwrap();
    assert_eq!(
        (shows(&e, "main"), shows(&e, "room2")),
        (Some("a".into()), None)
    );
    e.apply(&Action::SetFreeze { on: false }, 0).unwrap();
    assert_eq!(
        (shows(&e, "main"), shows(&e, "room2")),
        (Some("c".into()), Some("c".into()))
    );
}

#[test]
fn retargeting_the_live_cue_reroutes() {
    let mut e = engine();
    e.apply(&Action::Next, 0).unwrap();
    let a = e.show().cues[0].id.clone();
    e.apply(
        &Action::SetCueTargets {
            cue_id: a.clone(),
            targets: None,
        },
        0,
    )
    .unwrap();
    assert_eq!(shows(&e, "room2"), Some("a".into()));
    assert_eq!(
        e.apply(
            &Action::SetCueTargets {
                cue_id: a,
                targets: Some(vec!["nope".into()])
            },
            0
        ),
        Err(ErrorCode::NotFound)
    );
}

#[test]
fn outputs_can_be_added_and_removed() {
    let mut e = engine();
    e.apply(&Action::Next, 0).unwrap();
    e.apply(
        &Action::PutOutput {
            output: room("lobby", OutputFeed::Program),
        },
        0,
    )
    .unwrap();
    assert_eq!(
        shows(&e, "lobby"),
        Some("a".into()),
        "new outputs start with the main content"
    );
    e.apply(
        &Action::RemoveOutput {
            output_id: "room2".into(),
        },
        0,
    )
    .unwrap();
    assert!(
        e.show().cues[1].targets.as_ref().unwrap().is_empty(),
        "targets are cleaned up"
    );
    e.apply(
        &Action::RemoveOutput {
            output_id: "lobby".into(),
        },
        0,
    )
    .unwrap();
    assert_eq!(
        e.apply(
            &Action::RemoveOutput {
                output_id: "main".into()
            },
            0
        ),
        Err(ErrorCode::InvalidState),
        "the last program output stays"
    );
    assert_eq!(
        e.show()
            .outputs
            .iter()
            .filter(|o| o.feed == OutputFeed::Program)
            .count(),
        1
    );
    let bad = OutputDef {
        margin: 50,
        ..room("x", OutputFeed::Program)
    };
    assert_eq!(
        e.apply(&Action::PutOutput { output: bad }, 0),
        Err(ErrorCode::InvalidState)
    );
}

#[test]
fn removing_a_cue_clears_outputs_showing_it() {
    let mut e = engine();
    e.apply(&Action::Next, 0).unwrap();
    e.apply(&Action::Next, 0).unwrap();
    let b = e.show().cues[1].id.clone();
    e.apply(&Action::RemoveCue { cue_id: b }, 0).unwrap();
    assert_eq!(
        shows(&e, "main"),
        Some("c".into()),
        "program falls forward to c"
    );
    assert_eq!(shows(&e, "room2"), Some("c".into()));
}

#[test]
fn test_patterns() {
    let mut e = engine();
    e.apply(
        &Action::SetTestPattern {
            pattern: Some(TestPattern::Grid),
        },
        0,
    )
    .unwrap();
    assert_eq!(e.live_state(0).test_pattern, Some(TestPattern::Grid));
    assert_eq!(
        e.apply(
            &Action::SetTestPattern {
                pattern: Some(TestPattern::Grid)
            },
            0
        ),
        Ok(crate::Change::NONE)
    );
}

#[test]
fn web_urls_are_validated() {
    let mut e = Engine::default();
    let web = |url: &str, zoom| WebInfo {
        url: url.into(),
        zoom,
        block_navigation: true,
        forward_keys: true,
        persist_session: false,
        openslides: false,
    };
    for (url, zoom, ok) in [
        ("https://example.org/slides/#/3", 100, true),
        ("http://192.168.1.10:8000/projector/1", 150, true),
        ("javascript:alert(1)", 100, false),
        ("file:///etc/passwd", 100, false),
        ("https://", 100, false),
        ("https://example.org", 10, false),
    ] {
        let r = e.apply(
            &Action::AddWeb {
                name: "".into(),
                web: web(url, zoom),
                at_index: None,
            },
            0,
        );
        assert_eq!(r.is_ok(), ok, "{url}");
    }
    assert_eq!(e.show_snapshot(false).cues[0].kind, CueKind::Web);
    assert_eq!(e.show().cues[0].name, "https://example.org/slides/#/3");
}

#[test]
fn capture_cues_and_lost_state() {
    let mut e = Engine::default();
    let window = CaptureSource::Window {
        app: "Firefox".into(),
        title: "".into(),
    };
    e.apply(
        &Action::AddCapture {
            name: "Browser".into(),
            source: window.clone(),
            at_index: None,
        },
        0,
    )
    .unwrap();
    let id = e.show().cues[0].id.clone();
    let blank = CaptureSource::Screen { name: " ".into() };
    assert_eq!(
        e.apply(
            &Action::SetCapture {
                cue_id: id.clone(),
                source: blank,
                fps: 30
            },
            0
        ),
        Err(ErrorCode::InvalidState)
    );
    assert_eq!(
        e.apply(
            &Action::SetCapture {
                cue_id: id.clone(),
                source: window,
                fps: 120
            },
            0
        ),
        Err(ErrorCode::InvalidState)
    );
    assert!(e.set_capture_lost(vec![id.clone(), id.clone()]).live);
    assert_eq!(e.live_state(0).capture_lost, vec![id.clone()]);
    assert_eq!(e.set_capture_lost(vec![id]), crate::Change::NONE);
}

#[test]
fn shows_without_outputs_get_the_main_output() {
    let json = r##"{"format_version":1,"id":"s","title":"t","cues":[]}"##;
    let show: Show = serde_json::from_str(json).unwrap();
    assert_eq!(show.outputs, vec![OutputDef::main()]);
}

#[test]
fn output_permissions() {
    let e = Engine::default();
    let ok = |role, a: Action| crate::permissions::check(role, false, &a, &e).is_ok();
    assert!(ok(Role::Operator, Action::SetTestPattern { pattern: None }));
    assert!(!ok(
        Role::Operator,
        Action::RemoveOutput {
            output_id: "x".into()
        }
    ));
    assert!(ok(
        Role::Admin,
        Action::SetCueTargets {
            cue_id: "c".into(),
            targets: None
        }
    ));
}

#[test]
fn web_cues_forward_next_and_prev_as_keys() {
    let mut e = Engine::default();
    let web = WebInfo {
        url: "https://example.org/reveal/".into(),
        zoom: 100,
        block_navigation: true,
        forward_keys: true,
        persist_session: false,
        openslides: false,
    };
    e.apply(
        &Action::AddWeb {
            name: "Deck".into(),
            web,
            at_index: None,
        },
        0,
    )
    .unwrap();
    e.insert_cues(vec![pdf("after", 1)], None);
    e.apply(&Action::Go, 0).unwrap();
    let web_id = e.show().cues[0].id.clone();
    assert_eq!(e.program().unwrap().cue_id, web_id);
    e.apply(&Action::Next, 0).unwrap();
    e.apply(&Action::Next, 0).unwrap();
    e.apply(&Action::Prev, 0).unwrap();
    let nav = e.live_state(0).web_nav.unwrap();
    assert_eq!(
        (nav.forward, nav.seq, nav.cue_id),
        (false, 3, web_id.clone())
    );
    assert_eq!(e.program().unwrap().cue_id, web_id, "still on the web cue");
    // Presenters can step through the page.
    assert!(crate::permissions::check(Role::Presenter, false, &Action::Next, &e).is_ok());
    // The cue is left with next cue.
    e.apply(&Action::NextCue, 0).unwrap();
    assert_eq!(
        e.show().cue(&e.program().unwrap().cue_id).unwrap().name,
        "after"
    );
}
