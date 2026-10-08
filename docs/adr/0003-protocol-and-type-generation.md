# 0003. Wire protocol and TypeScript type generation

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The host, its own UI windows and remote clients exchange messages over WebSocket and HTTP.
Hand-maintaining matching types in Rust and TypeScript drifts quickly.

## Decision

- `crates/protocol` is the single source of truth. Messages are JSON, internally tagged with a
  `type` field in `snake_case`.
- TypeScript bindings are generated with **ts-rs** into `packages/protocol/src/generated` and
  committed. `pnpm gen:types` regenerates them; CI fails if they are stale.
- Constants that must match on both sides (e.g. `PROTOCOL_VERSION`) are generated too.
- Breaking changes bump `PROTOCOL_VERSION`; the first message on every connection is `hello`
  with the client's version.

## Consequences

- Every protocol change is visible in review as both a Rust and a TS diff.
- Contributors must remember to regenerate; CI catches it.
