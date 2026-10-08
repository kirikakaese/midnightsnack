// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistent show model (serialized as `show.json`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::protocol::{CueKind, CueSummary};

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
    Pdf { file: MediaRef, page_count: u32 },
    Image { file: MediaRef },
    ImageFolder { files: Vec<MediaRef> },
    Blank { color: String },
}

impl CueContent {
    pub fn kind(&self) -> CueKind {
        match self {
            CueContent::Pdf { .. } => CueKind::Pdf,
            CueContent::Image { .. } => CueKind::Image,
            CueContent::ImageFolder { .. } => CueKind::ImageFolder,
            CueContent::Blank { .. } => CueKind::Blank,
        }
    }

    pub fn slide_count(&self) -> u32 {
        match self {
            CueContent::Pdf { page_count, .. } => *page_count,
            CueContent::Image { .. } | CueContent::Blank { .. } => 1,
            CueContent::ImageFolder { files } => files.len() as u32,
        }
    }

    /// All media referenced by this cue.
    pub fn media(&self) -> Vec<&MediaRef> {
        match self {
            CueContent::Pdf { file, .. } | CueContent::Image { file } => vec![file],
            CueContent::ImageFolder { files } => files.iter().collect(),
            CueContent::Blank { .. } => vec![],
        }
    }

    pub fn media_mut(&mut self) -> Vec<&mut MediaRef> {
        match self {
            CueContent::Pdf { file, .. } | CueContent::Image { file } => vec![file],
            CueContent::ImageFolder { files } => files.iter_mut().collect(),
            CueContent::Blank { .. } => vec![],
        }
    }
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Show {
    pub format_version: u32,
    pub id: String,
    pub title: String,
    pub cues: Vec<Cue>,
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
            media_dir: None,
        }
    }
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
