# Third-party licenses

midnightsnack is licensed under GPL-3.0-or-later. All dependencies must be compatible with it.

## Policy

- **Rust:** enforced by [`cargo deny check licenses`](deny.toml) in CI. Permissive licenses
  (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode, MPL-2.0, …) are allowed.
- **JavaScript:** dependencies are bundled into the host and the web remote. Only permissive or
  GPL-compatible packages are used; check new ones before adding them.
- **Bundled binaries:** listed below with their license.

## Notable components

| Component                                  | License               | Use                       |
| ------------------------------------------ | --------------------- | ------------------------- |
| [Tauri](https://tauri.app)                 | MIT OR Apache-2.0     | Desktop shell             |
| [Svelte](https://svelte.dev)               | MIT                   | UI framework              |
| [intl-messageformat](https://formatjs.github.io) | BSD-3-Clause    | ICU message formatting    |
| [ts-rs](https://github.com/Aleph-Alpha/ts-rs) | MIT                | TS type generation (build-time) |
| [PDFium](https://pdfium.googlesource.com/pdfium/) via [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) | BSD-3-Clause / Apache-2.0 | PDF rendering (bundled binary, license shipped as `PDFIUM-LICENSE`) |
| [pdfium-render](https://github.com/ajrcarey/pdfium-render) | MIT OR Apache-2.0 | PDFium bindings |
| [image](https://github.com/image-rs/image) | MIT OR Apache-2.0 | Image decoding and scaling |
| [axum](https://github.com/tokio-rs/axum) / [tokio](https://tokio.rs) | MIT | Embedded server |
| [mdns-sd](https://github.com/keepsimple1/mdns-sd) | MIT OR Apache-2.0 | mDNS/DNS-SD advertisement |
| [qrcode](https://github.com/kennytm/qrcode-rust) | MIT OR Apache-2.0 | QR codes |
| [keepawake](https://github.com/segevfiner/keepawake-rs) | MIT | Prevent screen sleep |
| [zip](https://github.com/zip-rs/zip2) | MIT | `.msnack` show bundles |
| [rust-embed](https://github.com/pyrossh/rust-embed) | MIT | Embeds the web remote |
| [xcap](https://github.com/nashaofu/xcap) | Apache-2.0 | Screen and window capture (macOS, Windows) |
| [x11rb](https://github.com/psychon/x11rb) | MIT OR Apache-2.0 | Screen and window capture (Linux/X11) |
| [jpeg-encoder](https://github.com/vstroebel/jpeg-encoder) | (MIT OR Apache-2.0) AND IJG | JPEG frames for capture streams |
| [quick-xml](https://github.com/tafia/quick-xml) | MIT | Speaker notes from PPTX/ODP |
| [midir](https://github.com/Boddlnagg/midir) | MIT | MIDI input |
| [rosc](https://github.com/klingtnet/rosc) | MIT OR Apache-2.0 | OSC encoding and decoding |
| [reqwest](https://github.com/seanmonstar/reqwest) | MIT OR Apache-2.0 | Pairing with another host (controller mode), the OpenSlides adapter |
| [snow](https://github.com/mcginty/snow) | Apache-2.0 OR MIT | Noise protocol (relay end-to-end encryption, host side) |
| [@noble/curves, ciphers, hashes](https://paulmillr.com/noble/) | MIT | X25519, ChaCha20-Poly1305, BLAKE2s (relay encryption, web remote) |
| [rustls](https://github.com/rustls/rustls) / [tokio-rustls](https://github.com/rustls/tokio-rustls) | Apache-2.0 OR ISC OR MIT | TLS for HTTPS and `wss://` relays |
| [ring](https://github.com/briansmith/ring) | Apache-2.0 AND ISC | Cryptography provider for rustls and rcgen |
| [rustls-native-certs](https://github.com/rustls/rustls-native-certs) | Apache-2.0 OR ISC OR MIT | The operating system's trusted certificates |
| [rcgen](https://github.com/rustls/rcgen) | MIT OR Apache-2.0 | Self-signed HTTPS certificate |
| [hyper-util](https://github.com/hyperium/hyper-util) | MIT | HTTPS connections |
| [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite) | MIT | WebSocket client to the relay |
| [@companion-module/base](https://github.com/bitfocus/companion-module-base) | MIT | Companion module framework |
| [ws](https://github.com/websockets/ws) | MIT | WebSocket client of the Companion module |
| [LibreOffice](https://www.libreoffice.org) | MPL-2.0 | Optional, not bundled: converts presentations to PDF when installed |

## Exceptions

- **jpeg-encoder** carries the Independent JPEG Group license in addition to MIT/Apache-2.0. The
  FSF lists the IJG license as free and GPL-compatible; `deny.toml` allows it for this crate only.
