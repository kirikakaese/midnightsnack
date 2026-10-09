// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenSlides meeting data as midnightsnack draws it, and the settings of the connection.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Agenda entries per page of an agenda cue.
pub const OS_AGENDA_PAGE_SIZE: usize = 12;

/// What an OpenSlides cue shows.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum OpenSlidesSlide {
    /// The agenda (common items).
    Agenda,
    Motion {
        motion_id: u32,
    },
    Topic {
        topic_id: u32,
    },
    /// A list of speakers; `None` = the current one (of what the reference projector shows).
    Speakers {
        list_id: Option<u32>,
    },
    /// Whatever an OpenSlides projector currently shows; `None` = the reference projector.
    Follow {
        projector_id: Option<u32>,
    },
}

/// An OpenSlides cue as clients see it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OpenSlidesCue {
    pub slide: OpenSlidesSlide,
    /// Per-cue theme; `None` uses the show default.
    pub theme: Option<crate::TextTheme>,
}

/// A text block converted from OpenSlides' HTML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OsBlock {
    pub kind: OsBlockKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OsBlockKind {
    Paragraph,
    Heading,
    ListItem,
    /// Starts a motion's reason (clients show a translated "Reason" heading; `text` is empty).
    ReasonHeading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OsAgendaType {
    Common,
    Internal,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OsAgendaItem {
    pub id: u32,
    /// e.g. `TOP 1.2`; empty if numbering is off.
    pub number: String,
    pub title: String,
    /// Nesting depth (0 = top level).
    pub level: u32,
    pub closed: bool,
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub kind: OsAgendaType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OsMotion {
    pub id: u32,
    pub number: String,
    pub title: String,
    pub state: Option<String>,
    pub submitters: Vec<String>,
    /// Motion text, then (under a heading) the reason.
    pub body: Vec<OsBlock>,
    /// Index into `body` where each page starts (`[0]` for one page).
    pub page_starts: Vec<u32>,
    pub list_of_speakers_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OsTopic {
    pub id: u32,
    pub title: String,
    pub body: Vec<OsBlock>,
    /// Index into `body` where each page starts (`[0]` for one page).
    pub page_starts: Vec<u32>,
    pub list_of_speakers_id: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OsSpeakerState {
    Waiting,
    Speaking,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OsSpeaker {
    pub name: String,
    pub state: OsSpeakerState,
    /// `pro`, `contra`, `contribution`, `intervention`, `interposed_question` or none.
    pub speech_state: Option<String>,
    pub point_of_order: bool,
    /// When the speech started / ended (Unix ms, OpenSlides server clock).
    #[ts(type = "number | null")]
    pub begin_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub end_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OsSpeakerList {
    pub id: u32,
    /// Title of what the list belongs to (motion, topic…).
    pub title: String,
    pub closed: bool,
    /// Finished speakers (oldest first), the current one, then waiting ones in order.
    pub speakers: Vec<OsSpeaker>,
}

/// Something an OpenSlides projector shows, resolved to what midnightsnack can draw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum OsProjected {
    Agenda,
    Motion {
        motion_id: u32,
    },
    Topic {
        topic_id: u32,
    },
    Speakers {
        list_id: u32,
    },
    /// The current list of speakers (follows the reference projector).
    CurrentSpeakers,
    /// Not drawn natively (elections, polls, files, messages…).
    Other {
        collection: String,
        title: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OsProjector {
    pub id: u32,
    pub name: String,
    /// The main projection, if any (stable overlays like the clock are left out).
    pub current: Option<OsProjected>,
}

/// The meeting as midnightsnack shows it. Sent to every client.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OsMeetingData {
    pub meeting_id: u32,
    pub name: String,
    pub agenda: Vec<OsAgendaItem>,
    pub motions: Vec<OsMotion>,
    pub topics: Vec<OsTopic>,
    pub lists: Vec<OsSpeakerList>,
    pub projectors: Vec<OsProjector>,
    pub reference_projector_id: Option<u32>,
    /// The list of speakers of what the reference projector shows.
    pub current_list_id: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OpenSlidesState {
    Off,
    Connecting,
    /// Logged in, no meeting selected yet.
    SelectMeeting,
    Connected,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OpenSlidesError {
    InvalidUrl,
    /// DNS, TCP or TLS failure, or not an OpenSlides server.
    Unreachable,
    /// Wrong username or password, or no access to the meeting.
    Unauthorized,
    /// The meeting does not exist or is not visible to this account.
    MeetingNotFound,
    /// The server answered something the adapter does not understand.
    Incompatible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OsMeetingRef {
    pub id: u32,
    pub name: String,
}

/// The connection as admins see it (the password is never sent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OpenSlidesStatus {
    pub enabled: bool,
    pub url: String,
    /// Empty for public access.
    pub username: String,
    pub has_password: bool,
    pub meeting_id: Option<u32>,
    pub state: OpenSlidesState,
    pub error: Option<OpenSlidesError>,
    /// Meetings the account can open.
    pub meetings: Vec<OsMeetingRef>,
}
