# Releasing

Releases are built by the `Release` workflow (`.github/workflows/release.yml`) when a tag
`vX.Y.Z` (or a prerelease tag `vX.Y.Z-rc.N` / `vX.Y.Z-beta.N`) is pushed. It builds the host
for macOS (universal `.dmg`), Windows (an English/German NSIS `-setup.exe`, plus an `.msi` for
stable releases) and Linux (`.deb`, `.AppImage`), the relay binaries and the relay Docker image
(`ghcr.io/<owner>/deck-relay`), and creates a **draft** GitHub release with the changelog of
that version. Nothing is public until the draft is published.

## Steps

1. **Release branch.** `git switch -c release/vX.Y.Z main`.
2. **Version.** Set the version (for a release candidate e.g. `1.0.0-rc.1`) in:
   - `Cargo.toml` (`[workspace.package] version`; the host app, its installers and
     `APP_VERSION` in `crates/core` take it from there),
   - every `package.json` (`apps/host`, `apps/remote`, `packages/ui`, `packages/protocol`,
     `integrations/companion`).

   Then run `cargo check --workspace` and `pnpm install` so that `Cargo.lock` and
   `pnpm-lock.yaml` follow, and `pnpm gen:types`. The workflow refuses a tag that does not
   match the version in `Cargo.toml`.
3. **Protocol version.** If the wire protocol changed incompatibly since the last release,
   `PROTOCOL_VERSION` in `crates/protocol` must have been raised (remotes of the old version
   are told to reload).
4. **Changelog.** `git cliff --unreleased --tag vX.Y.Z` shows what the release notes will say
   (they are generated from Conventional Commits; `cliff.toml`). Fix up anything misleading
   with follow-up commits, not by editing the release afterwards.
5. **Checks.** Everything CI runs, plus the E2E suite and a manual run of the host on each
   platform you can reach: open a show from the file manager, put an output on a second
   display, pair a phone, run through the show.
6. **Pull request.** Open `release/vX.Y.Z` against `main`, wait for green CI, merge.
7. **Tag.** Create the tag on the merge commit (GitHub → Releases → *Draft a new release* →
   choose a tag, or `git tag vX.Y.Z <commit> && git push origin vX.Y.Z`). The workflow builds
   everything into a draft release; tags with a suffix and versions below 1.0 become
   prereleases.
8. **Check the draft.** Download each installer once and start it; check the relay image with
   `docker run --rm ghcr.io/<owner>/deck-relay:<version> --version`.
9. **Publish** the draft. The `Update feeds` job then offers it to the in-app updater (below).

## Release candidates

Test a version before calling it final with release candidates: set the version to
`1.0.0-rc.1` (step 2), tag `v1.0.0-rc.1`, test the draft, publish it as a prerelease. Fix,
then `1.0.0-rc.2`, and so on; the final `1.0.0` follows the same steps without the suffix.

- Prereleases never become the "Latest" release on GitHub: the relay image's `latest` tag and
  the stable update feed stay on the last stable release.
- On Windows a prerelease only has the NSIS installer (MSI versions cannot carry `-rc.1`).
- People who tick **Include beta versions and release candidates** in the app (Control →
  Updates) are offered prereleases; everyone else only stable releases. A release candidate
  itself always looks for newer candidates (and then the final release).

## Updates

DECK updates itself like SMP does: the app checks a feed on GitHub, downloads the new version
and installs it only if it carries a valid signature from DECK's release key (minisign, via
`tauri-plugin-updater`, with the signed version bound into the signature). Hosts run live
events, so nothing restarts on its own: an update is installed when the operator asks for it
(while no output is open), or, with automatic installation, downloaded in the background and
installed when the app quits.

- **Feeds.** Both live on the latest stable release: `latest.json` (written by the build) is
  the stable feed; `beta.json` (written by the `Update feeds` job when a release is
  published) offers the newest release, prereleases included.
- **What updates.** macOS (`.app`), Windows (NSIS) and the Linux AppImage. A `.deb` is updated
  by the package manager; the app says so instead of offering updates.
- **Builds without the key** (local builds, forks) never check for updates.

### The release key (once)

1. On any computer with Node.js: `npx @tauri-apps/cli signer generate -w deck-updater.key`
   and choose a password. This writes the private key `deck-updater.key` and the public key
   `deck-updater.key.pub`.
2. In the repository settings (Secrets and variables → Actions):
   - secret `TAURI_SIGNING_PRIVATE_KEY`: the contents of `deck-updater.key`,
   - secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: the password,
   - variable `TAURI_UPDATER_PUBKEY`: the contents of `deck-updater.key.pub`.
3. Keep `deck-updater.key` and its password somewhere safe and offline. Losing it means
   installed apps can no longer be updated; anyone who has it can ship updates to everyone.

Without the key, prereleases are built with a warning (they do not update themselves) and
stable releases fail.

## Homebrew

After every stable release (not prereleases) the `Update feeds` job writes the new version into
the tap [kirikakaese/homebrew-tap](https://github.com/kirikakaese/homebrew-tap) as
`Casks/deck.rb` (from `scripts/deck.rb.template`), the same tap SMP uses. People then install
and upgrade with:

```sh
brew install --cask kirikakaese/tap/deck
```

The cask declares `auto_updates`, so `brew upgrade` leaves DECK to its own updater.

### The tap token (once)

The job needs a token that may change only the tap. The token SMP uses for its tap works as
well (same repository, same permission); to make a new one:

1. [Settings → Developer settings → Fine-grained tokens → Generate new token](https://github.com/settings/personal-access-tokens/new):
   resource owner `kirikakaese`, **Only select repositories** → `kirikakaese/homebrew-tap`,
   permission **Contents → Read and write**, nothing else.
2. In this repository: Settings → Secrets and variables → Actions → **New repository secret**,
   name `TAP_TOKEN`, paste the token.

Without the secret, releases still work; the job notes that it skipped the tap.

## Signing

macOS builds are signed ad hoc (`signingIdentity: "-"`), which Apple silicon requires for
downloaded apps, but not with a Developer ID or notarized; the user guide explains how to open
them. Windows builds are not signed. To sign
later, add the certificates as repository secrets and pass them to `tauri-action` (macOS:
`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, plus
`APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` for notarization; Windows: a certificate in
`bundle.windows.certificateThumbprint` or a `signCommand`). See the Tauri distribution guides.

## Pre-releases during development

Each phase of the roadmap ended with a pre-release (`v0.N.0`) tagged on the merge commit of its
pull request.
