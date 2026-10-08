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

## Exceptions

None so far.
