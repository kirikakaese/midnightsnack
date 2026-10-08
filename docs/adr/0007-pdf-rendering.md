# 0007. PDF rendering with PDFium at runtime

- **Status:** accepted
- **Date:** 2026-10-08

## Context

Slides must look exactly like the author's PDF at projector resolution and switch instantly.
Rendering in the webview with pdf.js is possible but slower for large pages and differs between
webviews.

## Decision

- PDFs are rendered natively with **PDFium** through `pdfium-render`, loaded dynamically at
  runtime. Prebuilt binaries (bblanchon/pdfium-binaries, chromium/7881) are downloaded by
  `scripts/fetch-pdfium.mjs` and bundled as a Tauri resource; `MIDNIGHTSNACK_PDFIUM` overrides
  the location. If PDFium cannot be loaded, images still work and PDF actions report
  `pdf_engine_missing`.
- Rendering runs on one worker thread with two priority queues (on-demand before prefetch).
  Results are cached on disk keyed by file path, size, modification time, page and target size,
  and written atomically.
- PDF pages are encoded as PNG, photos as JPEG (alpha composited onto black).
- Annotations are not rendered, so sticky-note speaker notes never reach the audience.

## Consequences

- PDF annotations that are meant to be visible (ink, stamps) are not shown — tracked in
  `docs/ideas.md` as a per-cue option.
- pdf.js as a fallback engine is not implemented; it is listed in `docs/ideas.md`.
