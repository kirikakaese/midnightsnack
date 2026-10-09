// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenSlides cues: adding them, page counts from live data, permissions.

use crate::engine::Engine;
use crate::permissions::check;
use crate::protocol::*;

fn add(e: &mut Engine, slide: OpenSlidesSlide) {
    e.apply(
        &Action::AddOpenSlides {
            name: String::new(),
            slide,
            at_index: None,
        },
        0,
    )
    .unwrap();
}

#[test]
fn cues_get_names_and_pages_from_data_without_becoming_edits() {
    let mut e = Engine::default();
    add(&mut e, OpenSlidesSlide::Agenda);
    add(&mut e, OpenSlidesSlide::Motion { motion_id: 4 });
    add(&mut e, OpenSlidesSlide::Speakers { list_id: None });
    let show = e.show_snapshot(true);
    let names: Vec<&str> = show.cues.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Agenda", "Motion 4", "List of speakers"]);
    assert_eq!(
        show.cues[1].openslides,
        Some(OpenSlidesCue {
            slide: OpenSlidesSlide::Motion { motion_id: 4 },
            theme: None
        })
    );
    assert!(show.cues.iter().all(|c| c.slide_count == 1));

    e.mark_saved("show.msnack".into());
    let pages = |s: &OpenSlidesSlide| match s {
        OpenSlidesSlide::Motion { .. } => 3,
        _ => 1,
    };
    let change = e.set_openslides_pages(0, pages);
    assert!(change.show);
    assert_eq!(e.show_snapshot(true).cues[1].slide_count, 3);
    assert!(!e.is_dirty(), "page counts are not an edit");
    assert_eq!(e.set_openslides_pages(0, pages), crate::Change::NONE);

    // Live on page 3 of the motion; the motion gets shorter.
    let cue = e.show_snapshot(true).cues[1].id.clone();
    e.apply(
        &Action::GoTo {
            position: Position {
                cue_id: cue.clone(),
                slide: 2,
            },
        },
        0,
    )
    .unwrap();
    let change = e.set_openslides_pages(0, |_| 1);
    assert!(
        change.live,
        "the live position is moved back onto the content"
    );
    let live = e.live_state(0);
    assert_eq!(
        live.program.unwrap(),
        Position {
            cue_id: cue,
            slide: 0
        }
    );
}

#[test]
fn only_admins_add_or_configure() {
    let e = Engine::default();
    let add = Action::AddOpenSlides {
        name: String::new(),
        slide: OpenSlidesSlide::Agenda,
        at_index: None,
    };
    let configure = Action::ConfigureOpenSlides {
        enabled: true,
        url: "https://os.example.org".into(),
        username: String::new(),
        password: None,
        meeting_id: None,
    };
    for a in [&add, &configure] {
        assert_eq!(
            check(Role::Operator, false, a, &e),
            Err(ErrorCode::Forbidden)
        );
        assert!(check(Role::Admin, false, a, &e).is_ok());
    }
}

#[test]
fn old_show_files_without_pages_load() {
    let json = r#"{"type":"open_slides","slide":{"kind":"agenda"}}"#;
    let content: crate::CueContent = serde_json::from_str(json).unwrap();
    assert_eq!(content.slide_count(), 1);
}
