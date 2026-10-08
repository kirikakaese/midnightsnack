# 0001. Tech stack and monorepo layout

- **Status:** accepted
- **Date:** 2026-10-08

## Context

midnightsnack must run on macOS, Windows and Linux, render audience-facing output windows,
embed a network server for remotes, and share UI between the operator view, the output windows
and a browser-based remote. It must be maintainable as a public open-source project.

## Decision

- **Tauri v2** for the desktop shell: small binaries, native webviews, Rust backend.
- **Svelte 5 + TypeScript + Vite** for every frontend. One shared component library
  (`packages/ui`) is used by the host frontend and the web remote.
- **Rust workspace** at the repository root (`crates/*`, `apps/host/src-tauri`) and a **pnpm
  workspace** (`apps/*`, `packages/*`).
- Domain logic lives in `crates/core` with no UI, network or platform dependencies, so it can be
  unit-tested exhaustively. Every input source (UI, remotes, hardware) goes through its single
  action dispatcher.
- Crates are added in the phase that needs them (`render`, `server`, `capture`, `control`,
  `relay`, …) instead of committing empty placeholders.

## Consequences

- Webview differences (WebView2, WKWebView, WebKitGTK) must be accounted for, especially for
  media codecs.
- Contributors need both Rust and Node toolchains.
