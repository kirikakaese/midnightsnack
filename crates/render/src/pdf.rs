// SPDX-License-Identifier: GPL-3.0-or-later
//! PDF support through PDFium, loaded at runtime from a bundled or system library.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use image::DynamicImage;
use pdfium_render::prelude::*;

use crate::notes::{parse_sidecar_notes, sidecar_path};
use crate::{RenderError, TargetSize};

/// Environment variable pointing at a PDFium library file or the directory containing it.
pub const PDFIUM_ENV: &str = "MIDNIGHTSNACK_PDFIUM";

static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();

/// Candidate library paths, in priority order: `$MIDNIGHTSNACK_PDFIUM`, the given directories
/// (e.g. the app's resource directory), and the executable's directory. The system library
/// search path is tried last.
pub fn locate_pdfium(extra_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let name = Pdfium::pdfium_platform_library_name();
    let mut out = Vec::new();
    if let Some(env) = std::env::var_os(PDFIUM_ENV) {
        let p = PathBuf::from(env);
        out.push(if p.is_dir() { p.join(&name) } else { p });
    }
    for d in extra_dirs {
        out.push(d.join(&name));
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_owned))
    {
        out.push(exe_dir.join(&name));
    }
    out
}

/// Loads PDFium once per process. Later calls return the first result.
pub fn init_pdfium(extra_dirs: &[PathBuf]) -> Result<&'static Pdfium, RenderError> {
    let result = PDFIUM.get_or_init(|| {
        let mut errors = Vec::new();
        for candidate in locate_pdfium(extra_dirs) {
            if !candidate.is_file() {
                continue;
            }
            match Pdfium::bind_to_library(&candidate) {
                Ok(b) => {
                    tracing::info!(path = %candidate.display(), "loaded PDFium");
                    return Ok(Pdfium::new(b));
                }
                Err(e) => errors.push(format!("{}: {e}", candidate.display())),
            }
        }
        match Pdfium::bind_to_system_library() {
            Ok(b) => {
                tracing::info!("loaded system PDFium");
                Ok(Pdfium::new(b))
            }
            Err(e) => {
                errors.push(format!("system: {e}"));
                tracing::warn!(?errors, "PDFium not available");
                Err(errors.join("; "))
            }
        }
    });
    result
        .as_ref()
        .map_err(|e| RenderError::PdfEngineMissing(e.clone()))
}

/// Whether PDFium has been loaded successfully.
pub fn pdfium_available() -> bool {
    PDFIUM.get().is_some_and(|r| r.is_ok())
}

fn pdf_err(e: PdfiumError) -> RenderError {
    RenderError::Pdf(e.to_string())
}

/// Facts about a PDF needed to create a cue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfInfo {
    pub page_count: u32,
    /// Speaker notes per page: from a sidecar file if present, else from text annotations.
    pub notes: Vec<String>,
}

pub(crate) fn inspect(pdfium: &Pdfium, path: &Path) -> Result<PdfInfo, RenderError> {
    let doc = pdfium.load_pdf_from_file(path, None).map_err(pdf_err)?;
    let pages = doc.pages();
    let page_count = pages.len() as u32;

    let notes = if let Some(sidecar) = sidecar_path(path) {
        parse_sidecar_notes(&std::fs::read_to_string(sidecar)?)
    } else {
        let mut notes = Vec::with_capacity(page_count as usize);
        for page in pages.iter() {
            let text: Vec<String> = page
                .annotations()
                .iter()
                .filter(|a| a.annotation_type() == PdfPageAnnotationType::Text)
                .filter_map(|a| a.contents())
                .map(|c| c.trim().to_owned())
                .filter(|c| !c.is_empty())
                .collect();
            notes.push(text.join("\n\n"));
        }
        while notes.last().is_some_and(String::is_empty) {
            notes.pop();
        }
        notes
    };
    Ok(PdfInfo { page_count, notes })
}

pub(crate) fn render_page(
    pdfium: &Pdfium,
    path: &Path,
    page: u32,
    size: TargetSize,
) -> Result<DynamicImage, RenderError> {
    let doc = pdfium.load_pdf_from_file(path, None).map_err(pdf_err)?;
    let pages = doc.pages();
    if page >= pages.len() as u32 {
        return Err(RenderError::PageOutOfRange(page));
    }
    let p = pages.get(page as _).map_err(pdf_err)?;
    let (w, h) = size.fit(p.width().value, p.height().value);
    let config = PdfRenderConfig::new()
        .set_target_size(w as Pixels, h as Pixels)
        .render_form_data(true)
        .render_annotations(false)
        .set_clear_color(PdfColor::WHITE);
    let bitmap = p.render_with_config(&config).map_err(pdf_err)?;
    bitmap.as_image().map_err(pdf_err)
}
