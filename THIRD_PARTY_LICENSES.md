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

## Exceptions

None so far.
