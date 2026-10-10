# 0016. In-app updates

- **Status:** accepted (supersedes the update part of 0014)
- **Date:** 2026-10-10

## Context

People should get fixes without hunting for installers, like in SMP (which uses Sparkle on
macOS). DECK is a Tauri app on three platforms and runs live events: an update must never
interrupt a show or install something nobody signed.

## Decision

- `tauri-plugin-updater` with a minisign release key; the public key is compiled in by the
  release workflow (`DECK_UPDATER_PUBKEY`), so local builds never update. Signatures must name
  the version (`requireSignedVersion`), which rules out downgrades through a crafted feed.
- Two feeds on the latest stable GitHub release, as SMP's appcast: `latest.json` (stable) and
  `beta.json` (newest release including prereleases), written when a release is published.
- Settings in Control → Updates, as in SMP: check automatically (default on), how often (daily,
  weekly, monthly), download automatically and install on quit (default off), include betas
  and release candidates (default off), last check, Check now.
- Never during a show: "Install and restart" is refused while an output is open; automatic
  installation happens when the app quits.
- A Linux `.deb` is left to the package manager; macOS, Windows (NSIS) and the AppImage update
  themselves.

## Consequences

- The repository needs the key as secrets (see `docs/dev/releasing.md`); without it stable
  releases fail to build, so no release ships that can never update.
- A prerelease build always reads the beta feed, so release candidate testers get the next
  candidate and then the final release without changing a setting.
- Losing the private key strands installed apps on their version.
