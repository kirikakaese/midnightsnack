# Architecture

```
            ┌──────────── host (Tauri app) ─────────────────────────────┐
 clicker ─▶ │  operator window   output window(s)                       │
 keyboard   │        │ IPC / WS        │ WS                              │
            │        ▼                 ▼                                 │
            │  ┌───────────── action dispatcher (crates/core) ────────┐  │
 phone  ─WS▶│  │ permissions → cue engine → state → broadcast         │  │
 browser    │  └──────────────────────────────────────────────────────┘  │
 MIDI/OSC ─▶│  embedded server (axum) · render cache · discovery (mDNS) │
            └───────────────────────────────────────────────────────────┘
```

## Principles

- **One dispatcher.** Every input source — operator UI, keyboard/clicker, web remotes, MIDI,
  OSC, HTTP — produces an `Action` that is checked against the device's `Role` and applied by the
  cue engine in `crates/core`. Identical behavior and permissions everywhere.
- **State flows one way.** The engine owns the show state; changes are broadcast as snapshots or
  patches to every window and remote. Clients never mutate state locally.
- **Outputs are dumb and defensive.** Output windows render the state they are given, only swap
  to new content once it is fully loaded, and hold the last good frame on any error.
- **Pure core.** `crates/core` has no I/O beyond (de)serialization and is unit-tested.

## Components

| Component          | Responsibility                                                      |
| ------------------ | ------------------------------------------------------------------- |
| `crates/protocol`  | Message and state types shared with every client; TS generation      |
| `crates/core`      | Show model, cue engine, master states, roles/permissions, dispatcher |
| `apps/host`        | Windows, displays, OS integration, wiring of all services           |
| `apps/remote`      | Browser remote served by the host                                    |
| `packages/ui`      | Components, design tokens, i18n shared by all frontends              |
| `crates/render`    | PDF (PDFium) and image rendering, render cache, office → PDF conversion |
| `crates/server`    | Embedded axum HTTP + WebSocket server, pairing, devices, media, autosave, HTTPS listener, relay link (Noise responder, tunnel) |
| `crates/integrations/openslides` | OpenSlides 4 adapter: login, autoupdate subscription, meeting views, HTML to text blocks, mock server ([ADR 0013](../adr/0013-openslides-adapter.md)) |
| `crates/relay`     | Optional self-hosted relay: forwards encrypted channels between remotes and hosts, serves the web remote |
| `crates/capture`   | Screen and window capture, shared JPEG frame workers               |
| `crates/control`   | MIDI decoding/bindings and port listener, OSC address space and feedback |
| `integrations/companion` | Bitfocus Companion module (WebSocket client with an API key)  |

Output windows hold child webviews: the output page and, for web page cues, the remote page
([ADR 0010](../adr/0010-web-cues-in-child-webviews.md)). Control surfaces (MIDI, OSC, the HTTP
API, Companion, controller mode) are described in
[ADR 0011](../adr/0011-control-surfaces.md). Connectivity (HTTPS, hotspot, the relay and its
end-to-end encryption) is described in [ADR 0012](../adr/0012-relay-and-connectivity.md): the
web remote talks to the host through a transport (direct or relay tunnel), and the server runs
the same WebSocket session code over both.

See the ADRs in [../adr](../adr) for the reasoning behind these choices.
