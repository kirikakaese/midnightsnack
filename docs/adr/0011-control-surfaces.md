# 0011. Control surfaces

- **Status:** accepted
- **Date:** 2026-10-09

## Context

Phase 4 adds many ways to drive a show: laser pointer and drawing from phones, uploads, MIDI,
OSC, an HTTP API, Bitfocus Companion and a second computer running the operator UI. Each must
respect roles, and none may become a back door around pairing.

## Decision

- **One dispatcher, one role per source.** Every surface produces protocol `Action`s that go
  through `AppState::perform` with an `Origin` (role, local). MIDI acts as operator; OSC senders
  on the host as operator, others with the role of the key they authenticated with.
- **API keys are devices.** A key is a remembered device flagged `api_key`, created by an admin
  with a role, its token shown once and stored hashed. Keys work for the WebSocket, the HTTP API
  (`POST /api/v1/action`, `GET /api/v1/state`, uploads) and OSC authentication. A host setting
  (on by default) only accepts them from the host itself; the OSC server then binds to loopback.
- **Compact state for surfaces.** `Engine::summary` produces a `StateSummary` (live cue with
  1-based numbers, masters, timers) used by the HTTP API and OSC feedback; Companion computes the
  same from the full state it mirrors.
- **Pointer is not state.** Laser pointer positions are ephemeral `pointer` messages relayed to
  other connections (rate-limited, never bumping revisions). Finished strokes are `draw_stroke`
  actions stored in the live state and bound to the slide on the main output, so every output
  and late joiner sees the same drawing. Coordinates are fractions of a 16:9 frame.
- **Uploads go through an inbox.** Files are streamed to disk with a size limit and a type
  allow-list, then wait for an admin unless the sender is an admin or auto-accept is on.
  Accepting reuses the regular file import.
- **MIDI lives in the host app, OSC in the server.** MIDI is about hardware plugged into this
  computer (bindings are host settings, learn mode is a host command). OSC needs live state and
  authentication, so it runs next to the WebSocket server (and in the development server).
  Parsing and mapping for both are pure functions in `crates/control`.
- **Controller mode reuses the operator UI.** The host app pairs with another host like a phone
  (PIN; the other operator approves) and opens a window whose connection info points at the other
  host. Host-only features (file dialogs, output placement, MIDI) are hidden there; files are
  uploaded instead. Pairing without the join token is allowed only when approval is required.
- **Companion module** in `integrations/companion`, using `@companion-module/base` and the
  WebSocket API with an API key; its state mapping is unit-tested and its client is tested
  against the development server.

## Consequences

- Controller windows need a Content-Security-Policy that allows other hosts on the network.
- Companion's module store requires modules under MIT; distributing the module there needs a
  licensing decision by the project owner (see the phase 4 plan). It can be used as a developer
  module meanwhile.
- MIDI output (LED feedback on controllers) is not implemented yet.
