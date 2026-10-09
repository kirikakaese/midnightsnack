// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Identifier of a cue (UUID string).
pub type CueId = String;

/// Permission level of a connected device. Ordered from least to most privileged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Role {
    StageViewer,
    Presenter,
    Operator,
    Admin,
}

/// A slide inside a cue. `slide` is zero-based.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Position {
    pub cue_id: CueId,
    pub slide: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CueKind {
    Pdf,
    Image,
    ImageFolder,
    Blank,
    Video,
    Audio,
    Text,
    Timer,
    Web,
    Capture,
}

/// What an output window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OutputFeed {
    /// Program content (cues targeted at this output).
    #[default]
    Program,
    /// The stage display (current/next, notes, timers, messages).
    Stage,
}

/// How content that does not match the output's aspect ratio is scaled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OutputScaling {
    /// Letterbox: the whole slide is visible.
    #[default]
    Fit,
    /// Crop: the screen is filled.
    Fill,
    /// Distort to fill.
    Stretch,
}

/// An output of the show (a projector, a second room, a confidence monitor). Which physical
/// display it uses is a host setting, not part of the show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OutputDef {
    pub id: String,
    pub name: String,
    pub feed: OutputFeed,
    /// Show overlays on this output.
    pub overlays: bool,
    pub scaling: OutputScaling,
    /// Safe margin in percent of each side (0–20).
    pub margin: u32,
}

impl OutputDef {
    pub const MAIN_ID: &'static str = "main";

    pub fn main() -> Self {
        OutputDef {
            id: Self::MAIN_ID.into(),
            name: "Main".into(),
            feed: OutputFeed::Program,
            overlays: true,
            scaling: OutputScaling::Fit,
            margin: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TestPattern {
    /// Grid with circles and the output resolution, for focus and geometry.
    Grid,
    /// Color bars, for color and brightness.
    Bars,
}

/// A web page shown in an isolated webview.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebInfo {
    pub url: String,
    /// Zoom in percent (25–400).
    pub zoom: u32,
    /// Refuse navigation to other sites than the cue's URL origin.
    pub block_navigation: bool,
    /// Forward next/prev to the page as arrow keys (reveal.js and similar).
    pub forward_keys: bool,
    /// Keep cookies and logins between launches (e.g. OpenSlides).
    pub persist_session: bool,
    /// Created as an OpenSlides projector cue.
    pub openslides: bool,
}

/// What a capture cue captures.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum CaptureSource {
    /// A whole display, by name.
    Screen { name: String },
    /// A window, by application and title (matched case-insensitively).
    Window { app: String, title: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CaptureInfo {
    pub source: CaptureSource,
    /// Frames per second (1–60).
    pub fps: u32,
}

/// A screen or window the host can capture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CaptureTarget {
    pub source: CaptureSource,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TransitionKind {
    #[default]
    Cut,
    Fade,
}

/// How a cue's slides appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Transition {
    pub kind: TransitionKind,
    pub duration_ms: u32,
}

impl Transition {
    pub const MAX_DURATION_MS: u32 = 10_000;
}

/// Playback options of a video or audio cue.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MediaOptions {
    #[serde(rename = "loop")]
    pub loop_playback: bool,
    /// Trim: playback starts here.
    pub start_ms: u32,
    /// Trim: playback ends here (`None` = end of file).
    pub end_ms: Option<u32>,
    /// 0.0 – 1.0
    pub volume: f32,
    /// Go to the next cue when playback ends (ignored when looping).
    pub auto_advance: bool,
}

impl Default for MediaOptions {
    fn default() -> Self {
        MediaOptions {
            loop_playback: false,
            start_ms: 0,
            end_ms: None,
            volume: 1.0,
            auto_advance: false,
        }
    }
}

/// Media facts and options sent with media cues.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MediaInfo {
    pub options: MediaOptions,
    /// Reported by the output once the file's metadata is loaded.
    pub duration_ms: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// Look of text and timer slides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TextTheme {
    pub font_family: String,
    /// Font size in percent of the output height; `None` fits the text to the screen.
    pub font_size: Option<u32>,
    pub color: String,
    pub background: String,
    /// Asset id of a background image.
    pub background_image: Option<String>,
    pub align: TextAlign,
}

impl Default for TextTheme {
    fn default() -> Self {
        TextTheme {
            font_family: "Inter, system-ui, sans-serif".into(),
            font_size: None,
            color: "#ffffff".into(),
            background: "#000000".into(),
            background_image: None,
            align: TextAlign::Center,
        }
    }
}

/// Text content of a text cue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TextInfo {
    /// The text as entered.
    pub source: String,
    /// Lyrics mode: verses are separated by blank lines (otherwise by `---` lines).
    pub lyrics: bool,
    pub slides: Vec<String>,
    pub theme: Option<TextTheme>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[ts(export)]
pub enum TimerMode {
    /// Counts down from `duration_ms` once the cue goes live.
    Countdown { duration_ms: u32 },
    /// Counts down to a local wall-clock time (`HH:MM`).
    CountdownTo { time: String },
    /// Counts up from when the cue goes live.
    CountUp,
    /// Shows the time of day.
    Clock,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TimerCue {
    pub mode: TimerMode,
    pub label: String,
    /// Color once a countdown has passed zero.
    pub overtime_color: String,
    pub theme: Option<TextTheme>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OverlayPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    #[default]
    BottomCenter,
    BottomRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum OverlayKind {
    LowerThird {
        title: String,
        subtitle: String,
    },
    /// Corner image; `image` is an asset id.
    LogoBug {
        image: Option<String>,
    },
    Clock {
        seconds: bool,
    },
    /// Scrolling text; `speed` in percent of the screen width per second.
    Ticker {
        text: String,
        speed: u32,
    },
    /// Shows the global countdown.
    Countdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Overlay {
    pub id: String,
    pub name: String,
    pub kind: OverlayKind,
    pub position: OverlayPosition,
    pub color: String,
    pub background: String,
    /// Size in percent of the default.
    pub scale: u32,
}

/// What clients need to know about a cue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CueSummary {
    pub id: CueId,
    pub name: String,
    pub kind: CueKind,
    pub slide_count: u32,
    /// CSS color of the cue's color tag, e.g. `#ef4444`.
    pub color: Option<String>,
    /// Cue-level notes.
    pub notes: String,
    /// Speaker notes per slide (may be shorter than `slide_count`).
    pub slide_notes: Vec<String>,
    /// Background color for blank cues.
    pub background: Option<String>,
    /// Per-cue transition; `None` uses the show default.
    pub transition: Option<Transition>,
    /// Advance to the next slide after this delay.
    pub auto_advance_ms: Option<u32>,
    pub media: Option<MediaInfo>,
    pub text: Option<TextInfo>,
    pub timer: Option<TimerCue>,
    pub web: Option<WebInfo>,
    pub capture: Option<CaptureInfo>,
    /// Output ids this cue is shown on; `None` = all program outputs.
    pub targets: Option<Vec<String>>,
    /// File name of the office document a PDF cue was converted from.
    pub converted_from: Option<String>,
}

/// The show structure. Sent whenever the cue list changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ShowSnapshot {
    pub id: String,
    pub title: String,
    pub cues: Vec<CueSummary>,
    /// Path of the show file on the host, if saved. Only sent to admins.
    pub path: Option<String>,
    /// Unsaved changes exist.
    pub dirty: bool,
    pub revision: u64,
    pub default_transition: Transition,
    pub default_theme: TextTheme,
    pub overlays: Vec<Overlay>,
    /// Asset id of the logo screen image (`None` = built-in logo).
    pub logo: Option<String>,
    pub outputs: Vec<OutputDef>,
}

/// Master states, applied on top of the program content.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Masters {
    pub blackout: bool,
    pub freeze: bool,
    pub logo: bool,
}

/// A stopwatch. Elapsed time is `accumulated_ms + (now - running_since_ms)` while running.
/// Timestamps are Unix epoch milliseconds of the host clock; `LiveState::host_time_ms` lets
/// clients compensate for clock offset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Stopwatch {
    #[ts(type = "number")]
    pub accumulated_ms: i64,
    #[ts(type = "number | null")]
    pub running_since_ms: Option<i64>,
}

impl Stopwatch {
    pub fn elapsed_ms(&self, now_ms: i64) -> i64 {
        self.accumulated_ms + self.running_since_ms.map_or(0, |s| (now_ms - s).max(0))
    }
    pub fn is_running(&self) -> bool {
        self.running_since_ms.is_some()
    }
}

/// Playback of the media cue that is on the output. Every view derives the current position
/// from this timeline, so the output, the operator and phones agree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MediaPlayback {
    pub cue_id: CueId,
    /// Position in the file (ms) as a stopwatch: running while playing.
    pub position: Stopwatch,
    /// Playback reached the end and stopped.
    pub ended: bool,
}

/// The global countdown shown by countdown overlays, stage displays and remotes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Countdown {
    pub duration_ms: u32,
    pub label: String,
    pub elapsed: Stopwatch,
}

impl Default for Countdown {
    fn default() -> Self {
        Countdown {
            duration_ms: 5 * 60 * 1000,
            label: String::new(),
            elapsed: Stopwatch::default(),
        }
    }
}

/// What one output currently shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OutputLive {
    pub output_id: String,
    pub position: Option<Position>,
}

/// Live show state. Sent on every change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LiveState {
    /// Where the operator is.
    pub program: Option<Position>,
    /// What the main output shows (differs from `program` while frozen or when the program cue
    /// is not targeted at it).
    pub output: Option<Position>,
    /// What `next` would go to.
    pub next: Option<Position>,
    /// What `prev` would go to.
    pub prev: Option<Position>,
    pub masters: Masters,
    /// Show running time.
    pub show_timer: Stopwatch,
    /// Time on the current slide.
    pub slide_timer: Stopwatch,
    pub media: Option<MediaPlayback>,
    pub countdown: Countdown,
    /// Ids of overlays currently shown.
    pub overlays_visible: Vec<String>,
    /// Message from the operator to stage displays.
    pub stage_message: Option<String>,
    /// Per program output: what it shows (outputs keep their last targeted cue).
    pub outputs: Vec<OutputLive>,
    pub test_pattern: Option<TestPattern>,
    /// Capture cues whose source is currently unavailable.
    pub capture_lost: Vec<CueId>,
    /// When the program will advance automatically.
    #[ts(type = "number | null")]
    pub auto_advance_at_ms: Option<i64>,
    /// Host clock at the time this state was sent.
    #[ts(type = "number")]
    pub host_time_ms: i64,
    pub revision: u64,
}
