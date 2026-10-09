// SPDX-License-Identifier: GPL-3.0-or-later
//! Screen and window capture.
//!
//! A [`CaptureHub`] runs one worker thread per captured source, started by the first viewer and
//! stopped shortly after the last one leaves. Frames are JPEG-encoded once and shared with every
//! viewer through a watch channel (only the newest frame matters for live video). If the source
//! disappears (window closed, display unplugged) the worker keeps the last frame, reports the
//! source as lost and keeps looking for it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use image::RgbaImage;
use midnightsnack_protocol::{CaptureSource, CaptureTarget};
use tokio::sync::watch;

/// Frames larger than this (in either dimension) are scaled down to keep latency low.
const MAX_WIDTH: u32 = 1920;
const MAX_HEIGHT: u32 = 1200;
const JPEG_QUALITY: u8 = 80;
/// How long a worker keeps running without viewers.
const IDLE_TIMEOUT: Duration = Duration::from_secs(3);
/// How often a lost source is looked for again.
const RETRY: Duration = Duration::from_millis(500);

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("capture failed: {0}")]
    Capture(String),
    #[error("screen recording permission missing")]
    Permission,
}

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

/// One encoded frame.
#[derive(Debug)]
pub struct Frame {
    pub jpeg: Bytes,
    pub width: u32,
    pub height: u32,
    pub seq: u64,
}

pub type FrameReceiver = watch::Receiver<Option<Arc<Frame>>>;

/// Screens and (visible, titled) windows that can be captured.
pub fn list_targets() -> Result<Vec<CaptureTarget>, CaptureError> {
    // Listing opens a fresh display connection, which can fail transiently; retry briefly.
    let mut attempt = 0;
    loop {
        match list_targets_once() {
            Ok(t) => return Ok(t),
            Err(CaptureError::Permission) => return Err(CaptureError::Permission),
            Err(_) if attempt < 3 => {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e),
        }
    }
}

fn list_targets_once() -> Result<Vec<CaptureTarget>, CaptureError> {
    let mut out = Vec::new();
    for m in xcap::Monitor::all()? {
        let Ok(name) = m.name() else { continue };
        out.push(CaptureTarget {
            source: CaptureSource::Screen { name },
            width: m.width().unwrap_or(0),
            height: m.height().unwrap_or(0),
        });
    }
    // Window listing is not available everywhere (e.g. Wayland); screens still work.
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

enum Target {
    Monitor(xcap::Monitor),
    Window(xcap::Window),
    /// Linux/X11: one persistent connection instead of one per frame.
    #[cfg(target_os = "linux")]
    X11(Box<x11::Grabber>),
}

impl Target {
    fn capture(&mut self) -> Result<RgbaImage, CaptureError> {
        Ok(match self {
            Target::Monitor(m) => m.capture_image()?,
            Target::Window(w) => {
                if w.is_minimized().unwrap_or(false) {
                    return Err(CaptureError::Capture("window minimized".into()));
                }
                w.capture_image()?
            }
            #[cfg(target_os = "linux")]
            Target::X11(g) => g.capture()?,
        })
    }
}

fn matches(haystack: &str, needle: &str) -> bool {
    needle.trim().is_empty()
        || haystack
            .to_lowercase()
            .contains(&needle.trim().to_lowercase())
}

fn resolve(source: &CaptureSource) -> Option<Target> {
    let target = resolve_xcap(source)?;
    #[cfg(target_os = "linux")]
    if let Some(g) = x11::Grabber::for_target(&target) {
        return Some(Target::X11(Box::new(g)));
    }
    Some(target)
}

fn resolve_xcap(source: &CaptureSource) -> Option<Target> {
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

/// JPEG-encodes a frame, scaling it down if it is very large.
pub fn encode(img: &RgbaImage) -> Result<(Vec<u8>, u32, u32), CaptureError> {
    let scaled;
    let img = if img.width() > MAX_WIDTH || img.height() > MAX_HEIGHT {
        let scale =
            (MAX_WIDTH as f32 / img.width() as f32).min(MAX_HEIGHT as f32 / img.height() as f32);
        let (w, h) = (
            ((img.width() as f32) * scale) as u32,
            ((img.height() as f32) * scale) as u32,
        );
        scaled = image::imageops::thumbnail(img, w.max(1), h.max(1));
        &scaled
    } else {
        img
    };
    let (w, h) = (img.width(), img.height());
    let mut out = Vec::with_capacity((w * h / 4) as usize);
    let encoder = jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY);
    encoder
        .encode(
            img.as_raw(),
            w as u16,
            h as u16,
            jpeg_encoder::ColorType::Rgba,
        )
        .map_err(|e| CaptureError::Capture(e.to_string()))?;
    Ok((out, w, h))
}

struct Session {
    tx: watch::Sender<Option<Arc<Frame>>>,
    lost: Arc<AtomicBool>,
    fps: Arc<std::sync::atomic::AtomicU32>,
}

/// Shared capture workers, one per source.
#[derive(Default, Clone)]
pub struct CaptureHub {
    sessions: Arc<Mutex<HashMap<CaptureSource, Session>>>,
}

impl CaptureHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribes to a source, starting its worker if needed. Higher `fps` requests raise the
    /// worker's frame rate; the stream is never slower than what any viewer asked for.
    pub fn subscribe(&self, source: &CaptureSource, fps: u32) -> FrameReceiver {
        let fps = fps.clamp(1, 60);
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = sessions.get(source) {
            s.fps.fetch_max(fps, Ordering::Relaxed);
            return s.tx.subscribe();
        }
        let (tx, rx) = watch::channel(None);
        let lost = Arc::new(AtomicBool::new(false));
        let fps = Arc::new(std::sync::atomic::AtomicU32::new(fps));
        sessions.insert(
            source.clone(),
            Session {
                tx: tx.clone(),
                lost: lost.clone(),
                fps: fps.clone(),
            },
        );
        let hub = self.clone();
        let source = source.clone();
        std::thread::Builder::new()
            .name("midnightsnack-capture".into())
            .spawn(move || hub.run(source, tx, lost, fps))
            .expect("spawn capture thread");
        rx
    }

    /// Sources that are being captured but currently unavailable.
    pub fn lost_sources(&self) -> Vec<CaptureSource> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions
            .iter()
            .filter(|(_, s)| s.lost.load(Ordering::Relaxed))
            .map(|(src, _)| src.clone())
            .collect()
    }

    fn run(
        &self,
        source: CaptureSource,
        tx: watch::Sender<Option<Arc<Frame>>>,
        lost: Arc<AtomicBool>,
        fps: Arc<std::sync::atomic::AtomicU32>,
    ) {
        let mut target = resolve(&source);
        let mut seq = 0u64;
        let mut last_viewer = Instant::now();
        let mut last_resolve = Instant::now();
        tracing::info!(?source, "capture started");
        loop {
            let started = Instant::now();
            if tx.receiver_count() > 0 {
                last_viewer = started;
            } else if started.duration_since(last_viewer) > IDLE_TIMEOUT {
                let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
                // Someone may have subscribed while we were checking.
                if tx.receiver_count() == 0 {
                    sessions.remove(&source);
                    tracing::info!(?source, "capture stopped (no viewers)");
                    return;
                }
            }

            let frame = target.as_mut().map(Target::capture);
            match frame {
                Some(Ok(img)) => match encode(&img) {
                    Ok((jpeg, width, height)) => {
                        seq += 1;
                        lost.store(false, Ordering::Relaxed);
                        tx.send_replace(Some(Arc::new(Frame {
                            jpeg: jpeg.into(),
                            width,
                            height,
                            seq,
                        })));
                    }
                    Err(e) => tracing::warn!(error = %e, "capture encode failed"),
                },
                other => {
                    if !lost.swap(true, Ordering::Relaxed) {
                        let reason = match other {
                            Some(Err(e)) => e.to_string(),
                            _ => "source not found".into(),
                        };
                        tracing::warn!(?source, %reason, "capture source lost");
                    }
                    if last_resolve.elapsed() >= RETRY {
                        last_resolve = Instant::now();
                        target = resolve(&source);
                    }
                }
            }

            let period = Duration::from_secs_f64(1.0 / fps.load(Ordering::Relaxed).max(1) as f64);
            let wait = if lost.load(Ordering::Relaxed) {
                RETRY
            } else {
                period
            };
            if let Some(rest) = wait.checked_sub(started.elapsed()) {
                std::thread::sleep(rest);
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod x11 {
    //! Frame grabbing over a persistent X11 connection (not used under Wayland).
    use image::RgbaImage;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, ImageOrder};
    use x11rb::rust_connection::RustConnection;

    use super::{CaptureError, Target};

    pub struct Grabber {
        conn: RustConnection,
        drawable: u32,
        /// Region for screens; `None` = the whole drawable (windows).
        region: Option<(i16, i16, u16, u16)>,
    }

    fn is_x11_session() -> bool {
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        std::env::var_os("DISPLAY").is_some()
            && (session == "x11" || std::env::var_os("WAYLAND_DISPLAY").is_none())
    }

    impl Grabber {
        pub fn for_target(target: &Target) -> Option<Self> {
            if !is_x11_session() {
                return None;
            }
            let (conn, screen) = x11rb::connect(None).ok()?;
            let root = conn.setup().roots.get(screen)?.root;
            let (drawable, region) = match target {
                Target::Monitor(m) => (
                    root,
                    Some((
                        m.x().ok()? as i16,
                        m.y().ok()? as i16,
                        m.width().ok()? as u16,
                        m.height().ok()? as u16,
                    )),
                ),
                Target::Window(w) => (w.id().ok()?, None),
                Target::X11(_) => return None,
            };
            Some(Grabber {
                conn,
                drawable,
                region,
            })
        }

        pub fn capture(&mut self) -> Result<RgbaImage, CaptureError> {
            let err = |e: &dyn std::fmt::Display| CaptureError::Capture(e.to_string());
            let (x, y, w, h) = match self.region {
                Some(r) => r,
                None => {
                    let g = self
                        .conn
                        .get_geometry(self.drawable)
                        .map_err(|e| err(&e))?
                        .reply()
                        .map_err(|e| err(&e))?;
                    (0, 0, g.width, g.height)
                }
            };
            let reply = self
                .conn
                .get_image(ImageFormat::Z_PIXMAP, self.drawable, x, y, w, h, !0)
                .map_err(|e| err(&e))?
                .reply()
                .map_err(|e| err(&e))?;
            let setup = self.conn.setup();
            let bpp = setup
                .pixmap_formats
                .iter()
                .find(|f| f.depth == reply.depth)
                .map(|f| f.bits_per_pixel)
                .unwrap_or(0);
            if bpp != 32 || setup.image_byte_order != ImageOrder::LSB_FIRST {
                return Err(CaptureError::Capture(format!(
                    "unsupported pixel format ({bpp} bpp)"
                )));
            }
            // BGRX -> RGBA
            let mut rgba = reply.data;
            for px in rgba.chunks_exact_mut(4) {
                px.swap(0, 2);
                px[3] = 255;
            }
            RgbaImage::from_raw(w as u32, h as u32, rgba)
                .ok_or_else(|| CaptureError::Capture("short image".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_downscales() {
        let img = RgbaImage::from_pixel(3840, 2160, image::Rgba([10, 200, 30, 255]));
        let (jpeg, w, h) = encode(&img).unwrap();
        assert_eq!((w, h), (1920, 1080));
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8], "JPEG magic");
        let small = RgbaImage::new(640, 360);
        assert_eq!(encode(&small).unwrap().1, 640);
    }

    #[test]
    fn matching_is_case_insensitive_and_optional() {
        assert!(matches("Mozilla Firefox", "firefox"));
        assert!(matches("anything", " "));
        assert!(!matches("Chromium", "firefox"));
    }

    /// Needs a desktop session (run with `--ignored`).
    #[test]
    #[ignore]
    fn captures_the_first_screen() {
        let targets = list_targets().unwrap();
        let screen = targets
            .iter()
            .find(|t| matches!(t.source, CaptureSource::Screen { .. }))
            .expect("a screen");
        let hub = CaptureHub::new();
        let mut rx = hub.subscribe(&screen.source, 30);
        let started = Instant::now();
        let mut frames = 0;
        while started.elapsed() < Duration::from_secs(2) {
            if rx.has_changed().unwrap_or(false) {
                rx.borrow_and_update();
                frames += 1;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let frame = rx.borrow().clone().expect("a frame");
        eprintln!(
            "{frames} frames in 2 s, {}x{}, {} KiB",
            frame.width,
            frame.height,
            frame.jpeg.len() / 1024
        );
        assert!(frames >= 10);
        let started = Instant::now();
        for _ in 0..10 {
            let img = image::RgbaImage::new(frame.width, frame.height);
            encode(&img).unwrap();
        }
        eprintln!(
            "encode: {:.1} ms per frame",
            started.elapsed().as_secs_f64() * 100.0
        );
    }

    #[test]
    fn missing_sources_report_lost_and_stop_without_viewers() {
        let hub = CaptureHub::new();
        let source = CaptureSource::Window {
            app: "no-such-app-xyz".into(),
            title: "nope".into(),
        };
        let rx = hub.subscribe(&source, 5);
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(hub.lost_sources(), vec![source.clone()]);
        assert!(rx.borrow().is_none());
        drop(rx);
        std::thread::sleep(IDLE_TIMEOUT + Duration::from_millis(1200));
        assert!(
            hub.lost_sources().is_empty(),
            "worker exits without viewers"
        );
    }
}
