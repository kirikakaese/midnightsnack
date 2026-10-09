// SPDX-License-Identifier: GPL-3.0-or-later
//! Turns the raw autoupdate store into the meeting data midnightsnack draws, and splits long
//! content into pages.

use std::collections::HashMap;

use midnightsnack_protocol::{
    OpenSlidesSlide, OsAgendaItem, OsAgendaType, OsBlock, OsBlockKind, OsMeetingData, OsMeetingRef,
    OsMotion, OsProjected, OsProjector, OsSpeaker, OsSpeakerList, OsSpeakerState, OsTopic,
    OS_AGENDA_PAGE_SIZE,
};

use crate::html;
use crate::store::Store;

/// Text characters per motion/topic page (the first page also holds the title).
pub const PAGE_CHARS: usize = 900;
const FIRST_PAGE_CHARS: usize = 600;
/// A block counts at least this many characters (headings and short lines take room too).
const MIN_BLOCK_COST: usize = 80;

/// The user's meetings, active ones only, sorted by name.
pub fn meetings(store: &Store, user_id: u32) -> Vec<OsMeetingRef> {
    let mut v: Vec<OsMeetingRef> = store
        .ids("user", user_id, "meeting_ids")
        .into_iter()
        .filter(|&id| {
            store
                .get("meeting", id, "is_active_in_organization_id")
                .is_some()
        })
        .map(|id| OsMeetingRef {
            id,
            name: store.str("meeting", id, "name"),
        })
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    v
}

/// `Title First Last`, or the username.
fn user_name(store: &Store, meeting_user_id: u32) -> Option<String> {
    let user = store.u32("meeting_user", meeting_user_id, "user_id")?;
    let name = ["title", "first_name", "last_name"]
        .iter()
        .map(|f| store.str("user", user, f))
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Some(if name.is_empty() {
        store.str("user", user, "username")
    } else {
        name
    })
}

/// Number and title of a motion, e.g. `A1 Climate budget`.
fn motion_title(store: &Store, id: u32) -> String {
    let number = store.str("motion", id, "number");
    let title = store.str("motion", id, "title");
    if number.is_empty() {
        title
    } else {
        format!("{number} · {title}")
    }
}

/// Title of the object an agenda item or list of speakers belongs to.
fn object_title(store: &Store, collection: &str, id: u32) -> String {
    match collection {
        "motion" => motion_title(store, id),
        "topic" | "assignment" | "motion_block" => store.str(collection, id, "title"),
        _ => String::new(),
    }
}

/// Splits blocks into pages by a character budget. Returns start indices (`[0]` at least).
pub fn paginate(blocks: &[OsBlock]) -> Vec<u32> {
    let mut starts = vec![0u32];
    let mut used = 0usize;
    let mut budget = FIRST_PAGE_CHARS;
    for (i, b) in blocks.iter().enumerate() {
        let cost = b.text.chars().count().max(MIN_BLOCK_COST);
        if used > 0 && used + cost > budget {
            starts.push(i as u32);
            used = 0;
            budget = PAGE_CHARS;
        }
        used += cost;
    }
    starts
}

fn agenda(store: &Store, meeting: u32) -> Vec<OsAgendaItem> {
    let show_internal = store.bool(
        "meeting",
        meeting,
        "agenda_show_internal_items_on_projector",
    );
    let ids: Vec<u32> = store
        .ids("meeting", meeting, "agenda_item_ids")
        .into_iter()
        .filter(|&id| store.exists("agenda_item", id))
        .collect();
    // Depth-first by weight, like OpenSlides' agenda tree.
    let mut children: HashMap<Option<u32>, Vec<u32>> = HashMap::new();
    for &id in &ids {
        let parent = store
            .u32("agenda_item", id, "parent_id")
            .filter(|p| ids.contains(p));
        children.entry(parent).or_default().push(id);
    }
    for list in children.values_mut() {
        list.sort_by_key(|&id| (store.i64("agenda_item", id, "weight").unwrap_or(0), id));
    }
    let mut order = Vec::new();
    let mut stack: Vec<(u32, u32)> = children
        .get(&None)
        .map(|v| v.iter().rev().map(|&id| (id, 0)).collect())
        .unwrap_or_default();
    while let Some((id, depth)) = stack.pop() {
        order.push((id, depth));
        if let Some(kids) = children.get(&Some(id)) {
            stack.extend(kids.iter().rev().map(|&k| (k, depth + 1)));
        }
    }
    order
        .into_iter()
        .filter_map(|(id, depth)| {
            let kind = match store.str("agenda_item", id, "type").as_str() {
                "internal" => OsAgendaType::Internal,
                "hidden" => OsAgendaType::Hidden,
                _ => OsAgendaType::Common,
            };
            let shown =
                kind == OsAgendaType::Common || (kind == OsAgendaType::Internal && show_internal);
            if !shown {
                return None;
            }
            let title = store
                .fqid("agenda_item", id, "content_object_id")
                .map(|(c, oid)| object_title(store, &c, oid))
                .unwrap_or_default();
            Some(OsAgendaItem {
                id,
                number: store.str("agenda_item", id, "item_number"),
                title,
                level: store.u32("agenda_item", id, "level").unwrap_or(depth),
                closed: store.bool("agenda_item", id, "closed"),
                kind,
            })
        })
        .collect()
}

fn motions(store: &Store, meeting: u32) -> Vec<OsMotion> {
    let mut ids: Vec<u32> = store
        .ids("meeting", meeting, "motion_ids")
        .into_iter()
        .filter(|&id| store.exists("motion", id))
        .collect();
    ids.sort_by_key(|&id| {
        (
            store.u32("motion", id, "sequential_number").unwrap_or(id),
            id,
        )
    });
    ids.into_iter()
        .map(|id| {
            let mut submitter_ids = store.ids("motion", id, "submitter_ids");
            submitter_ids
                .sort_by_key(|&s| (store.i64("motion_submitter", s, "weight").unwrap_or(0), s));
            let mut submitters: Vec<String> = submitter_ids
                .into_iter()
                .filter_map(|s| store.u32("motion_submitter", s, "meeting_user_id"))
                .filter_map(|mu| user_name(store, mu))
                .collect();
            let additional = store.str("motion", id, "additional_submitter");
            if !additional.trim().is_empty() {
                submitters.push(additional.trim().to_owned());
            }
            let mut body = html::blocks(&store.str("motion", id, "text"));
            let reason = html::blocks(&store.str("motion", id, "reason"));
            if !reason.is_empty() {
                body.push(OsBlock {
                    kind: OsBlockKind::ReasonHeading,
                    text: String::new(),
                });
                body.extend(reason);
            }
            OsMotion {
                id,
                number: store.str("motion", id, "number"),
                title: store.str("motion", id, "title"),
                state: store
                    .u32("motion", id, "state_id")
                    .map(|s| store.str("motion_state", s, "name"))
                    .filter(|s| !s.is_empty()),
                submitters,
                page_starts: paginate(&body),
                body,
                list_of_speakers_id: store.u32("motion", id, "list_of_speakers_id"),
            }
        })
        .collect()
}

fn topics(store: &Store, meeting: u32) -> Vec<OsTopic> {
    store
        .ids("meeting", meeting, "topic_ids")
        .into_iter()
        .filter(|&id| store.exists("topic", id))
        .map(|id| {
            let body = html::blocks(&store.str("topic", id, "text"));
            OsTopic {
                id,
                title: store.str("topic", id, "title"),
                page_starts: paginate(&body),
                body,
                list_of_speakers_id: store.u32("topic", id, "list_of_speakers_id"),
            }
        })
        .collect()
}

fn speaker_lists(store: &Store, meeting: u32) -> Vec<OsSpeakerList> {
    store
        .ids("meeting", meeting, "list_of_speakers_ids")
        .into_iter()
        .filter(|&id| store.exists("list_of_speakers", id))
        .map(|id| {
            let title = store
                .fqid("list_of_speakers", id, "content_object_id")
                .map(|(c, oid)| object_title(store, &c, oid))
                .unwrap_or_default();
            let mut finished = Vec::new();
            let mut speaking = Vec::new();
            let mut waiting = Vec::new();
            for sid in store.ids("list_of_speakers", id, "speaker_ids") {
                if !store.exists("speaker", sid) {
                    continue;
                }
                let name = store
                    .u32("speaker", sid, "meeting_user_id")
                    .and_then(|mu| user_name(store, mu))
                    .unwrap_or_default();
                let begin = store.i64("speaker", sid, "begin_time");
                let end = store.i64("speaker", sid, "end_time");
                let state = match (begin, end) {
                    (_, Some(_)) => OsSpeakerState::Finished,
                    (Some(_), None) => OsSpeakerState::Speaking,
                    (None, None) => OsSpeakerState::Waiting,
                };
                let point_of_order = store.bool("speaker", sid, "point_of_order");
                let speech_state =
                    Some(store.str("speaker", sid, "speech_state")).filter(|s| !s.is_empty());
                let speaker = OsSpeaker {
                    name,
                    state,
                    speech_state,
                    point_of_order,
                    // OpenSlides timestamps are in seconds.
                    begin_ms: begin.map(|t| t * 1000),
                    end_ms: end.map(|t| t * 1000),
                };
                let weight = store.i64("speaker", sid, "weight").unwrap_or(0);
                match state {
                    OsSpeakerState::Finished => finished.push((end.unwrap_or(0), sid, speaker)),
                    OsSpeakerState::Speaking => speaking.push((begin.unwrap_or(0), sid, speaker)),
                    OsSpeakerState::Waiting => {
                        // Points of order go first, then by weight.
                        waiting.push(((i64::from(!point_of_order), weight), sid, speaker))
                    }
                }
            }
            finished.sort_by_key(|s| (s.0, s.1));
            speaking.sort_by_key(|s| (s.0, s.1));
            waiting.sort_by_key(|s| (s.0, s.1));
            let speakers = finished
                .into_iter()
                .map(|s| s.2)
                .chain(speaking.into_iter().map(|s| s.2))
                .chain(waiting.into_iter().map(|s| s.2))
                .collect();
            OsSpeakerList {
                id,
                title,
                closed: store.bool("list_of_speakers", id, "closed"),
                speakers,
            }
        })
        .collect()
}

/// The main (non-stable) projection of a projector, resolved.
fn projected(store: &Store, projector: u32) -> Option<OsProjected> {
    let mut ids: Vec<u32> = store
        .ids("projector", projector, "current_projection_ids")
        .into_iter()
        .filter(|&p| store.exists("projection", p) && !store.bool("projection", p, "stable"))
        .collect();
    ids.sort_by_key(|&p| (store.i64("projection", p, "weight").unwrap_or(0), p));
    let p = *ids.first()?;
    let (collection, id) = store.fqid("projection", p, "content_object_id")?;
    let kind = store.str("projection", p, "type");
    Some(resolve(store, &collection, id, &kind))
}

fn resolve(store: &Store, collection: &str, id: u32, kind: &str) -> OsProjected {
    match (collection, kind) {
        ("motion", _) => OsProjected::Motion { motion_id: id },
        ("topic", _) => OsProjected::Topic { topic_id: id },
        ("list_of_speakers", _) => OsProjected::Speakers { list_id: id },
        ("meeting", "agenda_item_list") => OsProjected::Agenda,
        ("meeting", "current_los") => OsProjected::CurrentSpeakers,
        ("agenda_item", _) => match store.fqid("agenda_item", id, "content_object_id") {
            Some((c, oid)) if c == "motion" || c == "topic" => resolve(store, &c, oid, ""),
            _ => OsProjected::Agenda,
        },
        _ => OsProjected::Other {
            collection: collection.to_owned(),
            title: object_title(store, collection, id),
        },
    }
}

fn list_of(store: &Store, p: &OsProjected) -> Option<u32> {
    match p {
        OsProjected::Motion { motion_id } => store.u32("motion", *motion_id, "list_of_speakers_id"),
        OsProjected::Topic { topic_id } => store.u32("topic", *topic_id, "list_of_speakers_id"),
        OsProjected::Speakers { list_id } => Some(*list_id),
        _ => None,
    }
}

/// The meeting as midnightsnack draws it; `None` while the meeting is not in the store.
pub fn meeting(store: &Store, meeting: u32) -> Option<OsMeetingData> {
    if !store.exists("meeting", meeting) {
        return None;
    }
    let mut projector_ids: Vec<u32> = store
        .ids("meeting", meeting, "projector_ids")
        .into_iter()
        .filter(|&p| store.exists("projector", p))
        .collect();
    projector_ids.sort_by_key(|&p| {
        (
            store.u32("projector", p, "sequential_number").unwrap_or(p),
            p,
        )
    });
    let projectors: Vec<OsProjector> = projector_ids
        .into_iter()
        .map(|id| OsProjector {
            id,
            name: store.str("projector", id, "name"),
            current: projected(store, id),
        })
        .collect();
    let reference = store.u32("meeting", meeting, "reference_projector_id");
    let current_list_id = reference
        .and_then(|r| projectors.iter().find(|p| p.id == r))
        .and_then(|p| p.current.as_ref())
        .and_then(|p| list_of(store, p));
    Some(OsMeetingData {
        meeting_id: meeting,
        name: store.str("meeting", meeting, "name"),
        agenda: agenda(store, meeting),
        motions: motions(store, meeting),
        topics: topics(store, meeting),
        lists: speaker_lists(store, meeting),
        projectors,
        reference_projector_id: reference,
        current_list_id,
    })
}

/// Pages of an OpenSlides cue for the given data (1 if unknown).
pub fn pages(data: Option<&OsMeetingData>, slide: &OpenSlidesSlide) -> u32 {
    let Some(data) = data else { return 1 };
    let n = match slide {
        OpenSlidesSlide::Agenda => data.agenda.len().div_ceil(OS_AGENDA_PAGE_SIZE),
        OpenSlidesSlide::Motion { motion_id } => data
            .motions
            .iter()
            .find(|m| m.id == *motion_id)
            .map_or(1, |m| m.page_starts.len()),
        OpenSlidesSlide::Topic { topic_id } => data
            .topics
            .iter()
            .find(|t| t.id == *topic_id)
            .map_or(1, |t| t.page_starts.len()),
        OpenSlidesSlide::Speakers { .. } | OpenSlidesSlide::Follow { .. } => 1,
    };
    n.max(1) as u32
}
