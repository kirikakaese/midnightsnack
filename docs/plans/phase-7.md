# Phase 7 — 1.0 release polish

Goal: make midnightsnack ready for its first stable release: a security review of everything
that faces the network, a performance and robustness pass with measurements, readiness for
translations, polished installers and complete documentation — then `v1.0.0`.

## Tasks

### Security review
- [ ] Review pairing, sessions and roles; the HTTP API, uploads and media; the relay, the
      Noise tunnel and the relay server; HTTPS; the OpenSlides adapter; host windows, web page
      cues and the Tauri capabilities. Record findings, fixes and accepted risks in
      `docs/security-review.md`.
- [ ] Fix what the review finds; add tests for each fix.

### Performance and robustness
- [ ] A benchmark against the development server: action-to-update latency over the
      WebSocket (local and through the relay) with many connected remotes and pointer traffic;
      memory over a soak run. Record results.
- [ ] Idle CPU of the host app with an output open; startup time.
- [ ] Fix what the measurements or the review of failure paths turn up (reconnects, full
      disks, bad files, lost displays).

### Translations
- [ ] Audit: no user-facing text outside the catalogs (operator, outputs, remote, native
      dialogs, server errors), numbers/dates/durations formatted with the locale, plurals via
      ICU messages; a check in CI for keys that are not used anymore.
- [ ] Language setting in the host (operator, outputs and stage use it; remotes follow their
      browser language unless chosen), and a guide for translators.
- [ ] A complete German translation as the first second language.

### Installers
- [ ] `.msnack` file association: double-clicking a show opens it (in the running app if one is
      open).
- [ ] Package metadata (publisher, homepage, copyright, minimum macOS version), Linux package
      dependencies for media playback, Windows installer languages.
- [ ] Keep the updater hookable (documented, not enabled).

### Documentation
- [ ] README with screenshots of the operator window, an output and the phone remote.
- [ ] Troubleshooting / FAQ; release process for maintainers; every guide checked against the
      app.

### Release
- [ ] Version 1.0.0 (separate release PR after this phase is merged), tag `v1.0.0`.

## Acceptance criteria

1. `docs/security-review.md` lists every area reviewed with its findings; all findings rated
   medium or higher are fixed and tested.
2. On the development machine, the median action-to-update latency is below 20 ms locally with
   25 remotes connected and pointers moving; no memory growth over a 10-minute soak.
3. With the language set to German, no English text remains in the operator window, outputs,
   stage display or remote (except names and content).
4. Opening a `.msnack` file from the file manager opens it in midnightsnack.
5. A new user can install, build a show, put it on a projector and pair a phone using only the
   README and the user guide.
