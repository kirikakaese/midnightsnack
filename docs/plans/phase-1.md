# Phase 1 — MVP

Goal: run a real show from PDF and image files on one projector, controlled from the laptop
keyboard, a presentation clicker and a paired phone.

## Tasks

### Core (`crates/core`)
- [ ] Show model: cues (PDF deck, image, image folder, blank), notes, color tags, linked vs.
      bundled media references.
- [ ] Cue engine: program position, next/prev across slides and cues, jump to cue/slide,
      master states (blackout, freeze, logo, panic), show timer and per-slide timer.
- [ ] Freeze semantics: the output keeps showing the frozen position while the operator moves on.
- [ ] Roles and permission table; presenters can only move within the current cue.
- [ ] Single action dispatcher used by every input source.
- [ ] `.msnack` bundle read/write (zip: `show.json` + `media/`), embedded and linked modes.

### Rendering (`crates/render`)
- [ ] PDFium loaded at runtime (bundled per platform; `MIDNIGHTSNACK_PDFIUM` override).
- [ ] PDF page count, page rendering to the requested size, speaker notes from annotations and
      from a sidecar `<deck>.notes.md` file.
- [ ] Image and image folder decoding and scaling.
- [ ] Disk cache keyed by file identity, page and size; prefetch of the next three slides.

### Server (`crates/server`)
- [ ] axum HTTP + WebSocket server embedded in the host; serves the web remote from the binary.
- [ ] Pairing: QR join URL with a one-time token → PIN → operator approval (or auto-approve
      role) → per-device session token. Remembered devices, revoke, disconnect all.
- [ ] PIN rate limiting and lockout.
- [ ] State broadcast to every client; per-role filtering.
- [ ] Slide image endpoint (thumbnail and output sizes), per-session media key.
- [ ] mDNS advertisement `_midnightsnack._tcp`.
- [ ] Autosave of the working show and position; restore on launch.

### Host (`apps/host`)
- [ ] Operator view: cue list (add, reorder, remove, rename), program and preview, current and
      next slide, notes, show/slide timers, wall clock, big GO / PREV / NEXT, BLACKOUT, FREEZE,
      LOGO buttons, connected devices with approve/kick, QR code and PIN.
- [ ] Output window on the chosen display: borderless fullscreen, hidden cursor, never steals
      focus, swaps only fully decoded frames, holds the last frame on errors; screen sleep
      prevented while the output is open. Windowed output for rehearsal on one screen.
- [ ] Keyboard and clicker control in operator and output windows (configurable keymap with
      defaults: Space/→/PageDown/↓ next, ←/PageUp/↑ prev, B or `.` blackout, F freeze, L logo,
      Esc panic-to-logo, F5 go).
- [ ] New / open / save / save as.

### Remote (`apps/remote`)
- [ ] Join and pairing flow (token from QR, PIN entry, waiting for approval), auto-reconnect,
      clear connection state.
- [ ] Role-dependent control: next/prev, current slide thumbnail, notes, timer,
      blackout/freeze/logo and cue list for operators.
- [ ] Stage viewer layout (read-only current/next, notes, big timer).

### Tests and docs
- [ ] Unit tests for engine, permissions, dispatcher and bundle format.
- [ ] Integration tests for pairing and WebSocket control.
- [ ] Playwright E2E for the remote against a headless development server.
- [ ] User docs: getting started, remotes and pairing, keyboard and clickers. Security model in
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

## Out of scope (later phases)

Multiple outputs, transitions, video, overlays, HTTPS, relay, hardware MIDI/OSC.
