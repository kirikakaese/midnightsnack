// SPDX-License-Identifier: GPL-3.0-or-later
//! Rendering tests. PDF tests need PDFium: run `node scripts/fetch-pdfium.mjs` first or set
//! `MIDNIGHTSNACK_PDFIUM`. Without it they are skipped (CI always provides it).

use std::path::PathBuf;

use midnightsnack_render::test_support::{build_pdf, TestPage};
use midnightsnack_render::{RenderCache, RenderService, SlideSource, TargetSize};

fn pdfium_dirs() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    vec![root.join("apps/host/src-tauri/resources/pdfium")]
}

fn service(dir: &std::path::Path) -> RenderService {
    RenderService::start(RenderCache::new(dir.join("cache")), pdfium_dirs())
}

fn pdfium_present(svc: &RenderService) -> bool {
    if !svc.pdf_available() {
        if std::env::var_os("MIDNIGHTSNACK_REQUIRE_PDFIUM").is_some() {
            panic!("PDFium required but not found");
        }
        eprintln!("PDFium not found, skipping PDF test");
        return false;
    }
    true
}

#[tokio::test]
async fn renders_pdf_pages_and_reads_notes() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("deck.pdf");
    std::fs::write(
        &pdf,
        build_pdf(&[
            TestPage::landscape([1.0, 0.0, 0.0]).with_note("First (slide)"),
            TestPage::landscape([0.0, 0.0, 1.0]),
            TestPage::landscape([0.0, 1.0, 0.0]).with_note("Third"),
        ]),
    )
    .unwrap();

    let svc = service(dir.path());
    let info = svc.inspect_pdf(pdf.clone()).await;
    if !pdfium_present(&svc) {
        return;
    }
    let info = info.unwrap();
    assert_eq!(info.page_count, 3);
    assert_eq!(info.notes, vec!["First (slide)", "", "Third"]);

    let size = TargetSize::new(1920, 1080);
    let out = svc
        .render(
            SlideSource::PdfPage {
                path: pdf.clone(),
                page: 1,
            },
            size,
        )
        .await
        .unwrap();
    let img = image::open(&out).unwrap().to_rgb8();
    assert_eq!((img.width(), img.height()), (1920, 1080));
    let px = img.get_pixel(1800, 100).0;
    assert!(px[2] > 200 && px[0] < 50, "page 2 is blue, got {px:?}");

    // Cached: second call returns the same file without re-rendering.
    let again = svc
        .render(
            SlideSource::PdfPage {
                path: pdf.clone(),
                page: 1,
            },
            size,
        )
        .await;
    assert_eq!(again.unwrap(), out);

    let err = svc
        .render(SlideSource::PdfPage { path: pdf, page: 9 }, size)
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn sidecar_notes_override_annotations() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("talk.pdf");
    std::fs::write(
        &pdf,
        build_pdf(&[TestPage::landscape([1.0; 3]).with_note("annot")]),
    )
    .unwrap();
    std::fs::write(dir.path().join("talk.notes.md"), "From sidecar\n---\nTwo").unwrap();
    let svc = service(dir.path());
    let info = svc.inspect_pdf(pdf).await;
    if !pdfium_present(&svc) {
        return;
    }
    assert_eq!(info.unwrap().notes, vec!["From sidecar", "Two"]);
}

#[tokio::test]
async fn renders_images_without_pdfium() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    image::RgbaImage::from_pixel(100, 50, image::Rgba([0, 255, 0, 255]))
        .save(&png)
        .unwrap();
    let svc = service(dir.path());
    let out = svc
        .render(SlideSource::Image { path: png }, TargetSize::new(480, 270))
        .await
        .unwrap();
    assert_eq!(out.extension().unwrap(), "jpg");
    let img = image::open(out).unwrap();
    assert_eq!((img.width(), img.height()), (480, 240));
}

#[tokio::test]
async fn broken_files_fail_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.png");
    std::fs::write(&bad, b"not an image").unwrap();
    let svc = service(dir.path());
    assert!(svc
        .render(SlideSource::Image { path: bad }, TargetSize::new(100, 100))
        .await
        .is_err());
    let missing = dir.path().join("missing.png");
    assert!(svc
        .render(
            SlideSource::Image { path: missing },
            TargetSize::new(100, 100)
        )
        .await
        .is_err());
}
