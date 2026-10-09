# Phase 3 — Outputs & sources

Goal: several screens at once, robust display handling, and every kind of source a show needs —
live screen capture, web pages, office presentations and OpenSlides.

## Tasks

### Outputs
- [x] Output definitions in the show: name, feed (program or stage display), overlays on/off,
      scaling (fit / fill / stretch) and safe margin. Display assignment and windowed mode are
      host settings, remembered by display name.
- [x] Cue targets: a cue goes to all program outputs or to a chosen subset; an output keeps
      showing its last targeted cue while others change.
- [x] One window per output; the operator view shows a monitor for every output.
- [x] Display hotplug: detect monitors appearing and disappearing; if an output's display is
      gone, keep running and warn the operator; move the output back when the display returns.
- [x] Test patterns per show (grid, color bars, resolution text) for setup.

### Sources
- [x] Screen and window capture (`xcap`): MJPEG stream from the embedded server, started on
      demand and shared by all viewers; "capture lost" state for the operator while the output
      holds the last frame; guided macOS Screen Recording permission.
- [x] Web page cues in an isolated child webview of the output window: zoom, optional blocking of
      navigation away, next/prev forwarded as arrow keys, optional persistent session (login),
      preloaded while the cue is next.
- [x] OpenSlides projector URL cue: a web cue preset with persistent session and zoom.
- [x] PPTX/ODP → PDF via headless LibreOffice (detected per OS, clear hint if missing), speaker
      notes extracted from the file; Keynote → PDF via AppleScript on macOS, guidance elsewhere.
      Conversions are cached by file identity and redone when the file changes.

### Docs and tests
- [x] Engine tests for targets and per-output positions; server tests for conversion detection
      and capture endpoint auth; E2E for multiple-output state on the remote.
- [x] User docs: outputs and displays, capture (incl. permissions), web/OpenSlides, office files.
      ADRs for capture streaming and child webviews.

## Acceptance criteria

1. Two outputs (main + second room) show different cues when targets differ; a stage display
   output shows the stage view.
2. Unplugging the projector does not crash or move anything onto the laptop screen silently; the
   operator sees a warning; re-plugging restores the output.
3. A captured window appears on the output with under 150 ms latency on a typical laptop; if the
   window closes, the last frame stays and the operator is told.
4. A reveal.js URL cue advances with the clicker; navigation away is blocked when set.
5. A PPTX with speaker notes becomes a PDF cue with the same notes; without LibreOffice the
   operator gets an actionable hint.

## Results

- Rust: protocol 51, core 64, render 13, capture 3, server 23, host 3 tests (plus ignored tests
  that need LibreOffice, a sample deck or a display). Frontend: 13 unit tests, 8 Playwright E2E
  tests (the new one covers cue targets across two outputs, a web cue and test patterns).
- Verified in the real host on Linux (WebKitGTK, X11) with two windowed outputs:
  1. A web cue targeted at **Main** only goes live there while **Output 2** keeps its previous
     cue; switching Output 2 to the stage feed shows the stage display in that window at once.
  2. Test patterns (grid, bars) appear on both outputs with name and resolution and on the
     program monitor.
  3. Screen capture appears on both outputs and the monitors at ≈ 28 fps (1600×1000, JPEG
     encode ≈ 18 ms per frame).
  4. A local test page receives NEXT as `ArrowRight`/`keyCode 39`; a link to another site is
     blocked; blackout covers the page and the page keeps its state afterwards.
  5. A three-slide PPTX with notes on slides 1 and 3 converts in ≈ 2 s with LibreOffice 24 and
     keeps both notes; a Keynote file on Linux reports `converter_missing`.
  6. Settings from phase 2 (single output window) migrate to the **Main** output; open outputs
     and the live capture come back after a restart.

## Known limitations and deviations

- **Display hotplug** could not be exercised with real hardware in CI or the development
  container (Xvfb has no hot-pluggable outputs). The decision logic is unit-tested; the
  watcher polls the display list every 2 s.
- **Linux web pages** are stacked, not layered, by Tauri's GTK multi-webview, so the output page
  is hidden while a web page shows: switching to and from a web page is always a cut there
  (ADR 0010). Overlays are not drawn over web pages on any platform.
- **Capture** has no audio; window capture on Wayland is untested (X11 works). Window lists may
  be empty under minimal window managers.
- **Office fidelity** is LibreOffice's: missing fonts are substituted and animations are
  flattened. Speaker notes come from PPTX and ODP only.
- Test patterns are global for all program outputs, not chosen per output.
