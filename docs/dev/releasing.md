# Releasing

Releases are built by the `Release` workflow (`.github/workflows/release.yml`) when a tag
`vX.Y.Z` is pushed. It builds the host for macOS (universal `.dmg`), Windows (`.msi` and an
English/German NSIS `.exe`) and Linux (`.deb`, `.AppImage`), the relay binaries and the relay
Docker image (`ghcr.io/<owner>/midnightsnack-relay`), and creates a GitHub release with the
changelog of that version.

## Steps

1. **Release branch.** `git switch -c release/vX.Y.Z main`.
2. **Version.** Set the version in:
   - `Cargo.toml` (`[workspace.package] version`; the host app, its installers and
     `APP_VERSION` in `crates/core` take it from there),
   - every `package.json` (`apps/host`, `apps/remote`, `packages/ui`, `packages/protocol`,
     `integrations/companion`).

   Then run `cargo check --workspace` and `pnpm install` so that `Cargo.lock` and
   `pnpm-lock.yaml` follow, and `pnpm gen:types`.
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
7. **Tag.** Create the tag `vX.Y.Z` on the merge commit (GitHub → Releases → *Draft a new
   release* → choose a tag, or `git tag vX.Y.Z <commit> && git push origin vX.Y.Z`). The
   workflow then builds and publishes. Versions below 1.0 and tags with a suffix
   (`v1.1.0-rc.1`) should be marked as pre-releases.
8. **Check the release.** Download each installer once and start it; check the relay image
   with `docker run --rm ghcr.io/<owner>/midnightsnack-relay --version`.

## Signing

Builds are not code-signed yet; the user guide explains how to open unsigned builds. To sign
later, add the certificates as repository secrets and pass them to `tauri-action` (macOS:
`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, plus
`APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` for notarization; Windows: a certificate in
`bundle.windows.certificateThumbprint` or a `signCommand`). See the Tauri distribution guides.

## Updates

There is no auto-updater: hosts run during live events, and an update must never start on its
own. The app is kept ready for one:

- The version is a single value (step 2), reported by the host in `/api/v1/info`
  (`app_version`).
- Releases have stable asset names, so an updater manifest (`latest.json`) can be generated
  from them.
- To add updates, enable `tauri-plugin-updater` with a signing key (`tauri signer generate`;
  the private key as the `TAURI_SIGNING_PRIVATE_KEY` secret, the public key in
  `plugins.updater.pubkey`), set `bundle.createUpdaterArtifacts`, let `tauri-action` upload the
  manifest, and offer the update only on request and never while an output is open.

## Pre-releases during development

Each phase of the roadmap ended with a pre-release (`v0.N.0`) tagged on the merge commit of its
pull request.
