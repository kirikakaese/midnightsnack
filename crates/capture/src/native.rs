// SPDX-License-Identifier: GPL-3.0-or-later
//! macOS and Windows capture through `xcap`.

use image::RgbaImage;
use midnightsnack_protocol::{CaptureSource, CaptureTarget};

use super::{matches, CaptureError};

impl From<xcap::XCapError> for CaptureError {
    fn from(e: xcap::XCapError) -> Self {
        let msg = e.to_string();
        if msg.to_ascii_lowercase().contains("permission") {
            CaptureError::Permission
        } else {
            CaptureError::Capture(msg)
        }
    }
}

pub fn list_targets_once() -> Result<Vec<CaptureTarget>, CaptureError> {
    let mut out = Vec::new();
    for m in xcap::Monitor::all()? {
        let Ok(name) = m.name() else { continue };
        out.push(CaptureTarget {
            source: CaptureSource::Screen { name },
            width: m.width().unwrap_or(0),
            height: m.height().unwrap_or(0),
        });
    }
    if let Ok(windows) = xcap::Window::all() {
        for w in windows {
            if w.is_minimized().unwrap_or(false) {
                continue;
            }
            let title = w.title().unwrap_or_default();
            let app = w.app_name().unwrap_or_default();
            if title.trim().is_empty() || app.to_ascii_lowercase().contains("midnightsnack") {
                continue;
            }
            out.push(CaptureTarget {
                source: CaptureSource::Window { app, title },
                width: w.width().unwrap_or(0),
                height: w.height().unwrap_or(0),
            });
        }
    }
    Ok(out)
}

pub enum Target {
    Monitor(xcap::Monitor),
    Window(xcap::Window),
}

impl Target {
    pub fn capture(&mut self) -> Result<RgbaImage, CaptureError> {
        Ok(match self {
            Target::Monitor(m) => m.capture_image()?,
            Target::Window(w) => {
                if w.is_minimized().unwrap_or(false) {
                    return Err(CaptureError::Capture("window minimized".into()));
                }
                w.capture_image()?
            }
        })
    }
}

pub fn resolve(source: &CaptureSource) -> Option<Target> {
    match source {
        CaptureSource::Screen { name } => xcap::Monitor::all()
            .ok()?
            .into_iter()
            .find(|m| m.name().is_ok_and(|n| n == *name))
            .map(Target::Monitor),
        CaptureSource::Window { app, title } => {
            let windows = xcap::Window::all().ok()?;
            let candidates = windows.into_iter().filter(|w| {
                !w.is_minimized().unwrap_or(false)
                    && matches(&w.app_name().unwrap_or_default(), app)
                    && matches(&w.title().unwrap_or_default(), title)
            });
            // Prefer an exact title match, then the front-most window.
            let mut best: Vec<xcap::Window> = candidates.collect();
            best.sort_by_key(|w| {
                let exact = w
                    .title()
                    .is_ok_and(|t| t.eq_ignore_ascii_case(title.trim()));
                (!exact, -w.z().unwrap_or(0))
            });
            best.into_iter().next().map(Target::Window)
        }
    }
}
