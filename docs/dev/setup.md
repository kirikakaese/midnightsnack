# Development setup

## Prerequisites

- Rust stable (`rustup`), with `clippy` and `rustfmt`
- Node.js 20 or newer and pnpm 10 (`corepack enable` works)
- Tauri v2 system dependencies — see <https://v2.tauri.app/start/prerequisites/>.
  On Debian/Ubuntu:

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev \
    libayatana-appindicator3-dev libasound2-dev
  ```

## Common tasks

| Task                               | Command                                              |
| ---------------------------------- | ---------------------------------------------------- |
| Install JS dependencies            | `pnpm install`                                       |
| Run the host with hot reload       | `pnpm dev`                                           |
| Run the web remote dev server      | `pnpm --filter @midnightsnack/remote dev`            |
| Rust tests                         | `cargo test --workspace`                             |
| Frontend tests                     | `pnpm test`                                          |
| Lint and format check              | `pnpm lint`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets` |
| Type-check Svelte/TS               | `pnpm check`                                         |
| Regenerate protocol TS types       | `pnpm gen:types`                                     |
| Debug build without installers     | `pnpm --filter @midnightsnack/host tauri build --debug --no-bundle` |
| Launch smoke test                  | `node scripts/smoke-test.mjs` (after the debug build) |

Plain `cargo build` of the host loads the frontend from the Vite dev server; use the Tauri CLI
(`pnpm dev` or `tauri build`) to get a binary with embedded assets.

## Releasing

See [ADR 0004](../adr/0004-versioning-and-releases.md). Bump the version in the root
`Cargo.toml`, commit `chore: release vX.Y.Z`, tag `vX.Y.Z`, push the tag, then review and publish
the draft release.
