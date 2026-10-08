// SPDX-License-Identifier: GPL-3.0-or-later
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

use crate::{SlideSource, TargetSize};

/// On-disk cache of rendered slides.
#[derive(Debug, Clone)]
pub struct RenderCache {
    root: PathBuf,
}

impl RenderCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        RenderCache { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Cache path for a slide at a size. The key includes the file's size and modification
    /// time, so editing a deck invalidates its cached slides.
    pub fn path_for(&self, source: &SlideSource, size: TargetSize) -> io::Result<PathBuf> {
        let (file, page, ext) = match source {
            SlideSource::PdfPage { path, page } => (path, *page, "png"),
            SlideSource::Image { path } => (path, 0, "jpg"),
        };
        let meta = std::fs::metadata(file)?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        let mut h = Sha256::new();
        h.update(file.to_string_lossy().as_bytes());
        h.update(meta.len().to_le_bytes());
        h.update(mtime.to_le_bytes());
        let digest = h.finalize();
        let key: String = digest[..12].iter().map(|b| format!("{b:02x}")).collect();
        Ok(self
            .root
            .join(key)
            .join(format!("{page}-{}x{}.{ext}", size.width, size.height)))
    }

    /// Removes everything in the cache.
    pub fn clear(&self) -> io::Result<()> {
        match std::fs::remove_dir_all(&self.root) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_changes_when_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.pdf");
        std::fs::write(&f, b"one").unwrap();
        let cache = RenderCache::new(dir.path().join("cache"));
        let src = SlideSource::PdfPage {
            path: f.clone(),
            page: 2,
        };
        let size = TargetSize::new(1920, 1080);
        let p1 = cache.path_for(&src, size).unwrap();
        assert!(p1.ends_with("2-1920x1080.png"));
        std::fs::write(&f, b"three").unwrap();
        assert_ne!(cache.path_for(&src, size).unwrap(), p1);
        assert_ne!(cache.path_for(&src, TargetSize::new(480, 270)).unwrap(), p1);
    }
}
