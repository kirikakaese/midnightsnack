// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistent show model (serialized as `show.json`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::protocol::{
    CaptureInfo, CueKind, CueSummary, MediaInfo, MediaOptions, OpenSlidesCue, OpenSlidesSlide,
    OutputDef, Overlay, TextInfo, TextTheme, TimerCue, Transition, WebInfo,
};

/// Version of the `show.json` format.
pub const SHOW_FORMAT_VERSION: u32 = 1;

/// Reference to a media file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaRef {
    /// A file referenced in place on the host's file system.
    Linked { path: PathBuf },
    /// A file stored inside the show bundle's `media/` directory.
    Bundled { name: String },
}

impl MediaRef {
    pub fn linked(path: impl Into<PathBuf>) -> Self {
        MediaRef::Linked { path: path.into() }
    }

    /// Resolves the reference to a path, given the directory bundled media was extracted to.
    pub fn resolve(&self, media_dir: Option<&Path>) -> Option<PathBuf> {
        match self {
            MediaRef::Linked { path } => Some(path.clone()),
            MediaRef::Bundled { name } => media_dir.map(|d| d.join(name)),
        }
    }

    /// File name used for display and for bundling.
    pub fn file_name(&self) -> String {
        match self {
            MediaRef::Linked { path } => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "media".to_owned()),
            MediaRef::Bundled { name } => name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CueContent {
    Pdf {
        file: MediaRef,
        page_count: u32,
        /// Office document the PDF was converted from (re-converted when it changes).
        #[serde(default)]
        source: Option<MediaRef>,
    },
    Image {
        file: MediaRef,
    },
    ImageFolder {
        files: Vec<MediaRef>,
    },
    Blank {
        color: String,
    },
    Media {
        file: MediaRef,
        video: bool,
        #[serde(default)]
        options: MediaOptions,
        #[serde(default)]
        duration_ms: Option<u32>,
    },
    Text {
        source: String,
        lyrics: bool,
        slides: Vec<String>,
        #[serde(default)]
        theme: Option<TextTheme>,
    },
    Timer {
        timer: TimerCue,
    },
    Web {
        web: WebInfo,
    },
    Capture {
        capture: CaptureInfo,
    },
    /// Agenda, motion, topic, list of speakers or a followed projector of an OpenSlides
    /// meeting, drawn by the clients. `pages` is maintained by the host from the live data.
    OpenSlides {
        slide: OpenSlidesSlide,
        #[serde(default = "one_page")]
        pages: u32,
        #[serde(default)]
        theme: Option<TextTheme>,
    },
}

fn one_page() -> u32 {
    1
}

impl CueContent {
    pub fn kind(&self) -> CueKind {
        match self {
            CueContent::Pdf { .. } => CueKind::Pdf,
            CueContent::Image { .. } => CueKind::Image,
            CueContent::ImageFolder { .. } => CueKind::ImageFolder,
            CueContent::Blank { .. } => CueKind::Blank,
            CueContent::Media { video: true, .. } => CueKind::Video,
            CueContent::Media { video: false, .. } => CueKind::Audio,
            CueContent::Text { .. } => CueKind::Text,
            CueContent::Timer { .. } => CueKind::Timer,
            CueContent::Web { .. } => CueKind::Web,
            CueContent::Capture { .. } => CueKind::Capture,
            CueContent::OpenSlides { .. } => CueKind::OpenSlides,
        }
    }

    pub fn slide_count(&self) -> u32 {
        match self {
            CueContent::Pdf { page_count, .. } => *page_count,
            CueContent::Image { .. }
            | CueContent::Blank { .. }
            | CueContent::Media { .. }
            | CueContent::Timer { .. }
            | CueContent::Web { .. }
            | CueContent::Capture { .. } => 1,
            CueContent::ImageFolder { files } => files.len() as u32,
            CueContent::Text { slides, .. } => slides.len() as u32,
            CueContent::OpenSlides { pages, .. } => (*pages).max(1),
        }
    }

    /// Media files referenced by this cue (not counting theme assets).
    pub fn media(&self) -> Vec<&MediaRef> {
        match self {
            CueContent::Pdf { file, source, .. } => std::iter::once(file).chain(source).collect(),
            CueContent::Image { file } | CueContent::Media { file, .. } => vec![file],
            CueContent::ImageFolder { files } => files.iter().collect(),
            CueContent::Blank { .. }
            | CueContent::Text { .. }
            | CueContent::Timer { .. }
            | CueContent::Web { .. }
            | CueContent::Capture { .. }
            | CueContent::OpenSlides { .. } => vec![],
        }
    }

    pub fn media_mut(&mut self) -> Vec<&mut MediaRef> {
        match self {
            CueContent::Pdf { file, source, .. } => {
                std::iter::once(file).chain(source.as_mut()).collect()
            }
            CueContent::Image { file } | CueContent::Media { file, .. } => vec![file],
            CueContent::ImageFolder { files } => files.iter_mut().collect(),
            CueContent::Blank { .. }
            | CueContent::Text { .. }
            | CueContent::Timer { .. }
            | CueContent::Web { .. }
            | CueContent::Capture { .. }
            | CueContent::OpenSlides { .. } => vec![],
        }
    }

    /// The cue's own theme override, for cues that have one.
    pub fn theme_mut(&mut self) -> Option<&mut Option<TextTheme>> {
        match self {
            CueContent::Text { theme, .. } => Some(theme),
            CueContent::Timer { timer } => Some(&mut timer.theme),
            CueContent::OpenSlides { theme, .. } => Some(theme),
            _ => None,
        }
    }
}

/// Splits text into slides. Lyrics: verses separated by blank lines. Otherwise: slides
/// separated by lines containing only `---`. Empty slides are dropped.
pub fn split_text(source: &str, lyrics: bool) -> Vec<String> {
    let mut slides = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let flush = |cur: &mut Vec<&str>, slides: &mut Vec<String>| {
        let s = cur.join("\n").trim().to_owned();
        if !s.is_empty() {
            slides.push(s);
        }
        cur.clear();
    };
    for line in source.lines() {
        let boundary = if lyrics {
            line.trim().is_empty()
        } else {
            line.trim() == "---"
        };
        if boundary {
            flush(&mut cur, &mut slides);
        } else {
            cur.push(line.trim_end());
        }
    }
    flush(&mut cur, &mut slides);
    slides
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub id: String,
    pub name: String,
    pub content: CueContent,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub color: Option<String>,
    /// Speaker notes per slide.
    #[serde(default)]
    pub slide_notes: Vec<String>,
    #[serde(default)]
    pub transition: Option<Transition>,
    #[serde(default)]
    pub auto_advance_ms: Option<u32>,
    /// Output ids this cue is shown on; `None` = all program outputs.
    #[serde(default)]
    pub targets: Option<Vec<String>>,
}

impl Cue {
    pub fn new(name: impl Into<String>, content: CueContent) -> Self {
        Cue {
            id: new_id(),
            name: name.into(),
            content,
            notes: String::new(),
            color: None,
            slide_notes: Vec::new(),
            transition: None,
            auto_advance_ms: None,
            targets: None,
        }
    }

    pub fn slide_count(&self) -> u32 {
        self.content.slide_count()
    }

    pub fn summary(&self) -> CueSummary {
        CueSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.content.kind(),
            slide_count: self.slide_count(),
            color: self.color.clone(),
            notes: self.notes.clone(),
            slide_notes: self.slide_notes.clone(),
            background: match &self.content {
                CueContent::Blank { color } => Some(color.clone()),
                _ => None,
            },
            transition: self.transition,
            auto_advance_ms: self.auto_advance_ms,
            media: match &self.content {
                CueContent::Media {
                    options,
                    duration_ms,
                    ..
                } => Some(MediaInfo {
                    options: *options,
                    duration_ms: *duration_ms,
                }),
                _ => None,
            },
            text: match &self.content {
                CueContent::Text {
                    source,
                    lyrics,
                    slides,
                    theme,
                } => Some(TextInfo {
                    source: source.clone(),
                    lyrics: *lyrics,
                    slides: slides.clone(),
                    theme: theme.clone(),
                }),
                _ => None,
            },
            timer: match &self.content {
                CueContent::Timer { timer } => Some(timer.clone()),
                _ => None,
            },
            web: match &self.content {
                CueContent::Web { web } => Some(web.clone()),
                _ => None,
            },
            capture: match &self.content {
                CueContent::Capture { capture } => Some(capture.clone()),
                _ => None,
            },
            openslides: match &self.content {
                CueContent::OpenSlides { slide, theme, .. } => Some(OpenSlidesCue {
                    slide: slide.clone(),
                    theme: theme.clone(),
                }),
                _ => None,
            },
            targets: self.targets.clone(),
            converted_from: match &self.content {
                CueContent::Pdf {
                    source: Some(src), ..
                } => Some(src.file_name()),
                _ => None,
            },
        }
    }
}

/// A file used by the show outside of cue content (logo, backgrounds, logo bugs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub file: MediaRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Show {
    pub format_version: u32,
    pub id: String,
    pub title: String,
    pub cues: Vec<Cue>,
    #[serde(default)]
    pub default_transition: Transition,
    #[serde(default)]
    pub default_theme: TextTheme,
    #[serde(default)]
    pub overlays: Vec<Overlay>,
    /// Asset id of the logo screen image.
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub assets: Vec<Asset>,
    #[serde(default = "default_outputs")]
    pub outputs: Vec<OutputDef>,
    /// Directory bundled media was extracted to. Runtime only.
    #[serde(skip)]
    pub media_dir: Option<PathBuf>,
}

impl Default for Show {
    fn default() -> Self {
        Show {
            format_version: SHOW_FORMAT_VERSION,
            id: new_id(),
            title: String::new(),
            cues: Vec::new(),
            default_transition: Transition::default(),
            default_theme: TextTheme::default(),
            overlays: Vec::new(),
            logo: None,
            assets: Vec::new(),
            outputs: default_outputs(),
            media_dir: None,
        }
    }
}

fn default_outputs() -> Vec<OutputDef> {
    vec![OutputDef::main()]
}

impl Show {
    pub fn cue(&self, id: &str) -> Option<&Cue> {
        self.cues.iter().find(|c| c.id == id)
    }

    pub fn cue_index(&self, id: &str) -> Option<usize> {
        self.cues.iter().position(|c| c.id == id)
    }

    pub fn resolve(&self, media: &MediaRef) -> Option<PathBuf> {
        media.resolve(self.media_dir.as_deref())
    }

    /// The first program output; content positions of "the output" refer to it.
    pub fn main_output_id(&self) -> &str {
        use crate::protocol::OutputFeed;
        self.outputs
            .iter()
            .find(|o| o.feed == OutputFeed::Program)
            .map_or(OutputDef::MAIN_ID, |o| o.id.as_str())
    }

    pub fn asset(&self, id: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.id == id)
    }

    /// Every media reference in the show, including assets.
    pub fn all_media(&self) -> Vec<&MediaRef> {
        let mut v: Vec<&MediaRef> = self.cues.iter().flat_map(|c| c.content.media()).collect();
        v.extend(self.assets.iter().map(|a| &a.file));
        v
    }

    /// Every media reference in the show, including assets (for bundling).
    pub fn all_media_mut(&mut self) -> Vec<&mut MediaRef> {
        let mut v: Vec<&mut MediaRef> = self
            .cues
            .iter_mut()
            .flat_map(|c| c.content.media_mut())
            .collect();
        v.extend(self.assets.iter_mut().map(|a| &mut a.file));
        v
    }

    /// Asset ids still referenced by the logo, themes or overlays.
    pub fn referenced_assets(&self) -> Vec<String> {
        use crate::protocol::OverlayKind;
        let mut ids: Vec<String> = Vec::new();
        ids.extend(self.logo.clone());
        ids.extend(self.default_theme.background_image.clone());
        for c in &self.cues {
            let theme = match &c.content {
                CueContent::Text { theme, .. } => theme.as_ref(),
                CueContent::Timer { timer } => timer.theme.as_ref(),
                _ => None,
            };
            ids.extend(theme.and_then(|t| t.background_image.clone()));
        }
        for o in &self.overlays {
            if let OverlayKind::LogoBug { image: Some(id) } = &o.kind {
                ids.push(id.clone());
            }
        }
        ids
    }

    /// Drops assets nothing refers to any more.
    pub fn prune_assets(&mut self) {
        let used = self.referenced_assets();
        self.assets.retain(|a| used.contains(&a.id));
    }
}

/// Generates a new random identifier.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Validates a CSS color of the form `#rgb` or `#rrggbb`.
pub fn is_valid_color(color: &str) -> bool {
    let Some(hex) = color.strip_prefix('#') else {
        return false;
    };
    (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lyrics_on_blank_lines() {
        let text = "Verse one line one\nline two\n\n\n  \nChorus\n\nVerse two\n";
        assert_eq!(
            split_text(text, true),
            vec!["Verse one line one\nline two", "Chorus", "Verse two"]
        );
    }

    #[test]
    fn splits_announcements_on_separators() {
        let text = "Welcome\n\nto the gala\n---\nDinner at 8\n---\n---\n";
        assert_eq!(
            split_text(text, false),
            vec!["Welcome\n\nto the gala", "Dinner at 8"]
        );
        assert!(split_text("   \n", false).is_empty());
    }

    #[test]
    fn old_show_files_still_load() {
        // A phase-1 show.json without the newer fields.
        let json = r##"{"format_version":1,"id":"s","title":"t","cues":[
            {"id":"c","name":"n","content":{"type":"blank","color":"#000"}}]}"##;
        let show: Show = serde_json::from_str(json).unwrap();
        assert_eq!(show.default_transition, Transition::default());
        assert!(show.cues[0].transition.is_none());
    }
}
