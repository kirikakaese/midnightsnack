# Phase 7 — 1.0 release polish

Goal: make midnightsnack ready for its first stable release: a security review of everything
that faces the network, a performance and robustness pass with measurements, readiness for
translations, polished installers and complete documentation — then `v1.0.0`.

## Tasks

### Security review
- [x] Review pairing, sessions and roles; the HTTP API, uploads and media; the relay, the
      Noise tunnel and the relay server; HTTPS; the OpenSlides adapter; host windows, web page
      cues and the Tauri capabilities. Record findings, fixes and accepted risks in
      `docs/security-review.md`.
- [x] Fix what the review finds; add tests for each fix.

### Performance and robustness
- [x] A benchmark against the development server: action-to-update latency over the
      WebSocket (local and through the relay) with many connected remotes and pointer traffic;
      memory over a soak run. Record results.
- [x] Idle CPU of the host app with an output open; startup time.
- [x] Fix what the measurements or the review of failure paths turn up (reconnects, full
      disks, bad files, lost displays).

### Translations
- [x] Audit: no user-facing text outside the catalogs (operator, outputs, remote, native
      dialogs, server errors), numbers/dates/durations formatted with the locale, plurals via
      ICU messages; a check in CI for keys that are not used anymore.
- [x] Language setting in the host (operator, outputs and stage use it; remotes follow their
      browser language unless chosen), and a guide for translators.
- [x] A complete German translation as the first second language.

### Installers
- [x] `.msnack` file association: double-clicking a show opens it (in the running app if one is
      open).
- [x] Package metadata (publisher, homepage, copyright, minimum macOS version), Linux package
      dependencies for media playback, Windows installer languages.
- [x] Keep the updater hookable (documented, not enabled).

### Documentation
- [x] README with screenshots of the operator window, an output and the phone remote.
- [x] Troubleshooting / FAQ; release process for maintainers; every guide checked against the
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

## Results

- **Security review** ([security-review.md](../security-review.md)): 24 findings. Fixed: 1 high
  (an unauthenticated OSC datagram could crash the host), 6 medium and 12 low or
  informational; 5 low or informational ones accepted with reasons. Every fix rated medium or higher has a test.
- **Benchmark** (`scripts/bench.mjs`, release build of the development server on the
  development container, 25 paired phones, 10 of them moving a laser pointer at 30 Hz, the
  operator alternating next/previous): 14,187 actions in 10 minutes, time until every
  connection had the new state p50 1.09 ms, p95 1.82 ms, p99 2.82 ms, max 117.85 ms; server
  memory 19.5 MB at the start, 19.6 MB peak and at the end (no growth).
- **Host app** (release build, Linux, X11 under Xvfb without a GPU): from start to a working
  operator window 1.0 s (three runs: 1.01, 1.04, 1.03 s). Idle with an output open and the
  show running: the output window's web process 0.2 % of one core, the host process 1 %; the
  operator window 12–14 %, almost all of it WebKitGTK compositing in software (with
  `WEBKIT_DISABLE_COMPOSITING_MODE=1` the operator window drops to 2 % and the whole app to
  about 3 %). Machines with a GPU composite in hardware. Memory about 950 MB for all
  processes together under software rendering.
- **Failure paths:** reconnects (E2E: phones, relay fallback), lost displays (phase 3),
  unreadable or hostile show files (now refused with `show_file_invalid`), and write errors
  (reported as `io`, autosave errors logged) were reviewed; the review's resource limits
  (stalled clients, full queues, upload volume) were the fixes this turned up.
- **German:** all 533 texts translated; an E2E test checks that a German phone shows no
  English text from the catalog while pairing, in the control view and in the stage view.
- **Installers:** built the `.deb` and checked its metadata, dependencies, desktop entry
  (`Exec=… %F`, `MimeType`) and MIME definition (`*.msnack`); with them installed, `gio open`
  of a show launches midnightsnack with the file. Starting the app with a show opens it;
  launching it again with another show (with a session bus, as on every Linux desktop) hands
  the file to the running app within 0.07 s and exits, and the running app opens it. The
  operator window in German was checked on screen.

## Known limitations and deviations

- **Latency through the relay** was not benchmarked: the benchmark speaks the plain WebSocket
  protocol, and the relay adds the round trip to the relay server (it forwards without
  processing). The relay E2E tests cover correctness.
- **Signing:** installers are still unsigned (see the user guide).
- **Windows and macOS installers** were not built in this environment; they are built by the
  release workflow. The file association there comes from the same configuration.
- **MIDI hardware and a real OpenSlides server** remain untested here (no hardware, no Docker).
