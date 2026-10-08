// SPDX-License-Identifier: GPL-3.0-or-later
//! Speaker notes from sidecar files.
//!
//! A deck `talk.pdf` may have a `talk.notes.md` (or `talk.notes.txt`) next to it. Slides are
//! separated by lines consisting only of `---`. The first block belongs to slide 1.

use std::path::{Path, PathBuf};

/// Sidecar notes file for a deck, if one exists.
pub fn sidecar_path(deck: &Path) -> Option<PathBuf> {
    let stem = deck.file_stem()?.to_string_lossy().into_owned();
    ["md", "txt"]
        .iter()
        .map(|ext| deck.with_file_name(format!("{stem}.notes.{ext}")))
        .find(|p| p.is_file())
}

pub fn parse_sidecar_notes(text: &str) -> Vec<String> {
    let mut slides = vec![String::new()];
    for line in text.lines() {
        if line.trim_end() == "---" {
            slides.push(String::new());
        } else {
            let cur = slides.last_mut().expect("non-empty");
            if !cur.is_empty() {
                cur.push('\n');
            }
            cur.push_str(line);
        }
    }
    slides.into_iter().map(|s| s.trim().to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_separator_lines() {
        let notes = parse_sidecar_notes("Hello\nworld\n---\n\n---\nThird --- not a split\n");
        assert_eq!(notes, vec!["Hello\nworld", "", "Third --- not a split"]);
    }

    #[test]
    fn finds_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let deck = dir.path().join("talk.pdf");
        assert!(sidecar_path(&deck).is_none());
        std::fs::write(dir.path().join("talk.notes.txt"), "x").unwrap();
        assert_eq!(
            sidecar_path(&deck).unwrap(),
            dir.path().join("talk.notes.txt")
        );
    }
}
