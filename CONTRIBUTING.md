# Contributing to midnightsnack

Thanks for helping! This document covers the workflow; the architecture is described in
[docs/dev/architecture.md](docs/dev/architecture.md).

## Ground rules

- Be kind — see the [Code of Conduct](CODE_OF_CONDUCT.md).
- Every contribution is licensed under **GPL-3.0-or-later**. New source files start with an SPDX
  header: `// SPDX-License-Identifier: GPL-3.0-or-later` (or the comment syntax of the language).
- New dependencies must be GPL-3.0-compatible. `cargo deny check licenses` enforces this for
  Rust; for npm packages check the license by hand. Document anything unusual in
  [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
- Never commit secrets, signing keys or personal email addresses.
- Features not covered by the roadmap start as a proposal in [docs/ideas.md](docs/ideas.md) or
  an issue.

## Branches and commits

- `main` is always releasable. Work happens on short-lived branches named
  `feat/<topic>`, `fix/<topic>`, `chore/<topic>`, `docs/<topic>` or `release/<version>`.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org):
  `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`, `ci:`, `perf:`. The changelog is
  generated from them.
- Keep commits small and focused; open a pull request against `main`.

## Before you open a pull request

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm lint && pnpm check && pnpm test && pnpm i18n:check
```

If you changed anything in `crates/protocol`, run `pnpm gen:types` and commit the generated
TypeScript.

## User-facing text

Never hard-code user-facing strings. Add a key to `packages/ui/src/locales/en.json` and use
`t("your.key")`. Rust code reports errors as stable codes (see `ErrorCode` in
`crates/protocol`), which the frontend translates. Translations: see
[docs/dev/translating.md](docs/dev/translating.md).

## Architecture decisions

Significant decisions are recorded as ADRs in [docs/adr](docs/adr). Copy
`docs/adr/0000-template.md`, number it sequentially, and include it in your pull request.
