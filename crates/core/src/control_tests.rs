// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for pointer drawings and the presenter rules around them.

use crate::engine::tests::two_cue_engine;
use crate::permissions::check;
use crate::protocol::*;

fn stroke(points: Vec<[f32; 2]>) -> Stroke {
    Stroke {
        color: "#ff0000".into(),
        width: 0.01,
        points,
    }
}

fn draw(e: &mut crate::Engine, s: Stroke) -> Result<(), ErrorCode> {
    e.apply(&Action::DrawStroke { stroke: s }, 0).map(|_| ())
}

#[test]
fn strokes_belong_to_the_slide_on_the_output() {
    let mut e = two_cue_engine();
    assert_eq!(
        draw(&mut e, stroke(vec![[0.1, 0.1]])),
        Err(ErrorCode::InvalidState),
        "nothing on the output"
    );
    e.apply(&Action::Go, 0).unwrap();
    draw(&mut e, stroke(vec![[0.1, 0.1], [0.2, 0.2]])).unwrap();
    draw(&mut e, stroke(vec![[0.5, 0.5]])).unwrap();
    let d = e.live_state(0).drawing.unwrap();
    assert_eq!(d.position, e.live_state(0).output.unwrap());
    assert_eq!(d.strokes.len(), 2);

    e.apply(&Action::Next, 0).unwrap();
    assert!(e.live_state(0).drawing.is_none(), "cleared on slide change");
}

#[test]
fn freeze_keeps_the_drawing_until_the_output_moves() {
    let mut e = two_cue_engine();
    e.apply(&Action::Go, 0).unwrap();
    draw(&mut e, stroke(vec![[0.3, 0.3]])).unwrap();
    e.apply(&Action::SetFreeze { on: true }, 0).unwrap();
    e.apply(&Action::Next, 0).unwrap();
    assert!(
        e.live_state(0).drawing.is_some(),
        "the audience still sees the slide"
    );
    e.apply(&Action::SetFreeze { on: false }, 0).unwrap();
    assert!(e.live_state(0).drawing.is_none());
}

#[test]
fn clear_and_limits() {
    let mut e = two_cue_engine();
    e.apply(&Action::Go, 0).unwrap();
    for _ in 0..(Drawing::MAX_STROKES + 5) {
        draw(&mut e, stroke(vec![[0.0, 0.0]])).unwrap();
    }
    assert_eq!(
        e.live_state(0).drawing.unwrap().strokes.len(),
        Drawing::MAX_STROKES
    );
    let change = e.apply(&Action::ClearDrawing, 0).unwrap();
    assert!(change.live);
    assert!(e.live_state(0).drawing.is_none());
    assert!(!e.apply(&Action::ClearDrawing, 0).unwrap().live, "no-op");
}

#[test]
fn strokes_are_validated_and_clamped() {
    let mut e = two_cue_engine();
    e.apply(&Action::Go, 0).unwrap();
    assert!(draw(&mut e, stroke(vec![])).is_err());
    assert!(draw(&mut e, stroke(vec![[f32::NAN, 0.0]])).is_err());
    assert!(draw(&mut e, stroke(vec![[0.0, 0.0]; Stroke::MAX_POINTS + 1])).is_err());
    let mut bad = stroke(vec![[0.0, 0.0]]);
    bad.color = "red; background:url(x)".into();
    assert!(draw(&mut e, bad).is_err());
    let mut wide = stroke(vec![[-1.0, 2.0]]);
    wide.width = 9.0;
    draw(&mut e, wide).unwrap();
    let s = &e.live_state(0).drawing.unwrap().strokes[0];
    assert_eq!(s.points[0], [0.0, 1.0]);
    assert_eq!(s.width, Stroke::MAX_WIDTH);
}

#[test]
fn presenters_draw_stage_viewers_do_not() {
    let mut e = two_cue_engine();
    e.apply(&Action::Go, 0).unwrap();
    let a = Action::DrawStroke {
        stroke: stroke(vec![[0.5, 0.5]]),
    };
    assert!(check(Role::Presenter, false, &a, &e).is_ok());
    assert!(check(Role::Presenter, false, &Action::ClearDrawing, &e).is_ok());
    assert_eq!(
        check(Role::StageViewer, false, &a, &e),
        Err(ErrorCode::Forbidden)
    );
}

#[test]
fn summary_describes_positions_for_control_surfaces() {
    let mut e = two_cue_engine();
    let s = e.summary(0);
    assert!(s.program.is_none());
    assert_eq!(s.cue_count, 2);
    e.apply(&Action::Go, 1000).unwrap();
    e.apply(&Action::SetBlackout { on: true }, 1000).unwrap();
    let s = e.summary(4000);
    let p = s.program.unwrap();
    assert_eq!((p.cue_number, p.slide), (1, 1));
    assert_eq!(s.next.unwrap().slide, 2);
    assert!(s.masters.blackout);
    assert_eq!(s.show_timer_ms, 3000);
    assert_eq!(s.countdown_remaining_ms, 5 * 60 * 1000);
    assert!(!s.countdown_running);
}
