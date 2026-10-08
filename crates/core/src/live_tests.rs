// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for media playback, text and timer cues, overlays, countdown and auto-advance.

use crate::engine::tests::{pdf, two_cue_engine};
use crate::model::{Cue, CueContent, MediaRef, Show};
use crate::permissions::check;
use crate::protocol::*;
use crate::{Change, Engine};

fn video(name: &str, options: MediaOptions, duration_ms: Option<u32>) -> Cue {
    Cue::new(
        name,
        CueContent::Media {
            file: MediaRef::linked(format!("/{name}.mp4")),
            video: true,
            options,
            duration_ms,
        },
    )
}

fn engine_with(cues: Vec<Cue>) -> Engine {
    Engine::new(Show {
        cues,
        ..Show::default()
    })
}

fn id(e: &Engine, i: usize) -> String {
    e.show().cues[i].id.clone()
}

fn overlay(kind: OverlayKind) -> Overlay {
    Overlay {
        id: "ov1".into(),
        name: "Lower third".into(),
        kind,
        position: OverlayPosition::BottomLeft,
        color: "#ffffff".into(),
        background: "#000000".into(),
        scale: 100,
    }
}

#[test]
fn media_starts_with_the_cue_and_follows_the_output() {
    let opts = MediaOptions {
        start_ms: 2_000,
        ..MediaOptions::default()
    };
    let mut e = engine_with(vec![pdf("a", 1), video("clip", opts, Some(10_000))]);
    e.apply(&Action::Next, 0).unwrap();
    assert!(e.live_state(0).media.is_none());

    e.apply(&Action::Next, 1_000).unwrap();
    let m = e.live_state(1_000).media.unwrap();
    assert_eq!(m.cue_id, id(&e, 1));
    assert_eq!(
        m.position.elapsed_ms(1_500),
        2_500,
        "starts at the trim point"
    );

    // Freezing on the video and moving on keeps it playing on the output.
    e.apply(&Action::SetFreeze { on: true }, 1_000).unwrap();
    e.apply(&Action::Prev, 1_200).unwrap();
    assert_eq!(e.live_state(0).media.unwrap().cue_id, id(&e, 1));
    e.apply(&Action::SetFreeze { on: false }, 1_300).unwrap();
    assert!(e.live_state(0).media.is_none());
}

#[test]
fn media_transport() {
    let mut e = engine_with(vec![video("clip", MediaOptions::default(), Some(10_000))]);
    assert_eq!(
        e.apply(&Action::MediaPause, 0),
        Err(ErrorCode::InvalidState)
    );
    e.apply(&Action::Next, 0).unwrap();
    e.apply(&Action::MediaPause, 3_000).unwrap();
    let pos = |e: &Engine, now| e.live_state(now).media.unwrap().position.elapsed_ms(now);
    assert_eq!(pos(&e, 9_000), 3_000);
    e.apply(&Action::MediaSeek { position_ms: 7_000 }, 9_000)
        .unwrap();
    assert_eq!(pos(&e, 9_500), 7_000, "seeking while paused stays paused");
    e.apply(&Action::MediaPlay, 10_000).unwrap();
    assert_eq!(pos(&e, 11_000), 8_000);
    e.apply(&Action::MediaRestart, 12_000).unwrap();
    assert_eq!(pos(&e, 12_500), 500);
}

#[test]
fn media_end_stops_or_advances() {
    let plain = MediaOptions::default();
    let advance = MediaOptions {
        auto_advance: true,
        ..MediaOptions::default()
    };
    let looping = MediaOptions {
        loop_playback: true,
        auto_advance: true,
        ..MediaOptions::default()
    };
    let mut e = engine_with(vec![
        video("a", plain, None),
        video("b", advance, None),
        video("c", looping, None),
        pdf("end", 1),
    ]);
    e.apply(&Action::Next, 0).unwrap();
    let a = id(&e, 0);
    // Reports from other cues are ignored.
    e.apply(&Action::MediaEnded { cue_id: id(&e, 1) }, 0)
        .unwrap();
    assert!(!e.live_state(0).media.unwrap().ended);
    e.apply(&Action::MediaEnded { cue_id: a.clone() }, 5_000)
        .unwrap();
    let m = e.live_state(6_000).media.unwrap();
    assert!(m.ended && !m.position.is_running());
    assert_eq!(e.program().unwrap().cue_id, a, "no auto-advance");

    e.apply(&Action::Next, 6_000).unwrap();
    e.apply(&Action::MediaEnded { cue_id: id(&e, 1) }, 9_000)
        .unwrap();
    assert_eq!(e.program().unwrap().cue_id, id(&e, 2), "auto-advanced");

    e.apply(&Action::MediaEnded { cue_id: id(&e, 2) }, 12_000)
        .unwrap();
    assert_eq!(
        e.program().unwrap().cue_id,
        id(&e, 2),
        "looping media never ends"
    );
}

#[test]
fn media_metadata_is_not_a_user_edit() {
    let mut e = engine_with(vec![video("a", MediaOptions::default(), None)]);
    e.mark_saved("/x.msnack".into());
    e.apply(
        &Action::MediaLoaded {
            cue_id: id(&e, 0),
            duration_ms: 4_200,
        },
        0,
    )
    .unwrap();
    assert!(!e.is_dirty());
    assert_eq!(
        e.show_snapshot(false).cues[0]
            .media
            .as_ref()
            .unwrap()
            .duration_ms,
        Some(4_200)
    );
}

#[test]
fn auto_advance_after_delay_and_at_media_end() {
    let mut e = engine_with(vec![
        pdf("a", 2),
        video(
            "v",
            MediaOptions {
                auto_advance: true,
                ..Default::default()
            },
            Some(3_000),
        ),
        pdf("b", 1),
    ]);
    let a = id(&e, 0);
    e.apply(
        &Action::SetCueAutoAdvance {
            cue_id: a,
            after_ms: Some(1_000),
        },
        0,
    )
    .unwrap();
    e.apply(&Action::Next, 10_000).unwrap();
    assert_eq!(e.auto_advance_at(), Some(11_000));
    assert_eq!(e.tick(10_500), Change::NONE);
    assert!(e.tick(11_000).live);
    assert_eq!(e.program().unwrap().slide, 1);
    e.tick(12_000);
    assert_eq!(e.program().unwrap().cue_id, id(&e, 1), "into the video");
    // Video: 3 s, auto-advance at the end.
    assert_eq!(e.auto_advance_at(), Some(15_000));
    e.apply(&Action::MediaPause, 13_000).unwrap();
    assert_eq!(e.auto_advance_at(), None, "paused media does not advance");
    e.apply(&Action::MediaPlay, 20_000).unwrap();
    assert_eq!(e.auto_advance_at(), Some(22_000));
    e.tick(22_000);
    assert_eq!(e.program().unwrap().cue_id, id(&e, 2));
    assert!(e.live_state(22_000).media.is_none());
}

#[test]
fn auto_advance_validation() {
    let mut e = two_cue_engine();
    let a = id(&e, 0);
    assert_eq!(
        e.apply(
            &Action::SetCueAutoAdvance {
                cue_id: a,
                after_ms: Some(5)
            },
            0
        ),
        Err(ErrorCode::InvalidState)
    );
}

#[test]
fn text_cues_split_and_edit() {
    let mut e = Engine::default();
    e.apply(
        &Action::AddText {
            name: "Song".into(),
            text: "One\ntwo\n\nThree\n\n\nFour".into(),
            lyrics: true,
            at_index: None,
        },
        0,
    )
    .unwrap();
    let cue = &e.show_snapshot(false).cues[0];
    assert_eq!(cue.kind, CueKind::Text);
    assert_eq!(cue.slide_count, 3);
    assert_eq!(cue.text.as_ref().unwrap().slides[0], "One\ntwo");

    let song = id(&e, 0);
    e.go_to(&song, 2, 0).unwrap();
    // Shrinking the text keeps the program on a valid slide.
    e.apply(
        &Action::SetCueText {
            cue_id: song.clone(),
            text: "Only\n---\nTwo".into(),
            lyrics: false,
        },
        0,
    )
    .unwrap();
    assert_eq!(e.program().unwrap().slide, 1);
    e.apply(
        &Action::SetCueText {
            cue_id: song,
            text: "   ".into(),
            lyrics: false,
        },
        0,
    )
    .unwrap();
    assert!(e.program().is_none(), "no slides left");
}

#[test]
fn themes_validate() {
    let mut e = Engine::default();
    e.apply(
        &Action::AddText {
            name: "".into(),
            text: "Hi".into(),
            lyrics: false,
            at_index: None,
        },
        0,
    )
    .unwrap();
    let cue = id(&e, 0);
    assert_eq!(e.show().cues[0].name, "Text");
    let bad = TextTheme {
        color: "red".into(),
        ..TextTheme::default()
    };
    assert_eq!(
        e.apply(
            &Action::SetCueTheme {
                cue_id: cue.clone(),
                theme: Some(bad)
            },
            0
        ),
        Err(ErrorCode::InvalidState)
    );
    let css = TextTheme {
        font_family: "x;}body{display:none".into(),
        ..TextTheme::default()
    };
    assert_eq!(
        e.apply(&Action::SetDefaultTheme { theme: css }, 0),
        Err(ErrorCode::InvalidState)
    );
    let missing = TextTheme {
        background_image: Some("nope".into()),
        ..TextTheme::default()
    };
    assert_eq!(
        e.apply(&Action::SetDefaultTheme { theme: missing }, 0),
        Err(ErrorCode::NotFound)
    );
    let good = TextTheme {
        font_size: Some(8),
        align: TextAlign::Left,
        ..TextTheme::default()
    };
    e.apply(
        &Action::SetCueTheme {
            cue_id: cue,
            theme: Some(good.clone()),
        },
        0,
    )
    .unwrap();
    assert_eq!(
        e.show_snapshot(false).cues[0].text.as_ref().unwrap().theme,
        Some(good)
    );
}

#[test]
fn timers_validate() {
    let mut e = Engine::default();
    let timer = |mode| TimerCue {
        mode,
        label: "Break".into(),
        overtime_color: "#ef4444".into(),
        theme: None,
    };
    for (mode, ok) in [
        (
            TimerMode::Countdown {
                duration_ms: 60_000,
            },
            true,
        ),
        (TimerMode::Countdown { duration_ms: 0 }, false),
        (
            TimerMode::CountdownTo {
                time: "19:30".into(),
            },
            true,
        ),
        (
            TimerMode::CountdownTo {
                time: "25:00".into(),
            },
            false,
        ),
        (
            TimerMode::CountdownTo {
                time: "7:30".into(),
            },
            false,
        ),
        (TimerMode::CountUp, true),
        (TimerMode::Clock, true),
    ] {
        let r = e.apply(
            &Action::AddTimer {
                name: "T".into(),
                timer: timer(mode.clone()),
                at_index: None,
            },
            0,
        );
        assert_eq!(r.is_ok(), ok, "{mode:?}");
    }
    assert_eq!(e.show().cues.len(), 4);
    assert_eq!(e.show_snapshot(false).cues[0].kind, CueKind::Timer);
}

#[test]
fn global_countdown() {
    let mut e = Engine::default();
    e.apply(
        &Action::CountdownSet {
            duration_ms: 120_000,
            label: "Q&A".into(),
        },
        0,
    )
    .unwrap();
    e.apply(&Action::CountdownStart, 1_000).unwrap();
    let c = e.live_state(31_000).countdown;
    assert_eq!(c.elapsed.elapsed_ms(31_000), 30_000);
    assert_eq!(c.label, "Q&A");
    e.apply(&Action::CountdownPause, 31_000).unwrap();
    e.apply(&Action::CountdownReset, 40_000).unwrap();
    assert_eq!(e.live_state(50_000).countdown.elapsed.elapsed_ms(50_000), 0);
    assert_eq!(
        e.apply(
            &Action::CountdownSet {
                duration_ms: 0,
                label: "".into()
            },
            0
        ),
        Err(ErrorCode::InvalidState)
    );
}

#[test]
fn overlays_are_defined_in_the_show_and_toggled_live() {
    let mut e = Engine::default();
    assert_eq!(
        e.apply(
            &Action::ToggleOverlay {
                overlay_id: "ov1".into()
            },
            0
        ),
        Err(ErrorCode::NotFound)
    );
    e.apply(
        &Action::PutOverlay {
            overlay: overlay(OverlayKind::LowerThird {
                title: "Ada".into(),
                subtitle: "Host".into(),
            }),
        },
        0,
    )
    .unwrap();
    e.apply(
        &Action::ToggleOverlay {
            overlay_id: "ov1".into(),
        },
        0,
    )
    .unwrap();
    assert_eq!(e.live_state(0).overlays_visible, vec!["ov1".to_owned()]);
    // Replacing keeps visibility; removing hides it.
    e.apply(
        &Action::PutOverlay {
            overlay: overlay(OverlayKind::Clock { seconds: true }),
        },
        0,
    )
    .unwrap();
    assert_eq!(e.show_snapshot(false).overlays.len(), 1);
    assert_eq!(e.live_state(0).overlays_visible.len(), 1);
    e.apply(
        &Action::RemoveOverlay {
            overlay_id: "ov1".into(),
        },
        0,
    )
    .unwrap();
    assert!(e.live_state(0).overlays_visible.is_empty());

    let bad = Overlay {
        color: "blue".into(),
        ..overlay(OverlayKind::Countdown)
    };
    assert_eq!(
        e.apply(&Action::PutOverlay { overlay: bad }, 0),
        Err(ErrorCode::InvalidState)
    );
    let fresh = Overlay {
        id: " ".into(),
        ..overlay(OverlayKind::Countdown)
    };
    e.apply(&Action::PutOverlay { overlay: fresh }, 0).unwrap();
    assert!(
        !e.show_snapshot(false).overlays[0].id.trim().is_empty(),
        "blank ids are generated"
    );
}

#[test]
fn assets_are_pruned_when_unused() {
    let mut e = Engine::default();
    let logo = e.add_asset(MediaRef::linked("/logo.png"));
    e.set_logo_asset(Some(logo.clone()));
    let bug = e.add_asset(MediaRef::linked("/bug.png"));
    e.apply(
        &Action::PutOverlay {
            overlay: overlay(OverlayKind::LogoBug {
                image: Some(bug.clone()),
            }),
        },
        0,
    )
    .unwrap();
    assert_eq!(e.show().assets.len(), 2);
    e.set_overlay_asset("ov1", None).unwrap();
    assert_eq!(e.show().assets.len(), 1);
    assert_eq!(e.show_snapshot(false).logo, Some(logo));
    e.set_logo_asset(None);
    assert!(e.show().assets.is_empty());
}

#[test]
fn stage_messages_are_cleaned() {
    let mut e = Engine::default();
    e.apply(
        &Action::SetStageMessage {
            text: Some("  2 minutes left\n\u{7}".into()),
        },
        0,
    )
    .unwrap();
    assert_eq!(
        e.live_state(0).stage_message.as_deref(),
        Some("2 minutes left")
    );
    e.apply(
        &Action::SetStageMessage {
            text: Some("   ".into()),
        },
        0,
    )
    .unwrap();
    assert_eq!(e.live_state(0).stage_message, None);
}

#[test]
fn transitions() {
    let mut e = two_cue_engine();
    let a = id(&e, 0);
    let fade = Transition {
        kind: TransitionKind::Fade,
        duration_ms: 400,
    };
    e.apply(
        &Action::SetCueTransition {
            cue_id: a,
            transition: Some(fade),
        },
        0,
    )
    .unwrap();
    e.apply(&Action::SetDefaultTransition { transition: fade }, 0)
        .unwrap();
    let snap = e.show_snapshot(false);
    assert_eq!(snap.cues[0].transition, Some(fade));
    assert_eq!(snap.default_transition, fade);
    let slow = Transition {
        kind: TransitionKind::Fade,
        duration_ms: 60_000,
    };
    assert_eq!(
        e.apply(&Action::SetDefaultTransition { transition: slow }, 0),
        Err(ErrorCode::InvalidState)
    );
}

#[test]
fn phase_two_permissions() {
    let e = Engine::default();
    let ok = |role, local, a: Action| check(role, local, &a, &e).is_ok();
    assert!(ok(Role::Operator, false, Action::MediaPause));
    assert!(ok(
        Role::Operator,
        false,
        Action::SetStageMessage { text: None }
    ));
    assert!(ok(
        Role::Operator,
        false,
        Action::ToggleOverlay {
            overlay_id: "x".into()
        }
    ));
    assert!(!ok(Role::Presenter, false, Action::MediaPause));
    assert!(!ok(
        Role::Operator,
        false,
        Action::PutOverlay {
            overlay: overlay(OverlayKind::Countdown)
        }
    ));
    // Playback reports and file paths only from the host itself.
    assert!(!ok(
        Role::Admin,
        false,
        Action::MediaEnded { cue_id: "c".into() }
    ));
    assert!(ok(
        Role::Operator,
        true,
        Action::MediaEnded { cue_id: "c".into() }
    ));
    assert!(!ok(
        Role::Admin,
        false,
        Action::SetLogoImage {
            path: Some("/x.png".into())
        }
    ));
    assert!(ok(Role::Admin, false, Action::SetLogoImage { path: None }));
}
