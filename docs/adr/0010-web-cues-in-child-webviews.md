# 0010. Web page cues in child webviews

- **Status:** accepted
- **Date:** 2026-10-09

## Context

Web page cues (reveal.js decks, OpenSlides projectors, result boards) must show live remote pages
on the outputs. An `<iframe>` inside the output page is blocked by most sites
(`X-Frame-Options`, `frame-ancestors`), shares the output's process and cannot keep a separate
login. The page must never get access to the host's IPC or the show.

## Decision

- Output windows are plain Tauri windows that hold child webviews (Tauri's multi-webview
  support, `unstable` feature): the output page itself, plus one webview per web cue that is live
  on that output or next.
- `apps/host/src-tauri/src/web.rs` follows the engine: it creates, shows, hides and closes the
  web views when the show or live state changes. The next web cue is loaded hidden so it appears
  without a loading screen. Under blackout, logo or a test pattern the web view is hidden and the
  output page underneath draws the master state.
- Web views load the remote URL directly, get no IPC capabilities and run either incognito or
  with a data directory per cue ("keep logins and cookies"). "Stay on this site" is enforced in
  `on_navigation` (same scheme, host and port).
- Next/previous on a web cue with key forwarding are counted in `LiveState.web_nav`; the host
  turns each step into a synthetic `keydown` (→/← with `key`, `code` and `keyCode`) dispatched on
  the page's `document`, which reveal.js and similar frameworks handle.
- Monitors and phones show a placeholder with the address instead of the page.

## Consequences

- On Linux, Tauri packs child webviews into a GTK box, so they are stacked rather than layered.
  The host hides the output page while a web view is shown, which gives the page the whole
  window; switching between a web page and other content is therefore always a cut there.
- Overlays are not drawn over web pages.
- Pages that only react to trusted (real) key events do not advance; the operator can still move
  on with **CUE ⏭**.
