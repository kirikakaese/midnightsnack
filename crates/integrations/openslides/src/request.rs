// SPDX-License-Identifier: GPL-3.0-or-later
//! Autoupdate key requests: which fields of which objects the host subscribes to.

use serde_json::{json, Value};

/// A user's name parts.
fn user_fields() -> Value {
    json!({
        "id": null, "title": null, "first_name": null, "last_name": null, "username": null
    })
}

fn meeting_user() -> Value {
    json!({
        "type": "relation",
        "collection": "meeting_user",
        "fields": {
            "id": null,
            "user_id": { "type": "relation", "collection": "user", "fields": user_fields() }
        }
    })
}

/// The user's meetings (for the meeting picker).
pub fn meetings(user_id: u32) -> Value {
    json!([{
        "collection": "user",
        "ids": [user_id],
        "fields": {
            "id": null,
            "meeting_ids": {
                "type": "relation-list",
                "collection": "meeting",
                "fields": { "id": null, "name": null, "is_active_in_organization_id": null }
            }
        }
    }])
}

/// Everything an OpenSlides cue may show for one meeting.
pub fn meeting(meeting_id: u32) -> Value {
    json!([{
        "collection": "meeting",
        "ids": [meeting_id],
        "fields": {
            "id": null,
            "name": null,
            "reference_projector_id": null,
            "agenda_show_internal_items_on_projector": null,
            "agenda_item_ids": {
                "type": "relation-list",
                "collection": "agenda_item",
                "fields": {
                    "id": null, "item_number": null, "closed": null, "type": null,
                    "level": null, "weight": null, "parent_id": null, "content_object_id": null
                }
            },
            "motion_ids": {
                "type": "relation-list",
                "collection": "motion",
                "fields": {
                    "id": null, "number": null, "title": null, "text": null, "reason": null,
                    "additional_submitter": null, "list_of_speakers_id": null,
                    "sequential_number": null,
                    "state_id": {
                        "type": "relation",
                        "collection": "motion_state",
                        "fields": { "id": null, "name": null }
                    },
                    "submitter_ids": {
                        "type": "relation-list",
                        "collection": "motion_submitter",
                        "fields": { "id": null, "weight": null, "meeting_user_id": meeting_user() }
                    }
                }
            },
            "topic_ids": {
                "type": "relation-list",
                "collection": "topic",
                "fields": { "id": null, "title": null, "text": null, "list_of_speakers_id": null }
            },
            "assignment_ids": {
                "type": "relation-list",
                "collection": "assignment",
                "fields": { "id": null, "title": null }
            },
            "motion_block_ids": {
                "type": "relation-list",
                "collection": "motion_block",
                "fields": { "id": null, "title": null }
            },
            "list_of_speakers_ids": {
                "type": "relation-list",
                "collection": "list_of_speakers",
                "fields": {
                    "id": null, "closed": null, "content_object_id": null,
                    "speaker_ids": {
                        "type": "relation-list",
                        "collection": "speaker",
                        "fields": {
                            "id": null, "begin_time": null, "end_time": null, "weight": null,
                            "speech_state": null, "point_of_order": null,
                            "meeting_user_id": meeting_user()
                        }
                    }
                }
            },
            "projector_ids": {
                "type": "relation-list",
                "collection": "projector",
                "fields": {
                    "id": null, "name": null, "sequential_number": null,
                    "current_projection_ids": {
                        "type": "relation-list",
                        "collection": "projection",
                        "fields": {
                            "id": null, "content_object_id": null, "type": null,
                            "stable": null, "weight": null
                        }
                    }
                }
            }
        }
    }])
}
