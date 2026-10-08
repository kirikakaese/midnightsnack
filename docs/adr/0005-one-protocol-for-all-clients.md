# 0005. Host windows use the same protocol as remotes

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The operator window, the output windows and remote devices all need show state and send
actions. Tauri offers IPC commands and events, which would give the host's own windows a
second, private API.

## Decision

The embedded server (`crates/server`) is the only gateway to the core. The host's windows
connect to it over `ws://127.0.0.1` with per-launch tokens that are only accepted from loopback
addresses: the operator window as **admin**, output windows as **operator**. Tauri IPC is used
only for things that are inherently local: window and display management, file dialogs, QR
rendering, settings.

The server lives in its own crate so it can be tested without a desktop environment and run
headless (`midnightsnack-devserver`) for remote development and E2E tests.

## Consequences

- One code path for every client: permissions, broadcasting and reconnection are exercised by
  the operator UI all the time, not only by phones.
- The operator UI can later run on a second computer ("controller mode") with no new API.
- Localhost WebSocket adds a little latency compared to IPC (measured ~1.5 ms median for an
  action round trip in a debug build).
- If the operator window crashes, the server and output windows keep running.
