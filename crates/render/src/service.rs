// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::VecDeque;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::DynamicImage;
use tokio::sync::{oneshot, watch};

use crate::cache::RenderCache;
use crate::images::render_image;
use crate::pdf::{self, PdfInfo};
use crate::{RenderError, SlideSource, TargetSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Someone is waiting for this slide right now.
    Now,
    /// Rendering ahead of time.
    Prefetch,
}

enum Job {
    Render {
        source: SlideSource,
        size: TargetSize,
        reply: Option<oneshot::Sender<Result<PathBuf, RenderError>>>,
    },
    Inspect {
        path: PathBuf,
        reply: oneshot::Sender<Result<PdfInfo, RenderError>>,
    },
}

#[derive(Default)]
struct Queues {
    now: VecDeque<Job>,
    prefetch: VecDeque<Job>,
    stopped: bool,
}

struct Shared {
    queues: Mutex<Queues>,
    wake: Condvar,
    queued: watch::Sender<u32>,
}

impl Shared {
    fn update_count(&self, q: &Queues) {
        self.queued
            .send_replace((q.now.len() + q.prefetch.len()) as u32);
    }
}

/// Handle to the render worker thread. Cheap to clone.
#[derive(Clone)]
pub struct RenderService {
    shared: Arc<Shared>,
    cache: RenderCache,
}

impl RenderService {
    /// Starts the worker. PDFium is looked up in `pdfium_dirs` (see [`pdf::locate_pdfium`]);
    /// if it is missing, image rendering still works and PDF requests fail with
    /// [`RenderError::PdfEngineMissing`].
    pub fn start(cache: RenderCache, pdfium_dirs: Vec<PathBuf>) -> Self {
        let (queued, _) = watch::channel(0);
        let shared = Arc::new(Shared {
            queues: Mutex::new(Queues::default()),
            wake: Condvar::new(),
            queued,
        });
        let worker_shared = shared.clone();
        let worker_cache = cache.clone();
        std::thread::Builder::new()
            .name("midnightsnack-render".into())
            .spawn(move || worker(worker_shared, worker_cache, pdfium_dirs))
            .expect("spawn render thread");
        RenderService { shared, cache }
    }

    pub fn cache(&self) -> &RenderCache {
        &self.cache
    }

    /// Number of queued jobs, for progress display.
    pub fn subscribe_queued(&self) -> watch::Receiver<u32> {
        self.shared.queued.subscribe()
    }

    fn push(&self, job: Job, priority: Priority) {
        let mut q = self.shared.queues.lock().expect("render queue poisoned");
        match priority {
            Priority::Now => q.now.push_back(job),
            Priority::Prefetch => {
                // Keep the prefetch queue bounded; drop the oldest speculative work.
                if q.prefetch.len() >= 256 {
                    q.prefetch.pop_front();
                }
                q.prefetch.push_back(job)
            }
        }
        self.shared.update_count(&q);
        drop(q);
        self.shared.wake.notify_one();
    }

    /// Renders (or fetches from cache) a slide and returns the path of the image file.
    pub async fn render(
        &self,
        source: SlideSource,
        size: TargetSize,
    ) -> Result<PathBuf, RenderError> {
        let path = self.cache.path_for(&source, size)?;
        if path.is_file() {
            return Ok(path);
        }
        let (tx, rx) = oneshot::channel();
        self.push(
            Job::Render {
                source,
                size,
                reply: Some(tx),
            },
            Priority::Now,
        );
        rx.await.map_err(|_| RenderError::WorkerGone)?
    }

    /// Queues a slide for background rendering.
    pub fn prefetch(&self, source: SlideSource, size: TargetSize) {
        if self
            .cache
            .path_for(&source, size)
            .is_ok_and(|p| p.is_file())
        {
            return;
        }
        self.push(
            Job::Render {
                source,
                size,
                reply: None,
            },
            Priority::Prefetch,
        );
    }

    /// Drops all queued background work (e.g. when a different show is loaded).
    pub fn cancel_prefetch(&self) {
        let mut q = self.shared.queues.lock().expect("render queue poisoned");
        q.prefetch.clear();
        self.shared.update_count(&q);
    }

    /// Page count and notes of a PDF.
    pub async fn inspect_pdf(&self, path: PathBuf) -> Result<PdfInfo, RenderError> {
        let (tx, rx) = oneshot::channel();
        self.push(Job::Inspect { path, reply: tx }, Priority::Now);
        rx.await.map_err(|_| RenderError::WorkerGone)?
    }

    pub fn pdf_available(&self) -> bool {
        pdf::pdfium_available()
    }
}

fn worker(shared: Arc<Shared>, cache: RenderCache, pdfium_dirs: Vec<PathBuf>) {
    // Load PDFium eagerly so the first slide is not delayed.
    let _ = pdf::init_pdfium(&pdfium_dirs);
    loop {
        let job = {
            let mut q = shared.queues.lock().expect("render queue poisoned");
            loop {
                if q.stopped {
                    return;
                }
                if let Some(j) = q.now.pop_front().or_else(|| q.prefetch.pop_front()) {
                    shared.update_count(&q);
                    break j;
                }
                // Exit once every service handle has been dropped.
                if Arc::strong_count(&shared) == 1 {
                    q.stopped = true;
                    continue;
                }
                q = shared
                    .wake
                    .wait_timeout(q, std::time::Duration::from_secs(5))
                    .expect("render queue poisoned")
                    .0;
            }
        };
        match job {
            Job::Render {
                source,
                size,
                reply,
            } => {
                let result = render_to_cache(&cache, &source, size, &pdfium_dirs);
                if let Err(e) = &result {
                    tracing::warn!(?source, error = %e, "render failed");
                }
                if let Some(reply) = reply {
                    let _ = reply.send(result);
                }
            }
            Job::Inspect { path, reply } => {
                let result = pdf::init_pdfium(&pdfium_dirs).and_then(|p| pdf::inspect(p, &path));
                let _ = reply.send(result);
            }
        }
    }
}

fn render_to_cache(
    cache: &RenderCache,
    source: &SlideSource,
    size: TargetSize,
    pdfium_dirs: &[PathBuf],
) -> Result<PathBuf, RenderError> {
    let out = cache.path_for(source, size)?;
    if out.is_file() {
        return Ok(out);
    }
    let img = match source {
        SlideSource::PdfPage { path, page } => {
            let pdfium = pdf::init_pdfium(pdfium_dirs)?;
            pdf::render_page(pdfium, path, *page, size)?
        }
        SlideSource::Image { path } => render_image(path, size)?,
    };
    write_atomic(&out, &img)?;
    Ok(out)
}

/// Writes next to the destination and renames, so a half-written file is never served.
fn write_atomic(out: &Path, img: &DynamicImage) -> Result<(), RenderError> {
    let dir = out.parent().expect("cache path has a parent");
    std::fs::create_dir_all(dir)?;
    let tmp = tempfile::NamedTempFile::new_in(dir)?;
    {
        let w = BufWriter::new(tmp.as_file());
        if out.extension().is_some_and(|e| e == "png") {
            let enc = PngEncoder::new_with_quality(w, CompressionType::Fast, FilterType::Adaptive);
            img.to_rgba8().write_with_encoder(enc)?;
        } else {
            let enc = JpegEncoder::new_with_quality(w, 90);
            flatten(img).write_with_encoder(enc)?;
        }
    }
    tmp.persist(out).map_err(|e| RenderError::Io(e.error))?;
    Ok(())
}

/// Composites transparency onto black (the output background).
fn flatten(img: &DynamicImage) -> image::RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = p[3] as u16;
        image::Rgb([
            (p[0] as u16 * a / 255) as u8,
            (p[1] as u16 * a / 255) as u8,
            (p[2] as u16 * a / 255) as u8,
        ])
    })
}
