# 0013. OpenSlides adapter

- **Status:** accepted
- **Date:** 2026-10-09

## Context

Assemblies run on OpenSlides; their projector content (agenda, motions, lists of speakers)
should appear on midnightsnack outputs in the event's look, not as an embedded web page. The
OpenSlides 4 API is a set of services behind one origin: auth (JWT plus refresh cookie),
autoupdate (a streaming key-value view of the data the user may see), action (writes) and a
projector service that renders HTML.

## Decision

- **Adapter crate** `crates/integrations/openslides` owns everything OpenSlides-specific:
  session handling, the autoupdate key requests, the key-value store, the conversion into
  midnightsnack's own types (`OsMeetingData` in the protocol crate) and an HTML-to-blocks
  converter. The server crate only runs it and forwards the result. A mock server in the same
  crate speaks the same endpoints for tests.
- **Read-only.** The adapter never calls the action service. Operators keep running the meeting
  in OpenSlides; midnightsnack displays it. (Controlling OpenSlides can be added later behind
  the same adapter.)
- **One subscription per meeting.** The host requests the agenda, motions, topics, lists of
  speakers and projectors of the selected meeting in one autoupdate request and keeps the
  resulting store; every change is applied and the typed meeting data recomputed and broadcast
  to clients (throttled). Tokens are refreshed through `who-am-i` before they expire; the stream
  is restarted with the new token.
- **Native rendering on the clients.** Like text and timer cues, OpenSlides cues are drawn by
  the clients from shared data with the show's theme. The data is plain text and structure:
  motion and topic HTML is reduced to paragraphs, headings and list items in Rust, so no markup
  from the OpenSlides server is ever inserted into a page.
- **Pages are part of the show.** Agenda and motion cues are split into pages by a fixed budget
  (agenda entries or characters per page). The host recomputes page counts when data changes and
  updates the cues without marking the show as edited, so navigation (next/prev, remotes,
  presenter rules) works unchanged.
- **Sync mode is a cue.** A "follow projector" cue shows whatever the chosen OpenSlides projector
  currently projects (its first non-stable projection), resolved to a native slide; anything
  midnightsnack cannot draw natively becomes a titled placeholder. The projector URL cue remains
  for exact replicas.
- **Credentials** are stored in the host's data directory, readable only by the user, and only
  admins can change them; the password is never sent to clients.

## Consequences

- Everything the configured OpenSlides account can see about the meeting's agenda, motions and
  speakers is sent to all paired devices of the show (they need it to draw the slides). Use an
  account with the visibility the audience may have.
- The autoupdate request format and the data model follow the OpenSlides sources read in phase
  6; changes in OpenSlides need adapter updates (the mock and its tests pin our assumptions).
- Elections (assignments), polls, files and messages are not rendered natively yet.
