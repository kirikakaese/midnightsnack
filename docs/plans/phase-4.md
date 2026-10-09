# Phase 4 — Control surfaces & interaction

Goal: every way people drive a show beyond the keyboard — pointing and drawing from a phone,
sending files to the host, a second computer, MIDI controllers, OSC, an HTTP API and Stream Deck
via Bitfocus Companion. All of them go through the one action dispatcher with a role.

## Tasks

### Pointer and drawing
- [ ] Presenters and operators move a laser pointer and draw on the slide from a remote (touch on
      the current-slide view) or from the operator window (mouse on the program monitor).
- [ ] The pointer is ephemeral and high-rate: its own small message, rate-limited per device,
      hidden after a short idle time. It never bumps the show revision.
- [ ] Drawings are part of the live state, bound to the slide they were drawn on, cleared on
      "clear drawing" and when the slide changes. Coordinates are normalized to the content
      area, so every output and monitor draws them in the same place.

### Upload inbox
- [ ] Paired devices (presenter and up) upload PDFs, presentations, images, video and audio over
      HTTP with their device token; size limit and allowed types enforced by the server.
- [ ] Uploads land in an **Inbox** visible to admins: name, size, who sent it; accept (adds a cue,
      optionally after the live cue) or reject (deletes the file). Admin uploads and devices
      with the auto-accept setting skip the inbox.

### API keys and HTTP API
- [ ] Admins create API keys (a named "device" with a role, the token shown once, revocable like
      any device). They work for the WebSocket and for a small HTTP API: `POST /api/v1/action`
      and `GET /api/v1/state` (current cue, slide n/m, masters, timers).
- [ ] The HTTP/WS API and OSC can be restricted to this computer (localhost only).

### MIDI and OSC (`crates/control`)
- [ ] MIDI input from every connected port (`midir`): note on and control change messages map to
      actions; **learn** mode binds the next message to the chosen action. Mappings are host
      settings; MIDI acts with the operator role.
- [ ] OSC over UDP (`rosc`): documented address space (`/midnightsnack/go`, `/next`, `/prev`,
      `/cue/{n}`, `/blackout {0|1}`, …), token-protected unless bound to localhost; feedback
      messages with the live state to subscribed clients.

### Controller mode
- [ ] The host app can control another midnightsnack host: discover hosts via mDNS, pair with
      the host's join link and PIN, then run the full operator UI against it. Host-only parts
      (file dialogs, output windows) are hidden or replaced in that mode.

### Bitfocus Companion
- [ ] A Companion module in `integrations/companion` using the WebSocket API with an API key:
      actions (go, next, prev, next/previous cue, go to cue, blackout/freeze/logo, panic,
      overlays), feedbacks (masters active, cue live), variables (cue name, slide n/m, timers).

### Docs and tests
- [ ] Unit tests for pointer/drawing rules, inbox, API keys, MIDI mapping, OSC parsing; server
      tests for upload limits and auth, HTTP API; E2E for drawing and uploading from a phone;
      Companion module unit tests.
- [ ] User docs: pointer/drawing, uploads, MIDI, OSC address space, HTTP API, Companion,
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
