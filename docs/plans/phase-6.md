# Phase 6 — OpenSlides native integration

Goal: connect to an [OpenSlides](https://openslides.com) 4 instance, pull the meeting's agenda,
motions, topics, lists of speakers and projectors, and show them natively with midnightsnack
themes — as regular cues or by following an OpenSlides projector live. The projector URL cue
from phase 3 stays for everything else.

## Research (OpenSlides 4)

Read from the sources of `openslides-auth-service`, `openslides-autoupdate-service`,
`openslides-meta` (data model) and `openslides-client`:

- **Login:** `POST /system/auth/login` with `{"username","password"}`. The access token comes
  back in the `authentication` response header (`bearer <JWT>`, short-lived, payload with `exp`
  and `userId`), plus an HTTP-only `refreshId` cookie. `POST /system/auth/who-am-i/` with the
  cookie returns a fresh token. Meetings with public access can be read without logging in.
- **Data:** `POST /system/autoupdate` with a JSON list of key requests (`collection`, `ids`,
  `fields`; relations followed with `{"type": "relation" | "relation-list" | "generic-relation",
  "collection", "fields"}`), header `authentication: <token>`. The response is a stream of
  newline-separated JSON objects mapping `collection/id/field` to values: first everything, then
  changes (`null` = removed). `?single=1` answers once.
- **Model:** `meeting` (name, `reference_projector_id`, `agenda_item_ids`, `motion_ids`,
  `topic_ids`, `list_of_speakers_ids`, `projector_ids`), `agenda_item` (item_number, type
  common/internal/hidden, level, weight, closed, `content_object_id`), `motion` (number, title,
  `text`/`reason` as HTML, `state_id`, `submitter_ids` → `meeting_user` → `user`), `topic`
  (title, text), `list_of_speakers` (closed, `content_object_id`, `speaker_ids`), `speaker`
  (begin/end time, weight, speech_state, point_of_order, `meeting_user_id`), `projector` (name,
  `current_projection_ids`) and `projection` (`content_object_id`, `type` such as
  `agenda_item_list` or `current_los`, `stable` for overlays like the clock).
- The official projector is rendered by a separate projector service
  (`/system/projector/get/{id}`), which is what the phase 3 URL cue shows.

## Tasks

### Adapter (`crates/integrations/openslides`)
- [x] Session: login (or anonymous), token refresh before it expires, logout-free reconnects.
- [x] Meetings of the user (`user/{id}/meeting_ids`), meeting names.
- [x] One autoupdate subscription for the selected meeting; a key-value store applying the
      stream; typed views computed from it (agenda, motions, topics, lists of speakers,
      projectors and what they show).
- [x] Motion and topic texts converted from OpenSlides' HTML into plain blocks (paragraphs,
      headings, list items) — no HTML from the server reaches any page.
- [x] A mock OpenSlides server implementing the same endpoints (login, who-am-i, autoupdate with
      relation following and live updates) for tests and the development server.

### Cues and rendering
- [x] New cue kind **OpenSlides** with slides: agenda, motion, topic, list of speakers (a given
      list or the current one), and **follow projector** (sync mode: shows natively whatever the
      chosen OpenSlides projector — default the reference projector — currently projects).
- [x] Agenda and long motion texts are split into pages (slides), recomputed when the data
      changes; navigation works like any other cue.
- [x] Rendering in outputs, monitors, stage display and remotes with the show's text theme:
      agenda with numbers and levels, motion with number, title, submitters, state and text,
      list of speakers with current speaker (elapsed time), next and last speakers.
      Unsupported projections (elections, polls, files) show a titled placeholder.

### Host UI
- [x] Connect tab → OpenSlides: URL, username, password (or public access), status, meeting
      picker.
- [x] Cue list "Add…" → OpenSlides: agenda, current list of speakers, follow projector, and a
      picker for motions, topics and lists of speakers.

### Docs and tests
- [x] Unit tests for the store, views, HTML conversion and pagination; integration tests of the
      session and subscription against the mock (login, refresh, live updates, errors); server
      tests for the service; E2E: an OpenSlides cue on a phone, updated live.
- [x] User guide, protocol and security notes, ADR.

## Acceptance criteria

1. With URL and credentials of an OpenSlides 4 meeting, the operator picks the meeting and adds
   an agenda cue, a motion cue and a current list of speakers cue; they show the meeting's data
   in the show's theme on the beamer, the monitors and phones.
2. Adding a speaker or starting the next speaker in OpenSlides updates the list of speakers on
   the beamer within about a second.
3. A follow-projector cue shows the motion, topic, list of speakers or agenda that the OpenSlides
   projector shows, and changes when the OpenSlides operator projects something else.
4. Wrong credentials, an unreachable server or an expired session are shown clearly and retried;
   the output keeps showing the last data.

## Results

- Rust: adapter 8 unit tests (HTML conversion, store, URLs, tokens, cookies, stream errors) and
  5 against the mock server (meeting view, pagination, live updates with login, token renewal
  through the refresh cookie, wrong password / unknown meeting / public access / unreachable);
  core 3 (cues, page counts without marking the show edited, permissions, old files); server 2
  (data to clients but status only to admins, pages following OpenSlides, credentials kept on
  the host and private, invalid settings). Frontend: 4 resolver tests. Playwright 2 new (motion
  and list of speakers on a phone, updated live; a follow cue switching with the OpenSlides
  projector), 15 in total.
- Verified in the real host on Linux (WebKitGTK, X11) against the mock: the Connect tab shows
  the connection, the Add menu's OpenSlides picker lists the meeting, an agenda cue renders in the
  preview and slide strip, and a follow cue showed motion A1 and switched to its list of speakers
  when the projection changed in "OpenSlides".
- Writing the renewal test found that renewing the stream logged in again with the password; the
  session is now kept and the refresh cookie used.

## Known limitations and deviations

- **Not tested against a real OpenSlides server:** the development container's network does not
  reach public instances, and running OpenSlides needs Docker. The adapter follows the OpenSlides
  4 sources (auth, autoupdate, models), and the mock implements the same endpoints, relation
  following and update stream; real-server testing is a 1.0 task.
- Read-only: midnightsnack does not start speakers or project items in OpenSlides.
- Elections, polls, files, messages and countdowns are not drawn natively (titled placeholder).
- Speaking times use the host clock; with large clock differences they are hidden.
- One meeting at a time.
