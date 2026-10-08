# Phase 0 — Foundations

Goal: a monorepo that builds, tests and lints on macOS, Windows and Linux, with the conventions
every later phase depends on.

## Tasks

- [x] Rust workspace + pnpm workspace scaffold (`crates/core`, `crates/protocol`, `apps/host`,
      `apps/remote`, `packages/ui`, `packages/protocol`).
- [x] Tauri v2 host app with a Svelte 5 frontend that boots and calls into Rust.
- [x] GPL-3.0-or-later `LICENSE`, SPDX headers, `cargo-deny` license policy,
      `THIRD_PARTY_LICENSES.md`.
- [x] Protocol crate with JSON round-trip tests and ts-rs TypeScript generation; CI check that
      generated files are current.
- [x] i18n scaffold (ICU MessageFormat, typed keys, English fallback, catalog check script).
- [x] Dark show-control design system (tokens, `Button`, `Panel`, `StatusDot`).
- [x] CI: rustfmt, clippy, eslint, prettier, svelte-check, i18n, cargo-deny, tests and a launch
      smoke test on Ubuntu 22.04, macOS 14 and Windows 2022.
- [x] Release workflow: tag → installers (dmg universal, msi + nsis, deb + AppImage), changelog
      from Conventional Commits, checksums.
- [x] Docs: README, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, ADRs 0001–0004, architecture and
      setup guides, issue/PR templates.

## Acceptance criteria

1. `cargo test --workspace`, `pnpm test`, `pnpm lint`, `pnpm check` pass locally and in CI.
2. `pnpm --filter @midnightsnack/host tauri build --debug --no-bundle` succeeds on all three
   OSes and the smoke test (`scripts/smoke-test.mjs`) exits 0: the operator window loads and
   round-trips an IPC call.
3. Changing a protocol type without running `pnpm gen:types` fails CI.

## Demo checklist

- [ ] `pnpm dev` opens the dark operator window showing the app name and version.
- [ ] `pnpm --filter @midnightsnack/remote dev` serves the placeholder remote on port 5173.
- [ ] `pnpm gen:types` produces no diff on a clean checkout.
- [ ] Tag `v0.0.1` produces a draft pre-release with installers for all platforms.
