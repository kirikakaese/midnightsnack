// SPDX-License-Identifier: GPL-3.0-or-later
//! Slide rendering for midnightsnack.
//!
//! - PDF pages are rendered with PDFium, which is loaded at runtime (see [`pdf::locate_pdfium`]).
//! - Images are decoded with the `image` crate (EXIF orientation applied).
//! - Results are cached on disk, keyed by file identity, page and target size, so a slide is
//!   rendered at most once per size. All rendering happens on one worker thread; on-demand
//!   requests jump ahead of prefetch requests.

mod cache;
mod images;
mod notes;
pub mod office;
pub mod pdf;
mod service;
#[doc(hidden)]
pub mod test_support;

pub use cache::RenderCache;
pub use images::{is_supported_image, list_image_folder};
pub use notes::parse_sidecar_notes;
pub use pdf::PdfInfo;
pub use service::{Priority, RenderService};

use std::path::PathBuf;

use midnightsnack_protocol::ErrorCode;

/// A single slide to render.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SlideSource {
    /// Zero-based page of a PDF.
    PdfPage {
        path: PathBuf,
        page: u32,
    },
    Image {
        path: PathBuf,
    },
}

/// Bounding box the slide is scaled to fit (aspect ratio preserved).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TargetSize {
    pub width: u32,
    pub height: u32,
}

impl TargetSize {
    /// Upper bound so a client cannot make the host render absurdly large bitmaps.
    pub const MAX: u32 = 7680;

    pub fn new(width: u32, height: u32) -> Self {
        TargetSize {
            width: width.clamp(16, Self::MAX),
            height: height.clamp(16, Self::MAX),
        }
    }

    /// Size that fits `src` into this box.
    pub fn fit(&self, src_w: f32, src_h: f32) -> (u32, u32) {
        if src_w <= 0.0 || src_h <= 0.0 {
            return (self.width, self.height);
        }
        let scale = (self.width as f32 / src_w).min(self.height as f32 / src_h);
        (
            ((src_w * scale).round() as u32).max(1),
            ((src_h * scale).round() as u32).max(1),
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("PDF engine not available: {0}")]
    PdfEngineMissing(String),
    #[error("PDF error: {0}")]
    Pdf(String),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("page {0} out of range")]
    PageOutOfRange(u32),
    #[error("render worker stopped")]
    WorkerGone,
}

impl RenderError {
    /// Stable error code for clients.
    pub fn code(&self) -> ErrorCode {
        match self {
            RenderError::PdfEngineMissing(_) => ErrorCode::PdfEngineMissing,
            RenderError::Io(_) => ErrorCode::Io,
            RenderError::PageOutOfRange(_) => ErrorCode::NotFound,
            RenderError::WorkerGone => ErrorCode::Internal,
            RenderError::Pdf(_) | RenderError::Image(_) => ErrorCode::UnsupportedFile,
        }
    }
}
