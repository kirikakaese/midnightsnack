# 0014. Language choice, opening shows from the system, and no auto-update in 1.0

- **Status:** accepted; the part on updates is superseded by [0016](0016-updates.md)
- **Date:** 2026-10-10

## Context

For 1.0 midnightsnack gets a second language, must open `.msnack` files from the file manager,
and needs a position on updates. The host runs several windows (operator, outputs, controller
windows) that must agree on one language; phones belong to other people and have their own
language. A show double-clicked while midnightsnack runs must not start a second host (two
servers, two sets of outputs). Hosts run live events: anything that restarts or changes the
app on its own is a risk.

## Decision

- **Language.** The host's language is a host setting (`language` in the host settings; empty
  follows the system), read by every window at start and changed for all of them with a
  `language-changed` event. Phones pick the first of their browser's languages that has a
  catalog (`de-AT` → `de`) unless one was chosen on the pairing page (kept in the phone's
  storage). Language names are shown in their own language. Numbers, times and sizes go
  through `Intl` with the current locale; catalogs only hold words.
- **Opening shows.** The installers associate `.msnack`. The host runs as a single instance
  (`tauri-plugin-single-instance`): a second launch forwards its arguments and exits. A show
  from the arguments, a second launch or a macOS open-document event is kept as pending and
  announced to the operator window, which takes it, asks about unsaved changes like **Open…**
  does, and opens it with the usual (local-only) action. Opened shows are checked like any
  untrusted file (see the [security model](../security.md)).
- **Updates.** No auto-updater in 1.0. Releases are built so one can be added
  (`docs/dev/releasing.md`): a single version, stable asset names, and the intended rule that
  an update is only offered on request and never while an output is open.

## Consequences

- One place decides the host's language; a phone and the host may differ, by design.
- Shows open in the running app; there is never a second server competing for the port.
- Users update by installing a new release; the user guide and release notes must say what
  changed.
