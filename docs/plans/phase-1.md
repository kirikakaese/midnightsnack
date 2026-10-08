# Phase 1 — MVP

Goal: run a real show from PDF and image files on one projector, controlled from the laptop
keyboard, a presentation clicker and a paired phone.

## Tasks

### Core (`crates/core`)
- [x] Show model: cues (PDF deck, image, image folder, blank), notes, color tags, linked vs.
      bundled media references.
- [x] Cue engine: program position, next/prev across slides and cues, jump to cue/slide,
      master states (blackout, freeze, logo, panic), show timer and per-slide timer.
- [x] Freeze semantics: the output keeps showing the frozen position while the operator moves on.
- [x] Roles and permission table; presenters can only move within the current cue.
- [x] Single action dispatcher used by every input source.
- [x] `.msnack` bundle read/write (zip: `show.json` + `media/`), embedded and linked modes.

### Rendering (`crates/render`)
- [x] PDFium loaded at runtime (bundled per platform; `MIDNIGHTSNACK_PDFIUM` override).
- [x] PDF page count, page rendering to the requested size, speaker notes from annotations and
      from a sidecar `<deck>.notes.md` file.
- [x] Image and image folder decoding and scaling.
- [x] Disk cache keyed by file identity, page and size; prefetch of the next three slides.

### Server (`crates/server`)
- [x] axum HTTP + WebSocket server embedded in the host; serves the web remote from the binary.
- [x] Pairing: QR join URL with a one-time token → PIN → operator approval (or auto-approve
      role) → per-device session token. Remembered devices, revoke, disconnect all.
- [x] PIN rate limiting and lockout.
- [x] State broadcast to every client; per-role filtering.
- [x] Slide image endpoint (thumbnail and output sizes), per-session media key.
- [x] mDNS advertisement `_midnightsnack._tcp`.
- [x] Autosave of the working show and position; restore on launch.

### Host (`apps/host`)
- [x] Operator view: cue list (add, reorder, remove, rename), program and preview, current and
      next slide, notes, show/slide timers, wall clock, big GO / PREV / NEXT, BLACKOUT, FREEZE,
      LOGO buttons, connected devices with approve/kick, QR code and PIN.
- [x] Output window on the chosen display: borderless fullscreen, hidden cursor, never steals
      focus, swaps only fully decoded frames, holds the last frame on errors; screen sleep
      prevented while the output is open. Windowed output for rehearsal on one screen.
- [x] Keyboard and clicker control in operator and output windows (configurable keymap with
      defaults: Space/→/PageDown/↓ next, ←/PageUp/↑ prev, B or `.` blackout, F freeze, L logo,
      Esc panic-to-logo, F5 go).
- [x] New / open / save / save as.

### Remote (`apps/remote`)
- [x] Join and pairing flow (token from QR, PIN entry, waiting for approval), auto-reconnect,
      clear connection state.
- [x] Role-dependent control: next/prev, current slide thumbnail, notes, timer,
      blackout/freeze/logo and cue list for operators.
- [x] Stage viewer layout (read-only current/next, notes, big timer).

### Tests and docs
- [x] Unit tests for engine, permissions, dispatcher and bundle format.
- [x] Integration tests for pairing and WebSocket control.
- [x] Playwright E2E for the remote against a headless development server.
- [x] User docs: getting started, remotes and pairing, keyboard and clickers. Security model in
      `docs/security.md`. Protocol spec in `docs/dev/protocol.md`.

## Acceptance criteria

1. A PDF and an image folder can be added, reordered and presented on a second display; the
   output never shows UI chrome, a cursor or a partially loaded slide.
2. Blackout, freeze, logo and panic work from the keyboard, a clicker (arrow/PageUp/PageDown/B
   keys), the operator buttons and a phone.
3. A phone joins by scanning the QR code and entering the PIN, after operator approval; a wrong
   PIN five times locks pairing for that client for 60 s.
4. A presenter-role phone cannot leave the current cue; a stage viewer cannot control anything.
5. Next-slide latency on localhost is below 100 ms with prefetched slides.
6. A show saved as `.msnack` (embedded) opens on another machine with all media.

## Demo checklist

- [ ] `node scripts/fetch-pdfium.mjs && pnpm build:remote && pnpm dev` opens the operator view.
- [ ] Add a PDF, an image and a folder of images; reorder, rename, color-tag a cue.
- [ ] Open the output on a second display (or windowed); step through slides with the arrow keys
      and a clicker; check notes, next-slide preview and timers.
- [ ] Blackout (B), freeze (F) then move on, logo (L), panic (Esc).
- [ ] Scan the QR code with a phone, enter the PIN, approve as presenter; next/prev stays inside
      the cue. Promote to operator; blackout from the phone.
- [ ] Save as `.msnack`, quit, delete the media, reopen: everything is there.
- [ ] Kill the app mid-show; restart: the show and slide are restored.

## Results

- Rust: protocol 27, core 34, render 9, server 17 tests. Frontend: 8 unit tests, 5 Playwright
  E2E tests against the headless development server.
- Latency (debug build, localhost): action → live update median 1.4 ms, p95 3.1 ms; fetching
  the prefetched next slide at 1080p median 3.6 ms.

## Known limitations and deviations

- **Keymap configuration** is file-based (`host-settings.json`, documented in
  `docs/user/keyboard.md`); an editor UI is listed in `docs/ideas.md`.
- **Transport security:** LAN traffic is plain HTTP/WebSocket; see `docs/security.md`. HTTPS
  with a generated certificate moves to phase 5 together with the relay.
- **Wayland:** the output window cannot position itself on a chosen display under Wayland
  compositors (ADR 0006); handled in phase 3.
- **Screen-sleep prevention** needs a session D-Bus on Linux; without one a warning is logged.

## Out of scope (later phases)

Multiple outputs, transitions, video, overlays, HTTPS, relay, hardware MIDI/OSC.
