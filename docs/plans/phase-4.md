# Phase 4 — Control surfaces & interaction

Goal: every way people drive a show beyond the keyboard — pointing and drawing from a phone,
sending files to the host, a second computer, MIDI controllers, OSC, an HTTP API and Stream Deck
via Bitfocus Companion. All of them go through the one action dispatcher with a role.

## Tasks

### Pointer and drawing
- [x] Presenters and operators move a laser pointer and draw on the slide from a remote (touch on
      the current-slide view) or from the operator window (mouse on the program monitor).
- [x] The pointer is ephemeral and high-rate: its own small message, rate-limited per device,
      hidden after a short idle time. It never bumps the show revision.
- [x] Drawings are part of the live state, bound to the slide they were drawn on, cleared on
      "clear drawing" and when the slide changes. Coordinates are normalized to the content
      area, so every output and monitor draws them in the same place.

### Upload inbox
- [x] Paired devices (presenter and up) upload PDFs, presentations, images, video and audio over
      HTTP with their device token; size limit and allowed types enforced by the server.
- [x] Uploads land in an **Inbox** visible to admins: name, size, who sent it; accept (adds a cue,
      optionally after the live cue) or reject (deletes the file). Admin uploads and devices
      with the auto-accept setting skip the inbox.

### API keys and HTTP API
- [x] Admins create API keys (a named "device" with a role, the token shown once, revocable like
      any device). They work for the WebSocket and for a small HTTP API: `POST /api/v1/action`
      and `GET /api/v1/state` (current cue, slide n/m, masters, timers).
- [x] The HTTP/WS API and OSC can be restricted to this computer (localhost only).

### MIDI and OSC (`crates/control`)
- [x] MIDI input from every connected port (`midir`): note on and control change messages map to
      actions; **learn** mode binds the next message to the chosen action. Mappings are host
      settings; MIDI acts with the operator role.
- [x] OSC over UDP (`rosc`): documented address space (`/midnightsnack/go`, `/next`, `/prev`,
      `/cue/{n}`, `/blackout {0|1}`, …), token-protected unless bound to localhost; feedback
      messages with the live state to subscribed clients.

### Controller mode
- [x] The host app can control another midnightsnack host: discover hosts via mDNS, pair with
      the host's join link and PIN, then run the full operator UI against it. Host-only parts
      (file dialogs, output windows) are hidden or replaced in that mode.

### Bitfocus Companion
- [x] A Companion module in `integrations/companion` using the WebSocket API with an API key:
      actions (go, next, prev, next/previous cue, go to cue, blackout/freeze/logo, panic,
      overlays), feedbacks (masters active, cue live), variables (cue name, slide n/m, timers).

### Docs and tests
- [x] Unit tests for pointer/drawing rules, inbox, API keys, MIDI mapping, OSC parsing; server
      tests for upload limits and auth, HTTP API; E2E for drawing and uploading from a phone;
      Companion module unit tests.
- [x] User docs: pointer/drawing, uploads, MIDI, OSC address space, HTTP API, Companion,
      controller mode. ADR for control surfaces; security model update.

## Acceptance criteria

1. Dragging a finger on the phone's current-slide view moves a pointer on the output within
   ~100 ms on a LAN; a drawn stroke stays until cleared or the slide changes.
2. A PDF sent from a presenter phone appears in the operator's inbox and becomes a cue after
   accepting; an oversized or disallowed file is rejected with a clear message.
3. A MIDI pad bound with learn advances the show; an OSC `/midnightsnack/blackout 1` blacks out
   and OSC feedback reports it.
4. `curl -H "Authorization: Bearer <key>" -d '{"action":"next"}' …/api/v1/action` advances; a
   revoked key gets 401.
5. A second computer in controller mode runs the show of the first.
6. The Companion module's actions, feedbacks and variables work against a running host (tested
   against the devserver).

## Results

- Rust: protocol 63, core 70, render 13, capture 3, control 8, server 31, host 5 tests (plus
  ignored ones that need LibreOffice, a sample deck or a display). Frontend: 13 unit tests,
  Companion module 7 (two of them against the development server), 10 Playwright E2E tests
  (new: drawing and pointing from a presenter phone; sending a file that the operator adds).
- Verified in the real host on Linux (WebKitGTK, X11):
  1. Drawing and the laser pointer with the mouse on the program monitor appear on both outputs
     and the monitors; the phone remote draws and points (E2E).
  2. A file uploaded with an API key appears in the inbox ("from Stream Deck") and becomes a cue
     on **Add**.
  3. An operator API key created in the Control tab runs `POST /api/v1/action` (blackout) and
     `GET /api/v1/state`; OSC on port 4748 sends feedback after `/subscribe` and
     `/midnightsnack/blackout 0` clears the blackout.
  4. Controller mode: pairing with a second host (the development server) by address and PIN,
     approved by its operator, opens its operator view; **GO** advances that host's show.
  5. The Companion module's client and state mapping drive the development server with an API
     key; an operator key is refused editing (`forbidden`).

## Known limitations and deviations

- **MIDI** could not be tested with hardware (no MIDI devices or ALSA sequencer in the
  development container). Decoding, edge detection and bindings are unit-tested; the listener
  rescans ports every 2 s. MIDI output (controller LEDs) is not implemented.
- **Companion module licensing:** Companion's packager (`companion-module-build`) only accepts
  modules whose source is MIT (and distributed as MIT, GPL-2.0-only or GPL-3.0-only). The module
  is GPL-3.0-or-later like the rest of the project, so it can be used as a developer module but
  not submitted to Companion's module list until the owner decides on its license.
- **Pointer coordinates** use a 16:9 frame; on outputs with *Fill* or *Stretch* scaling or slides
  with other aspect ratios the drawing is placed relative to that frame, not to the slide.
- **Controller mode** hides features that need the other computer's file system (open/save as,
  images) and output placement; **Save** works once the show has a file on the other host.
- **Uploads** are limited to 2 GB; the limit is not configurable in the UI yet.
