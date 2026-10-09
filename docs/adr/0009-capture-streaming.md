# 0009. Screen capture as a shared MJPEG stream

- **Status:** accepted
- **Date:** 2026-10-09

## Context

Capture cues show a screen or a window live on the outputs, on the operator's monitors and on
phones. Every view is a webview (or a browser) that already talks HTTP to the embedded server.
Options were WebRTC (low latency, but a large dependency and signalling for a LAN-only
feature), a native video surface in the output window (no preview on phones, a second rendering
path), or frames over HTTP.

## Decision

- `crates/capture` grabs frames with `xcap` (macOS ScreenCaptureKit/CoreGraphics, Windows
  DXGI/GDI, Linux X11 and Wayland). On X11 a persistent `x11rb` connection reads the screen with
  `GetImage`, because opening a connection per frame was slow and unreliable.
- One worker thread per source, started when the first viewer subscribes and stopped a few
  seconds after the last one leaves. Frames are scaled down to at most 1920×1200 and encoded once
  as JPEG (quality 80); all viewers share the encoded frame through a `tokio::sync::watch`
  channel.
- The server streams them as `multipart/x-mixed-replace` (MJPEG) at
  `GET /api/v1/media/capture/{cue_id}?k=&fps=`, behind the same per-connection media key as
  slides. Each viewer chooses its own rate up to the cue's (thumbnails and phones ask for 2 fps).
- Browsers render MJPEG in a plain `<img>`, which keeps the last frame when the stream stalls.
  A host task polls the workers and sets `LiveState.capture_lost` for cues whose source is gone,
  so the operator is told while the audience keeps the last picture.
- Listing sources (`list_capture_targets`) is admin-only, because window titles can be private.

## Consequences

- Latency is one capture plus one JPEG encode (≈ 20 ms at 1600×1000 on the development machine,
  ≈ 28 fps); good enough for slides, spreadsheets and demos, not for fast games.
- No audio is captured.
- macOS needs the Screen Recording permission; the operator UI links to the settings page.
- A native capture surface for the output window can replace the `<img>` later without changing
  the protocol.
