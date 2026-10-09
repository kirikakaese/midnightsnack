// SPDX-License-Identifier: GPL-3.0-or-later
//! The adapter against the mock OpenSlides server: login, meetings, the meeting view, live
//! updates, token renewal, public access and errors.

use std::time::Duration;

use midnightsnack_openslides::mock::{demo_meeting, MockOpenSlides, MockOptions};
use midnightsnack_openslides::{pages, run, view, Config, Store, Update};
use midnightsnack_protocol::*;
use serde_json::json;
use tokio::sync::mpsc;

fn config(url: &str, user: &str, pass: &str) -> Config {
    Config {
        url: url.into(),
        username: user.into(),
        password: pass.into(),
    }
}

async fn next_matching(rx: &mut mpsc::Receiver<Update>, pred: impl Fn(&Update) -> bool) -> Update {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let u = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .expect("timed out waiting for an update")
            .expect("channel open");
        if pred(&u) {
            return u;
        }
    }
}

fn data(u: Update) -> OsMeetingData {
    match u {
        Update::Data(Some(d)) => d,
        other => panic!("expected data, got {other:?}"),
    }
}

#[test]
fn demo_meeting_view() {
    let mut store = Store::new();
    store.apply(demo_meeting().into_iter().collect());
    let d = view::meeting(&store, 1).unwrap();
    assert_eq!(d.name, "Delegates assembly 2026");
    let agenda: Vec<(&str, &str, u32)> = d
        .agenda
        .iter()
        .map(|a| (a.number.as_str(), a.title.as_str(), a.level))
        .collect();
    assert_eq!(
        agenda,
        vec![
            ("TOP 1", "Welcome and opening", 0),
            ("TOP 2", "A1 · Climate budget", 0),
            ("TOP 2.1", "A2 · Amendment: 3% instead of 2%", 1),
        ],
        "internal items stay off the projector"
    );
    assert!(d.agenda[0].closed);

    let m = &d.motions[0];
    assert_eq!(m.number, "A1");
    assert_eq!(m.submitters, vec!["Ada Lovelace", "Dr. Grace Hopper"]);
    assert_eq!(m.state.as_deref(), Some("submitted"));
    let kinds: Vec<OsBlockKind> = m.body.iter().map(|b| b.kind).collect();
    assert_eq!(
        kinds,
        vec![
            OsBlockKind::Paragraph,
            OsBlockKind::ListItem,
            OsBlockKind::ListItem,
            OsBlockKind::ReasonHeading,
            OsBlockKind::Paragraph
        ]
    );
    assert_eq!(m.page_starts, vec![0]);

    let list = d.lists.iter().find(|l| l.id == 2).unwrap();
    assert_eq!(list.title, "A1 · Climate budget");
    let order: Vec<(&str, OsSpeakerState)> = list
        .speakers
        .iter()
        .map(|s| (s.name.as_str(), s.state))
        .collect();
    assert_eq!(
        order,
        vec![
            ("Ada Lovelace", OsSpeakerState::Finished),
            ("Dr. Grace Hopper", OsSpeakerState::Speaking),
            ("Alan Turing", OsSpeakerState::Waiting),
        ]
    );
    assert_eq!(list.speakers[1].begin_ms, Some(1_760_000_130_000));
    assert_eq!(list.speakers[1].speech_state.as_deref(), Some("pro"));

    assert_eq!(
        d.projectors[0].current,
        Some(OsProjected::Motion { motion_id: 1 }),
        "the stable clock projection is not the main one"
    );
    assert_eq!(d.projectors[1].current, Some(OsProjected::Agenda));
    assert_eq!(d.current_list_id, Some(2));
    assert!(view::meeting(&store, 99).is_none());
}

#[test]
fn long_content_is_split_into_pages() {
    let mut store = Store::new();
    store.apply(demo_meeting().into_iter().collect());
    let ids: Vec<u32> = (100..130).collect();
    let mut changes = serde_json::Map::new();
    changes.insert("meeting/1/agenda_item_ids".into(), json!(ids));
    for id in &ids {
        changes.insert(format!("agenda_item/{id}/id"), json!(id));
        changes.insert(format!("agenda_item/{id}/weight"), json!(id));
        changes.insert(
            format!("agenda_item/{id}/content_object_id"),
            json!("topic/1"),
        );
    }
    let long: String = (0..12)
        .map(|i| format!("<p>{} {i}</p>", "word ".repeat(40)))
        .collect();
    changes.insert("motion/1/text".into(), json!(long));
    store.apply(changes);
    let d = view::meeting(&store, 1).unwrap();
    assert_eq!(d.agenda.len(), 30);
    assert_eq!(pages(Some(&d), &OpenSlidesSlide::Agenda), 3);
    let m = &d.motions[0];
    assert!(m.page_starts.len() >= 3, "{:?}", m.page_starts);
    assert_eq!(
        pages(Some(&d), &OpenSlidesSlide::Motion { motion_id: 1 }),
        m.page_starts.len() as u32
    );
    assert_eq!(pages(None, &OpenSlidesSlide::Agenda), 1);
    assert_eq!(
        pages(Some(&d), &OpenSlidesSlide::Motion { motion_id: 9 }),
        1
    );
}

#[tokio::test]
async fn live_meeting_with_login_and_updates() {
    let mock = MockOpenSlides::start(MockOptions::default()).await.unwrap();
    let (tx, mut rx) = mpsc::channel(64);
    let task = tokio::spawn(run(config(&mock.url, "admin", "admin"), Some(1), tx));

    let meetings = next_matching(&mut rx, |u| matches!(u, Update::Meetings(_))).await;
    assert_eq!(
        meetings,
        Update::Meetings(vec![OsMeetingRef {
            id: 1,
            name: "Delegates assembly 2026".into()
        }])
    );
    let first = data(next_matching(&mut rx, |u| matches!(u, Update::Data(_))).await);
    assert_eq!(first.motions.len(), 2);
    next_matching(&mut rx, |u| {
        *u == Update::State(OpenSlidesState::Connected, None)
    })
    .await;

    // The next speaker starts in OpenSlides.
    mock.set(json!({"speaker/32/end_time": 1_760_000_200, "speaker/33/begin_time": 1_760_000_201}));
    let updated = data(next_matching(&mut rx, |u| matches!(u, Update::Data(_))).await);
    let list = updated.lists.iter().find(|l| l.id == 2).unwrap();
    assert_eq!(list.speakers[2].name, "Alan Turing");
    assert_eq!(list.speakers[2].state, OsSpeakerState::Speaking);

    // The OpenSlides operator projects the agenda instead.
    mock.set(json!({"projection/41/content_object_id": "meeting/1", "projection/41/type": "agenda_item_list"}));
    let updated = data(next_matching(&mut rx, |u| matches!(u, Update::Data(_))).await);
    assert_eq!(updated.projectors[0].current, Some(OsProjected::Agenda));
    assert_eq!(updated.current_list_id, None);
    task.abort();
}

#[tokio::test]
async fn tokens_are_renewed_without_logging_in_again() {
    let mock = MockOpenSlides::start(MockOptions {
        // Renewal happens 45 s before expiry, at least 5 s after connecting.
        token_ttl: Duration::from_secs(50),
        ..MockOptions::default()
    })
    .await
    .unwrap();
    let (tx, mut rx) = mpsc::channel(64);
    let task = tokio::spawn(run(config(&mock.url, "admin", "admin"), Some(1), tx));
    next_matching(&mut rx, |u| {
        *u == Update::State(OpenSlidesState::Connected, None)
    })
    .await;
    // After the renewal the stream is open again and still receives changes.
    tokio::time::sleep(Duration::from_secs(7)).await;
    mock.set(json!({"motion/2/title": "Amendment: 4%"}));
    let d = data(
        next_matching(
            &mut rx,
            |u| matches!(u, Update::Data(Some(d)) if d.motions[1].title == "Amendment: 4%"),
        )
        .await,
    );
    assert_eq!(d.motions[1].number, "A2");
    assert_eq!(
        mock.login_count(),
        1,
        "the refresh cookie was used, not the password"
    );
    task.abort();
}

#[tokio::test]
async fn wrong_password_unknown_meeting_and_public_access() {
    let mock = MockOpenSlides::start(MockOptions::default()).await.unwrap();
    let (tx, mut rx) = mpsc::channel(64);
    let task = tokio::spawn(run(config(&mock.url, "admin", "nope"), Some(1), tx));
    next_matching(&mut rx, |u| {
        *u == Update::State(OpenSlidesState::Failed, Some(OpenSlidesError::Unauthorized))
    })
    .await;
    task.abort();

    let (tx, mut rx) = mpsc::channel(64);
    let task = tokio::spawn(run(config(&mock.url, "admin", "admin"), Some(99), tx));
    next_matching(&mut rx, |u| *u == Update::Data(None)).await;
    next_matching(&mut rx, |u| {
        *u == Update::State(
            OpenSlidesState::Failed,
            Some(OpenSlidesError::MeetingNotFound),
        )
    })
    .await;
    task.abort();

    let (tx, mut rx) = mpsc::channel(64);
    let task = tokio::spawn(run(config(&mock.url, "admin", "admin"), None, tx));
    next_matching(&mut rx, |u| {
        *u == Update::State(OpenSlidesState::SelectMeeting, None)
    })
    .await;
    task.abort();

    // Anonymous access is refused by this server…
    let (tx, mut rx) = mpsc::channel(64);
    let task = tokio::spawn(run(config(&mock.url, "", ""), Some(1), tx));
    next_matching(&mut rx, |u| {
        *u == Update::State(OpenSlidesState::Failed, Some(OpenSlidesError::Unauthorized))
    })
    .await;
    task.abort();

    // …and allowed by this one.
    let public = MockOpenSlides::start(MockOptions {
        anonymous: true,
        ..MockOptions::default()
    })
    .await
    .unwrap();
    let (tx, mut rx) = mpsc::channel(64);
    let task = tokio::spawn(run(config(&public.url, "", ""), Some(1), tx));
    let d = data(next_matching(&mut rx, |u| matches!(u, Update::Data(_))).await);
    assert_eq!(d.name, "Delegates assembly 2026");
    task.abort();

    // Not an OpenSlides server at all.
    let (tx, mut rx) = mpsc::channel(64);
    let task = tokio::spawn(run(config("http://127.0.0.1:9", "a", "b"), Some(1), tx));
    next_matching(&mut rx, |u| {
        *u == Update::State(OpenSlidesState::Failed, Some(OpenSlidesError::Unreachable))
    })
    .await;
    task.abort();
}
