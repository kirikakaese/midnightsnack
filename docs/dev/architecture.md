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

Later phases add `crates/render` (PDF/office rendering and cache), `crates/server` (axum HTTP +
WebSocket, pairing), `crates/capture`, `crates/control` (MIDI/OSC/HTTP adapters),
`crates/relay` and `integrations/companion`.

See the ADRs in [../adr](../adr) for the reasoning behind these choices.
