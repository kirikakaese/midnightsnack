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
| `POST /api/v1/action`                           | bearer token  | Run an `Action` (JSON body) → `{}` |
| `GET /api/v1/state`                             | bearer token  | `StateSummary` (live cue, masters, timers) |
| `POST /api/v1/upload?name=`                     | bearer token  | Send a file (raw body) → `UploadResponse`; presenter and up |
| `GET /api/v1/media/slide/{cue_id}/{slide}?k=&w=&h=` | media key | Rendered slide image             |
| `GET /api/v1/media/file/{cue_id}?k=`            | media key     | Original video/audio file (HTTP range requests) |
| `GET /api/v1/media/asset/{asset_id}?k=`         | media key     | Image asset (logo, backgrounds, logo bug) |
| `GET /api/v1/media/capture/{cue_id}?k=&fps=`    | media key     | Live capture as MJPEG (`multipart/x-mixed-replace`) |
| `GET /*`                                        | none          | Web remote (single-page app)     |

Errors are returned as `{"code": "<error_code>"}` with a matching status: `401` for bad tokens or
PINs, `403` forbidden, `404` not found, `413` file too large, `415` unsupported file, `429`
pairing locked, `503` PDF engine missing. Bearer tokens are device tokens or API keys
(`Authorization: Bearer <token>`); API keys are refused from other computers while the host
restricts them to itself (`ControlSettings.api_local_only`, on by default).

### Pairing

1. The host shows a QR code with `http://<lan-ip>:<port>/join#t=<join token>`. The token is in
   the URL fragment so it is never sent in requests or logged.
2. The remote posts `{"join_token", "pin", "device_name"}` to `/api/v1/pair`.
   - The join token is **one-time**: it rotates after every successful submission.
   - An empty join token (a second computer typing the PIN) is accepted only while
     auto-approve is off, so the operator always approves such requests.
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
(admins: `devices`, `inbox`, `pairing`, `connectivity`), (paired remotes: `routes`),
`render_progress`.

### Client → host

| `type`     | Fields                         | Notes                                           |
| ---------- | ------------------------------ | ----------------------------------------------- |
| `hello`    | `protocol_version`, `token`    | First message only                              |
| `action`   | `request_id`, `action`         | Answered by `action_result` with the same id    |
| `viewport` | `width`, `height`              | Output windows only (ignored from remotes)      |
| `ping`     | `nonce`                        | Answered by `pong`                              |
| `list_capture_targets` |                    | Admins; answered by `capture_targets`           |
| `create_api_key` | `name`, `role`               | Admins; answered by `api_key` (token shown once) |
| `pointer`  | `pos` (`[x, y]` or `null`), `mode`, `color` | Presenters and up; relayed to other clients, ≤ 60/s per device |

### Host → client

| `type`            | Fields                         | When                                    |
| ----------------- | ------------------------------ | --------------------------------------- |
| `welcome`         | `host`, `session`              | After a valid `hello`                   |
| `show`            | `show` (`ShowSnapshot`)        | Cue list or show metadata changed       |
| `live`            | `live` (`LiveState`)           | Position, masters or timers changed     |
| `devices`         | `devices`, `pending`, `control` | Admins; device list, pairing requests, API/OSC settings |
| `pairing`         | `pairing` (PIN, join `links` with `kind` `lan`/`https`/`relay`) | Admins; PIN/join token or links changed |
| `connectivity`    | `connectivity` (`ConnectivityInfo`) | Admins; interfaces, HTTPS, relay status changed |
| `routes`          | `routes` (`lan` base URLs, `relay` `{url, key}`) | Paired remotes (not host windows or API keys); for falling back to the relay and back |
| `session`         | `session`                      | This device's role changed              |
| `render_progress` | `queued`                       | Background render queue length          |
| `capture_targets` | `targets`                      | Reply to `list_capture_targets` (`error` `capture_permission` if the OS denies capture) |
| `pointer`         | `device_id`, `pos`, `mode`, `color` | Another device's laser pointer / drawing in progress |
| `inbox`           | `items`, `auto_accept`         | Admins; uploads waiting for a decision  |
| `api_key`         | `device_id`, `name`, `token`   | Reply to `create_api_key`               |
| `action_result`   | `request_id`, `error \| null`  | Reply to `action`                       |
| `pong`            | `nonce`                        | Reply to `ping`                         |
| `error`           | `code`                         | Protocol-level error                    |

The server also sends WebSocket ping frames every 5 s to measure latency (shown in the device
list). Over the relay these are tunnel `PING` frames.

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
`test_pattern` replaces the content on program outputs. `drawing` holds the strokes drawn on the
slide the main output shows (coordinates are fractions of a 16:9 frame); it is dropped when that
slide changes. Laser pointers are not part of the state: they travel as `pointer` messages and
disappear after 3 s without updates. `capture_lost` lists capture cues whose
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
| `draw_stroke`, `clear_drawing`                           | presenter    |            |
| `accept_upload`, `reject_upload`, `set_auto_accept_uploads`, `set_api_local_only`, `configure_osc` | admin | |
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

## HTTPS

With HTTPS enabled the same API is served over TLS on its own port (default **4749**) with a
self-signed certificate (ECDSA P-256, all LAN addresses, `localhost` and `<name>.local` as
names, valid 820 days). Join links of kind `https` point there; `ConnectivityInfo.https`
carries the port and the certificate's SHA-256 fingerprint.

## Relay

The relay ([`crates/relay`](../../crates/relay/src/lib.rs)) forwards opaque binary messages;
protocol version `/relay/v1`.

| Path                           | Who     | Purpose                                              |
| ------------------------------ | ------- | ---------------------------------------------------- |
| `GET /relay/v1/host` (WS)      | host    | The host's link. `Authorization: Bearer <base64url secret, 32 bytes>`, `X-Relay-Token` if the relay requires one (`401` otherwise, `503` when full) |
| `GET /relay/v1/remote/{id}` (WS) | remote | One remote channel. Closed with `4404` if the host is offline, `4429` if it has too many remotes, `4000` when the host closed the channel |
| `GET /relay/v1/info`           | anyone  | `{name, version, protocol, access_token_required}`   |
| `GET /healthz`                 | anyone  | `ok`                                                 |
| `GET /r/{id}`, `/assets/*`     | phones  | The web remote                                       |

The host id is `base64url(SHA-256("midnightsnack relay host id" ‖ secret)[..16])`, computed by
the relay from the secret, so ids cannot be claimed without the secret. A second link with the
same secret replaces the first.

On the host link every binary message is `[op u8][channel u32 BE][payload]`: `1` OPEN (relay →
host, a remote connected), `2` DATA (a remote's message, either direction), `3` CLOSE (either
direction). Remote WebSockets carry the payloads directly.

### End-to-end encryption

Each channel is a [Noise](https://noiseprotocol.org/noise.html)
`Noise_NK_25519_ChaChaPoly_BLAKE2s` session: the remote is the initiator and knows the host's
static key from the join link, the host is the responder. The prologue is
`"midnightsnack relay 1\n" + host id`. Message 1 (`e, es`) and message 2 (`e, ee`) carry
empty payloads; every later message is one transport message (at most 65535 bytes). The relay
join link is `https://<relay>/r/<host id>#t=<join token>&k=<base64url static key>`; the fragment
never reaches the relay's server. Fixed test vectors shared by both implementations are in
`packages/ui/src/client/relay/noise-vectors.json`.

### Tunnel framing

Each decrypted message is one frame `[kind u8][stream u32 BE][flags u8][payload]`, flag `1` =
FIN (last fragment), payload at most 65513 bytes.

| Kind | Name            | Stream | Payload                                                  |
| ---- | --------------- | ------ | -------------------------------------------------------- |
| 1    | `WS_MESSAGE`    | 0      | Fragment of a WebSocket text message (FIN on the last)   |
| 2    | `REQUEST_HEAD`  | ≥ 1    | `{"method","path","headers":[[k,v]…]}`; FIN = no body    |
| 3    | `REQUEST_BODY`  | ≥ 1    | Body bytes; FIN on the last                              |
| 4    | `RESPONSE_HEAD` | ≥ 1    | `{"status","headers":[[k,v]…]}`; FIN = no body           |
| 5    | `RESPONSE_BODY` | ≥ 1    | Body bytes; FIN on the last                              |
| 6    | `RESET`         | ≥ 1    | Abort the exchange                                       |
| 7/8  | `PING`/`PONG`   | 0      | Echoed payload                                           |
| 9    | `WS_CLOSE`      | 0      | The WebSocket session ended                              |

Stream 0 is the WebSocket session: the first `WS_MESSAGE` after the handshake (or after a
`WS_CLOSE`) starts a new session, which begins with `hello` as usual. Streams ≥ 1 are HTTP
exchanges chosen by the remote (at most 32 at once). The host answers them with its own router
for `/api/v1/*` (except the WebSocket, video/audio files and capture streams), with a synthetic
client address from `100::/64` that is never treated as local.

## OSC

UDP, default port 4748, enabled in the Control tab. The address space and feedback messages are
documented in [docs/user/control.md](../user/control.md#osc) and in `crates/control/src/osc.rs`.
Senders on this computer act as operators; with API keys not restricted to this computer, the
server also listens on all interfaces and other senders must `/midnightsnack/auth <token>` first
(5 failed attempts per address and minute, then ignored).
