# midnightsnack

**A free, open-source, cross-platform presentation and beamer host with remote control.**

midnightsnack runs on a laptop (macOS, Windows, Linux) connected to one or more projectors or
screens. The laptop is the **host**: it holds the show (a cue list), renders fullscreen output
windows on the chosen displays and gives the operator a presenter view. **Remotes** control the
host — a phone or tablet browser (no install), a second computer, or hardware such as clickers,
MIDI controllers, OSC and Stream Deck via Bitfocus Companion.

- **Offline-first** — works on a local network with zero internet.
- **Never a broken frame** — outputs hold the last good frame or fall back to logo/black; no
  dialogs, cursors or spinners on the audience screen.
- **Fast** — live actions react in under 100 ms on LAN; upcoming slides are pre-rendered.

> **Status:** early development (phase 4). PDF, PowerPoint/Keynote, image, video, audio,
> text/lyrics, timer, web page and screen capture cues, transitions, overlays, several outputs
> and stage displays, phone remotes with laser pointer, drawing and file upload, a second
> computer as controller, MIDI, OSC, an HTTP API and Bitfocus Companion work today. See the
> [roadmap](#roadmap) and the [user guide](docs/user/README.md).

## Quick start (development)

Prerequisites: [Rust](https://rustup.rs) (stable), Node.js 20+, [pnpm](https://pnpm.io) 10, and
the [Tauri v2 system dependencies](https://v2.tauri.app/start/prerequisites/) for your OS.

```sh
pnpm install
node scripts/fetch-pdfium.mjs   # downloads the PDF engine for your platform
pnpm build:remote               # builds the phone remote served by the host
pnpm dev                        # starts the host app with hot reload
```

To work on the phone remote without the desktop app, run the headless development server with a
demo show and the remote's dev server:

```sh
cargo run -p midnightsnack-server --bin midnightsnack-devserver -- --demo
pnpm --filter @midnightsnack/remote dev
```

Run all checks the way CI does:

```sh
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm lint && pnpm check && pnpm test && pnpm i18n:check
pnpm --filter @midnightsnack/remote e2e   # after building the remote and the devserver
```

More in [docs/dev/setup.md](docs/dev/setup.md).

## Repository layout

| Path                 | Contents                                                        |
| -------------------- | --------------------------------------------------------------- |
| `apps/host`          | Tauri v2 desktop host (Rust in `src-tauri`, Svelte frontend)    |
| `apps/remote`        | Web remote (Svelte), embedded into the host binary              |
| `crates/core`        | Show model, cue engine, action dispatcher — no UI, unit-tested  |
| `crates/protocol`    | Wire protocol types; generates TypeScript via ts-rs             |
| `crates/render`      | PDF (PDFium) and image rendering with a disk cache              |
| `crates/server`      | Embedded HTTP/WebSocket server: pairing, devices, media, autosave |
| `crates/capture`     | Screen and window capture                                       |
| `crates/control`     | MIDI and OSC control surfaces                                   |
| `integrations/companion` | Bitfocus Companion (Stream Deck) module                      |
| `packages/ui`        | Shared Svelte component library, design tokens and i18n         |
| `packages/protocol`  | Generated TypeScript protocol types                             |
| `docs/`              | User guides, developer docs, ADRs and phase plans               |

Further crates (e.g. `relay`) are added in the phase that needs them; see
[docs/dev/architecture.md](docs/dev/architecture.md).

## Roadmap

| Phase | Scope                                                                       | Version |
| ----- | --------------------------------------------------------------------------- | ------- |
| 0     | Foundations: monorepo, CI, protocol + TS generation, i18n, design system    | 0.0.x   |
| 1     | MVP: PDF/image cues, output, presenter view, phone remote with pairing      | 0.1.0   |
| 2     | Media & live content: video, text/lyrics, timers, transitions, overlays     | 0.2.0   |
| 3     | Outputs & sources: multi-output, capture, web, office formats, OpenSlides URL | 0.3.0 |
| 4     | Control surfaces: pointer/drawing, upload inbox, MIDI, OSC, Companion       | 0.4.0   |
| 5     | Connectivity: hotspot guidance, E2E-encrypted relay                         | 0.5.0   |
| 6     | OpenSlides native integration                                               | 0.6.0   |
| 7     | 1.0 polish                                                                  | 1.0.0   |

Phase plans live in [docs/plans](docs/plans), decisions in [docs/adr](docs/adr).

## Contributing

Contributions are welcome — please read [CONTRIBUTING.md](CONTRIBUTING.md) and the
[Code of Conduct](CODE_OF_CONDUCT.md). Translations are especially easy: all strings live in
`packages/ui/src/locales/`.

## License

midnightsnack is free software, licensed under the
[GNU General Public License v3.0 or later](LICENSE). Third-party components are listed in
[THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
