// SPDX-License-Identifier: GPL-3.0-or-later
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageDecoder, ImageReader};

use crate::{RenderError, TargetSize};

const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Image files directly inside `dir`, in natural order (`2.png` before `10.png`).
/// Hidden files are skipped.
pub fn list_image_folder(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_supported_image(p))
        .filter(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect();
    files.sort_by(|a, b| {
        natural_cmp(
            &a.file_name().unwrap().to_string_lossy(),
            &b.file_name().unwrap().to_string_lossy(),
        )
    });
    Ok(files)
}

pub(crate) fn render_image(path: &Path, size: TargetSize) -> Result<DynamicImage, RenderError> {
    let mut decoder = ImageReader::open(path)?
        .with_guessed_format()?
        .into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    let (w, h) = size.fit(img.width() as f32, img.height() as f32);
    if (w, h) == (img.width(), img.height()) {
        return Ok(img);
    }
    Ok(img.resize_exact(w, h, image::imageops::FilterType::CatmullRom))
}

fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na: String = std::iter::from_fn(|| a.next_if(char::is_ascii_digit)).collect();
                let nb: String = std::iter::from_fn(|| b.next_if(char::is_ascii_digit)).collect();
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                a.next();
                b.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = vec!["10.png", "2.png", "Slide 1.png", "1.png", "a.png", "B.png"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            v,
            vec!["1.png", "2.png", "10.png", "a.png", "B.png", "Slide 1.png"]
        );
    }

    #[test]
    fn lists_and_renders_images() {
        let dir = tempfile::tempdir().unwrap();
        for (name, w, h) in [("10.png", 400, 200), ("2.png", 100, 100)] {
            image::RgbImage::from_pixel(w, h, image::Rgb([255, 0, 0]))
                .save(dir.path().join(name))
                .unwrap();
        }
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        std::fs::write(dir.path().join(".hidden.png"), "x").unwrap();
        let files = list_image_folder(dir.path()).unwrap();
        let names: Vec<_> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_owned())
            .collect();
        assert_eq!(names, vec!["2.png", "10.png"]);

        let img = render_image(&files[1], TargetSize::new(1920, 1080)).unwrap();
        assert_eq!((img.width(), img.height()), (1920, 960));
    }
}
