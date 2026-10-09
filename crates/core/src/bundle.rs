// SPDX-License-Identifier: GPL-3.0-or-later
//! `.msnack` show bundles: a zip archive containing `show.json` and a `media/` directory.
//!
//! - **Embedded** bundles copy every media file into `media/`, so the show is portable.
//! - **Linked** bundles only contain `show.json` and reference media in place.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;
use zip::write::SimpleFileOptions;

use crate::model::{MediaRef, Show, SHOW_FORMAT_VERSION};
use crate::protocol::ErrorCode;

pub const SHOW_FILE_EXTENSION: &str = "msnack";
const SHOW_JSON: &str = "show.json";
const MEDIA_PREFIX: &str = "media/";

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("invalid show.json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid show bundle: {0}")]
    Invalid(String),
    #[error("media file missing: {0}")]
    MissingMedia(PathBuf),
}

impl BundleError {
    pub fn code(&self) -> ErrorCode {
        match self {
            BundleError::Io(_) | BundleError::MissingMedia(_) => ErrorCode::Io,
            _ => ErrorCode::ShowFileInvalid,
        }
    }
}

/// Writes `show` to `path` atomically. The in-memory show is not modified.
pub fn save(show: &Show, path: &Path, embed_media: bool) -> Result<(), BundleError> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = tempfile::NamedTempFile::new_in(dir)?;
    {
        let mut zip = zip::ZipWriter::new(BufWriter::new(tmp.as_file()));
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let stored =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        let mut out = show.clone();
        out.format_version = SHOW_FORMAT_VERSION;
        // source path -> bundled name
        let mut names: HashMap<PathBuf, String> = HashMap::new();
        for media in out.all_media_mut() {
            let source = show
                .resolve(media)
                .ok_or_else(|| BundleError::Invalid("unresolvable media".into()))?;
            if !embed_media {
                *media = MediaRef::Linked { path: source };
                continue;
            }
            let name = match names.get(&source) {
                Some(n) => n.clone(),
                None => {
                    let mut f = File::open(&source)
                        .map_err(|_| BundleError::MissingMedia(source.clone()))?;
                    let name = format!("{:04}-{}", names.len(), sanitize(&media.file_name()));
                    // Media is usually already compressed (PDF, JPEG, PNG).
                    zip.start_file(format!("{MEDIA_PREFIX}{name}"), stored)?;
                    io::copy(&mut f, &mut zip)?;
                    names.insert(source, name.clone());
                    name
                }
            };
            *media = MediaRef::Bundled { name };
        }
        zip.start_file(SHOW_JSON, opts)?;
        zip.write_all(&serde_json::to_vec_pretty(&out)?)?;
        zip.finish()?.flush()?;
    }
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| BundleError::Io(e.error))?;
    Ok(())
}

/// Opens a bundle, extracting embedded media into `extract_dir`.
pub fn open(path: &Path, extract_dir: &Path) -> Result<Show, BundleError> {
    let mut zip = zip::ZipArchive::new(BufReader::new(File::open(path)?))?;

    let mut show: Show = {
        let mut entry = zip
            .by_name(SHOW_JSON)
            .map_err(|_| BundleError::Invalid("show.json missing".into()))?;
        let mut buf = Vec::new();
        entry
            .by_ref()
            .take(64 * 1024 * 1024)
            .read_to_end(&mut buf)?;
        serde_json::from_slice(&buf)?
    };
    if show.format_version > SHOW_FORMAT_VERSION {
        return Err(BundleError::Invalid(format!(
            "show format {} is newer than supported {}",
            show.format_version, SHOW_FORMAT_VERSION
        )));
    }

    let mut has_media = false;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(name) = entry.name().strip_prefix(MEDIA_PREFIX).map(str::to_owned) else {
            continue;
        };
        if name.is_empty() || entry.is_dir() {
            continue;
        }
        if !is_safe_name(&name) {
            return Err(BundleError::Invalid(format!("unsafe media name {name:?}")));
        }
        if !has_media {
            std::fs::create_dir_all(extract_dir)?;
            has_media = true;
        }
        let mut out = File::create(extract_dir.join(&name))?;
        io::copy(&mut entry, &mut out)?;
    }

    for media in show.all_media() {
        if let MediaRef::Bundled { name } = media {
            if !is_safe_name(name) || !extract_dir.join(name).is_file() {
                return Err(BundleError::Invalid(format!(
                    "bundled media {name:?} missing"
                )));
            }
        }
    }
    show.media_dir = Some(extract_dir.to_owned());
    Ok(show)
}

/// A bundled media name must be a single, plain path component.
fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':', '\0'])
        && !name.starts_with('.')
}

fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_start_matches('.').to_owned();
    if s.is_empty() {
        "media".to_owned()
    } else {
        s.chars().take(120).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Cue, CueContent};

    fn fixture(dir: &Path) -> Show {
        let pdf = dir.join("deck one.pdf");
        std::fs::write(&pdf, b"%PDF-1.4 fake").unwrap();
        let img = dir.join("pic.png");
        std::fs::write(&img, b"png").unwrap();
        let mut show = Show {
            title: "Gala".into(),
            ..Show::default()
        };
        let mut cue = Cue::new(
            "Deck",
            CueContent::Pdf {
                file: MediaRef::linked(&pdf),
                page_count: 2,
                source: None,
            },
        );
        cue.slide_notes = vec!["hi".into()];
        show.cues.push(cue);
        show.cues.push(Cue::new(
            "Pic",
            CueContent::Image {
                file: MediaRef::linked(&img),
            },
        ));
        // Same file twice is stored once.
        show.cues.push(Cue::new(
            "Folder",
            CueContent::ImageFolder {
                files: vec![MediaRef::linked(&img), MediaRef::linked(&img)],
            },
        ));
        show.cues.push(Cue::new(
            "Black",
            CueContent::Blank {
                color: "#000".into(),
            },
        ));
        show
    }

    #[test]
    fn embedded_round_trip_is_portable() {
        let src = tempfile::tempdir().unwrap();
        let show = fixture(src.path());
        let file = src.path().join("show.msnack");
        save(&show, &file, true).unwrap();

        // Delete the originals: the bundle must be self-contained.
        std::fs::remove_file(src.path().join("deck one.pdf")).unwrap();
        std::fs::remove_file(src.path().join("pic.png")).unwrap();

        let extract = tempfile::tempdir().unwrap();
        let opened = open(&file, extract.path()).unwrap();
        assert_eq!(opened.title, "Gala");
        assert_eq!(opened.cues.len(), 4);
        assert_eq!(opened.cues[0].slide_notes, vec!["hi".to_owned()]);
        let pdf = opened.resolve(opened.cues[0].content.media()[0]).unwrap();
        assert_eq!(std::fs::read(pdf).unwrap(), b"%PDF-1.4 fake");
        let entries = std::fs::read_dir(extract.path()).unwrap().count();
        assert_eq!(entries, 2, "duplicate media stored once");
    }

    #[test]
    fn assets_are_bundled() {
        let src = tempfile::tempdir().unwrap();
        let mut show = fixture(src.path());
        let logo = src.path().join("logo.png");
        std::fs::write(&logo, b"logo").unwrap();
        show.assets.push(crate::model::Asset {
            id: "a1".into(),
            file: MediaRef::linked(&logo),
        });
        show.logo = Some("a1".into());
        let file = src.path().join("show.msnack");
        save(&show, &file, true).unwrap();
        std::fs::remove_file(&logo).unwrap();
        let extract = tempfile::tempdir().unwrap();
        let opened = open(&file, extract.path()).unwrap();
        let path = opened.resolve(&opened.asset("a1").unwrap().file).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"logo");
    }

    #[test]
    fn linked_bundle_references_files_in_place() {
        let src = tempfile::tempdir().unwrap();
        let show = fixture(src.path());
        let file = src.path().join("linked.msnack");
        save(&show, &file, false).unwrap();
        let extract = tempfile::tempdir().unwrap();
        let opened = open(&file, extract.path()).unwrap();
        assert_eq!(
            opened.cues[1].content.media()[0],
            &MediaRef::linked(src.path().join("pic.png"))
        );
    }

    #[test]
    fn re_saving_an_opened_bundle_works() {
        let src = tempfile::tempdir().unwrap();
        let file = src.path().join("a.msnack");
        save(&fixture(src.path()), &file, true).unwrap();
        let extract = tempfile::tempdir().unwrap();
        let opened = open(&file, extract.path()).unwrap();
        let file2 = src.path().join("b.msnack");
        save(&opened, &file2, true).unwrap();
        let extract2 = tempfile::tempdir().unwrap();
        assert_eq!(open(&file2, extract2.path()).unwrap().cues.len(), 4);
    }

    #[test]
    fn missing_media_fails_save_without_clobbering() {
        let src = tempfile::tempdir().unwrap();
        let show = fixture(src.path());
        let file = src.path().join("show.msnack");
        save(&show, &file, true).unwrap();
        let before = std::fs::read(&file).unwrap();
        std::fs::remove_file(src.path().join("pic.png")).unwrap();
        assert!(matches!(
            save(&show, &file, true),
            Err(BundleError::MissingMedia(_))
        ));
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("evil.msnack");
        {
            let mut zip = zip::ZipWriter::new(File::create(&file).unwrap());
            let o = SimpleFileOptions::default();
            zip.start_file("show.json", o).unwrap();
            zip.write_all(serde_json::to_string(&Show::default()).unwrap().as_bytes())
                .unwrap();
            zip.start_file("media/../../escape.txt", o).unwrap();
            zip.write_all(b"x").unwrap();
            zip.finish().unwrap();
        }
        let extract = dir.path().join("x");
        assert!(matches!(
            open(&file, &extract),
            Err(BundleError::Invalid(_))
        ));
        assert!(!dir.path().join("escape.txt").exists());
    }

    #[test]
    fn rejects_missing_show_json_and_future_versions() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x.msnack");
        {
            let mut zip = zip::ZipWriter::new(File::create(&file).unwrap());
            zip.start_file("other.txt", SimpleFileOptions::default())
                .unwrap();
            zip.finish().unwrap();
        }
        assert!(open(&file, dir.path()).is_err());

        let file2 = dir.path().join("y.msnack");
        {
            let mut zip = zip::ZipWriter::new(File::create(&file2).unwrap());
            let show = Show {
                format_version: 999,
                ..Show::default()
            };
            zip.start_file("show.json", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(serde_json::to_string(&show).unwrap().as_bytes())
                .unwrap();
            zip.finish().unwrap();
        }
        assert!(matches!(
            open(&file2, dir.path()),
            Err(BundleError::Invalid(_))
        ));
    }

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize("../a b?.pdf"), "_a_b_.pdf");
        assert_eq!(sanitize("..."), "media");
        assert!(is_safe_name("0001-a.pdf"));
        assert!(!is_safe_name("../a"));
        assert!(!is_safe_name(".hidden"));
    }
}
