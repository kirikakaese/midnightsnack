# Protocol

The host's embedded server speaks HTTP and WebSocket on one port (default **4747**). The types
live in [`crates/protocol`](../../crates/protocol/src) and are generated into TypeScript in
`packages/protocol`. This document describes the wire format of protocol version **1**.

All JSON uses `snake_case`. Enums with data are internally tagged: messages by `type`, actions
by `action`, pairing status by `status`. Timestamps are Unix epoch milliseconds of the host
clock.

## HTTP

| Method & path                                   | Auth          | Purpose                          |
| ----------------------------------------------- | ------------- | -------------------------------- |
| `GET /api/v1/info`                              | none          | `HostInfo` (name, versions)      |
| `POST /api/v1/pair`                             | join token+PIN| Start pairing → `{request_id}`   |
| `GET /api/v1/pair/{request_id}`                 | request id    | `PairStatus`                     |
| `GET /api/v1/ws`                                | token (hello) | Realtime WebSocket               |
| `GET /api/v1/media/slide/{cue_id}/{slide}?k=&w=&h=` | media key | Rendered slide image             |
| `GET /api/v1/media/file/{cue_id}?k=`            | media key     | Original video/audio file (HTTP range requests) |
| `GET /api/v1/media/asset/{asset_id}?k=`         | media key     | Image asset (logo, backgrounds, logo bug) |
| `GET /api/v1/media/capture/{cue_id}?k=&fps=`    | media key     | Live capture as MJPEG (`multipart/x-mixed-replace`) |
| `GET /*`                                        | none          | Web remote (single-page app)     |

Errors are returned as `{"code": "<error_code>"}` with a matching status: `401` for bad tokens or
PINs, `403` forbidden, `404` not found, `429` pairing locked, `503` PDF engine missing.

### Pairing

1. The host shows a QR code with `http://<lan-ip>:<port>/join#t=<join token>`. The token is in
   the URL fragment so it is never sent in requests or logged.
2. The remote posts `{"join_token", "pin", "device_name"}` to `/api/v1/pair`.
   - The join token is **one-time**: it rotates after every successful submission.
   - 5 wrong attempts from one address lock that address for 60 s; 20 failures across all
     addresses within a minute lock pairing globally for 60 s.
3. The remote polls `/api/v1/pair/{request_id}` once per second:
   `{"status":"pending"}` → `{"status":"approved","token","device_id","role"}` (returned
   exactly once) or `{"status":"denied"}`. Requests expire after 5 minutes.
4. The operator approves with a role (or the host auto-approves with a configured role).
5. The remote stores the token and uses it for every WebSocket connection.

## WebSocket

The first client message must be `hello` within 10 seconds:

```json
{ "type": "hello", "protocol_version": 1, "token": "<session token>" }
```

On failure the server sends `{"type":"error","code":"unauthorized" | "protocol_mismatch" |
"malformed_message"}` and closes. On success it sends, in order: `welcome`, `show`, `live`,
(admins: `devices`, `pairing`), `render_progress`.

### Client → host

| `type`     | Fields                         | Notes                                           |
| ---------- | ------------------------------ | ----------------------------------------------- |
| `hello`    | `protocol_version`, `token`    | First message only                              |
| `action`   | `request_id`, `action`         | Answered by `action_result` with the same id    |
| `viewport` | `width`, `height`              | Output windows only (ignored from remotes)      |
| `ping`     | `nonce`                        | Answered by `pong`                              |
| `list_capture_targets` |                    | Admins; answered by `capture_targets`           |

### Host → client

| `type`            | Fields                         | When                                    |
| ----------------- | ------------------------------ | --------------------------------------- |
| `welcome`         | `host`, `session`              | After a valid `hello`                   |
| `show`            | `show` (`ShowSnapshot`)        | Cue list or show metadata changed       |
| `live`            | `live` (`LiveState`)           | Position, masters or timers changed     |
| `devices`         | `devices`, `pending`           | Admins; device list or pairing requests |
| `pairing`         | `pairing` (PIN, join URLs)     | Admins; PIN/join token changed          |
| `session`         | `session`                      | This device's role changed              |
| `render_progress` | `queued`                       | Background render queue length          |
| `capture_targets` | `targets`                      | Reply to `list_capture_targets` (`error` `capture_permission` if the OS denies capture) |
| `action_result`   | `request_id`, `error \| null`  | Reply to `action`                       |
| `pong`            | `nonce`                        | Reply to `ping`                         |
| `error`           | `code`                         | Protocol-level error                    |

The server also sends WebSocket ping frames every 5 s to measure latency (shown in the device
list).

### State

`LiveState.output` is what the audience sees; it differs from `program` only while frozen.
`LiveState.media` is the playback timeline of the media cue on the output (it follows `output`,
so a frozen video keeps playing): `position` is a stopwatch of the file position in ms. Looping
and trimming are applied by clients (`mediaPosition()` in `packages/ui`). `countdown` is the
global countdown, `overlays_visible` the ids of shown overlays, `stage_message` the operator's
message to stage displays, and `auto_advance_at_ms` when the program will advance on its own.
Master states are drawn on top in this order: content → logo → blackout.

`ShowSnapshot.outputs` lists the show's outputs (`OutputDef`: feed, scaling, margin, overlays);
`LiveState.outputs` holds the position each output shows. A cue with `targets` only moves the
listed outputs; the others keep their position. `output` is the main output's position.
`test_pattern` replaces the content on program outputs. `capture_lost` lists capture cues whose
source is gone. `web_nav` counts next/prev steps forwarded to the live web page (`seq` increases
with every step).
Stopwatch elapsed time is `accumulated_ms + (now - running_since_ms)`; clients compute
`now` as their clock plus `host_time_ms - Date.now()` from the last `live` message.

### Actions and roles

| Action                                                   | Minimum role | Local only |
| -------------------------------------------------------- | ------------ | ---------- |
| `go`, `next`, `prev` (presenters: within the live cue, or forwarded to a live web page) | presenter | |
| `next_cue`, `prev_cue`, `go_to`                          | operator     |            |
| `set_/toggle_blackout`, `set_/toggle_freeze`, `set_/toggle_logo`, `panic` | operator |  |
| `timer_start`, `timer_pause`, `timer_reset`              | operator     |            |
| `media_play`, `media_pause`, `media_seek`, `media_restart` | operator   |            |
| `countdown_set`, `countdown_start`, `countdown_pause`, `countdown_reset`, `set_stage_message` | operator | |
| `set_overlay_visible`, `toggle_overlay`                  | operator     |            |
| `media_loaded`, `media_ended` (reports from output windows) | operator  | yes        |
| `set_test_pattern`                                       | operator     |            |
| `put_output`, `remove_output`, `set_cue_targets`, `add_web`, `set_web_options`, `add_capture`, `set_capture` | admin | |
| `put_overlay`, `remove_overlay`, `add_text`, `set_cue_text`, `add_timer`, `set_cue_timer`, `set_cue_theme`, `set_default_theme`, `set_cue_transition`, `set_default_transition`, `set_cue_auto_advance`, `set_media_options` | admin | |
| `set_logo_image`, `set_background_image`, `set_overlay_image` with a `path` | admin | yes |
| `rename_show`, `rename_cue`, `set_cue_notes`, `set_cue_color`, `move_cue`, `remove_cue`, `add_blank` | admin | |
| `add_files`, `open_show`, `save_show` with a `path`      | admin        | yes        |
| `new_show`, `save_show` without `path`                   | admin        |            |
| `approve_pairing`, `deny_pairing`, `set_device_role`, `revoke_device`, `disconnect_all`, `set_auto_approve` | admin | |

"Local only" actions carry paths on the host's file system and are refused from remote devices
even with the admin role. Stage viewers cannot send any action.

## Media

Slide images require the per-connection `media_key` from `welcome.session` (image elements
cannot send headers). Without `w`/`h` a 640×360 thumbnail is returned; output windows request
their native pixel size. Responses carry an `ETag`; send `If-None-Match` to get `304`.
