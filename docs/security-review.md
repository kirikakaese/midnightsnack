# Security review for 1.0

A review of everything in midnightsnack that faces the network or handles untrusted input,
done before the 1.0 release. It complements the [security model](security.md), which
describes the controls; this document records what was checked, what was found and what was
done about it.

**Scope:** pairing, sessions and roles; the HTTP API, uploads and media endpoints; the relay
link, the Noise tunnel and the relay server; HTTPS; OSC and MIDI; show bundles; the OpenSlides
adapter; the host app's windows, web page cues and Tauri capabilities; the web remote's
storage and URL handling.

**Method:** reading the code paths from each network or file input to what it can reach,
with an attacker model per area (below), and a proof-of-concept or test for every finding
rated medium or higher. Severity assumes the host is used at events: the audience screen is
the main asset, and a crash mid-show is as bad as a takeover for many shows.

**Attackers considered:**

- Someone on the same network, not paired.
- Someone holding the relay link (it is shown as a QR code, so treat it as semi-public).
- A paired device with a low role (stage viewer, presenter), or a stolen API key.
- A rogue admin remote (phone, controller computer or admin API key).
- The author of a show file someone else opens.
- A malicious OpenSlides server or relay operator.

## Findings

| # | Severity | Area | Finding | Status |
|---|----------|------|---------|--------|
| 1 | High | OSC | A single UDP datagram with a few thousand nested bundles (or nested arrays in a message's type tags) overflowed the decoder's stack and aborted the whole app, before any authentication. Any local process could do this; with OSC open to the network, anyone on the LAN. | Fixed: the bundle framing and type tags are checked for nesting (at most 4 levels) before decoding. Tests with 5,000 nested bundles and 30,000 nested arrays. |
| 2 | Medium | OSC | An authenticated OSC sender kept the role it had when it authenticated, for 12 hours of activity: revoking its device or key, changing the role or "Disconnect all" did not affect it. | Fixed: the session stores the device and looks it up on every message. Test: role lowered and device revoked over OSC from another address. |
| 3 | Medium | Settings | An admin (including a rogue remote admin) could point OpenSlides or the relay at another server without retyping the password or token, and the host then sent the stored secret there. The OpenSlides login also followed redirects. | Fixed: a changed server (or OpenSlides user) clears the stored secret unless a new one comes with the change; the OpenSlides client no longer follows redirects. Tests for both. |
| 4 | Medium | Show files | A show file could link any file of the host as a video or as an image asset (or a symbolic link with a media name pointing at, say, an SSH key), and every paired device could then download it. Web page cues in a file were not checked like those added in the app (`file:` URLs, the app's own origin). | Fixed: on opening, every file a show refers to must have a type its cue shows, also after following links; web page and capture cues are validated like actions. The media and asset endpoints check the file type again. Test with four kinds of malicious shows. |
| 5 | Medium | Rendering | Any paired device (also through the relay) could request slide renders at arbitrary sizes up to 7680 × 7680; each size was a new render at top priority and a new cache file, starving the outputs and filling the disk. | Fixed: requested sizes snap to six steps (or the output's own size). Unit test. |
| 6 | Medium | Relay link | A tunneled HTTP request could keep its body open forever; holders of the relay link could make the host buffer up to 2 MB per request, for 32 requests on each of 64 channels, without pairing. The same applied on the local network (plain HTTP and HTTPS: no header read timeout either). | Fixed: request bodies must keep arriving (30 s between chunks), pairing requests are limited to 16 KB, HTTP and HTTPS connections have a 30 s header read timeout and at most 1,024 open connections, and relay channels without a session or request are closed after 60 s. Test for the body limit; the timeouts were checked manually. |
| 7 | Medium | Relay server | The relay's queues were bounded by message count only (up to 32 MB per remote, 128 MB per host link), so registering a host and attaching remotes that do not read could exhaust a small server's memory. A stalled remote's task also waited forever. | Fixed: at most 2 MB queued per remote and 256 MB across the relay (configurable), host link queue reduced, writes to a remote time out after 20 s. Test: a stalled remote is dropped with its queue bounded. |
| 8 | Low | Pairing | 20 wrong attempts per minute from anywhere (also through the relay, where every channel has its own address) locked pairing for everyone, indefinitely. | Fixed: attempts with a wrong join token count only against their sender (they cannot succeed); the global lockout is kept apart for QR pairing, PIN-only pairing and the relay. Unit test. |
| 9 | Low | Remote | Any link with `#a=<token>` replaced the phone's stored session (unpairing it mid-show, or making it act as another device), and `#k=` replaced a stored relay key. | Fixed: a handed-over token is only used when none is stored; a stored relay key always wins (keys never change for a host id). |
| 10 | Low | Relay link | Plain `http://` relays were accepted anywhere, sending the host secret and the remote page across the internet in the clear. Credentials in the relay URL would have been logged. | Fixed: `http://` only for loopback and local network addresses; URLs with credentials are refused. |
| 11 | Low | Relay link | Leaving the relay for the local network picked the plain HTTP address even when HTTPS was on. | Fixed: HTTPS addresses come first in the routes sent to remotes. |
| 12 | Low | Relay link | Channels opening and closing in a loop sent connectivity updates to every session each time. | Fixed: the relay's device count is published at most twice a second. |
| 13 | Low | Uploads | A presenter could upload repeatedly or in parallel and fill the disk before an admin decided. | Fixed: at most 8 pending or arriving uploads per device and 8 GB pending overall (`inbox_full`). Test. |
| 14 | Low | Files | `settings.json` (relay access token), the host settings (tokens for hosts this computer controls) and the autosaved show were written with default permissions. | Fixed: all data files are written readable only by the user. Test for `settings.json`. |
| 15 | Low | OSC | The table of failed OSC authentications was never pruned (spoofed source addresses could grow it). | Fixed: pruned once it exceeds 1,024 entries. |
| 16 | Low | OpenSlides | Error bodies and single answers were read without a size limit. | Fixed: limited (4 KB for errors, 64 MB for data, as for the stream). |
| 17 | Info | Host app | `connection_info` returned the admin token to any webview that was not an output or a controller window. Not exploitable (Tauri refuses IPC from remote origins), but a web page cue that navigated to the app's own origin would have been one. | Fixed: only the operator window gets the admin token; web page cues cannot navigate to non-web schemes or the app's origin, and such URLs are refused for cues. Unit test. |
| 18 | Info | Relay link | A `#` in a tunneled path passed the path check before it was dropped by parsing (harmless: the WebSocket route needs an upgrade). | Fixed: paths with `#` are refused. Test. |
| 19 | Info | API keys | Key names were not cleaned of control characters. | Fixed. |
| 20 | Low | Admin rights | A rogue admin remote can open the API to the network and create an admin API key, which survives "Disconnect all". | Accepted: admin is full control by design; "Disconnect all" documents that API keys stay and the Control tab lists them. Give the admin role only to devices you control. |
| 21 | Low | Hotspot | The hotspot password is passed to `nmcli` on its command line, so other local users can see it in the process list while it starts. | Accepted: the password is also shown as a QR code on the host's screen; presenting laptops rarely have other users. |
| 22 | Low | Relay | The relay learns the host secret (it derives the host id from it), so a relay operator could register the same host id elsewhere. | Accepted: the secret only identifies the host; remotes check the host's Noise key, which never leaves it. Use relays you trust; "Reset relay identity" changes both. |
| 23 | Info | Relay link | After a remote closes a session and starts a new one on the same channel, a late close of the old session can end the new one. | Accepted: reliability only (no data crosses identities); the remote reconnects. |
| 24 | Info | Pointer | The pointer rate limit is per connection, so a presenter with several sessions can send more. | Accepted: bounded by the number of sessions; pointers are validated. |

## Checked and found sound

- **Pairing and sessions:** 128-bit join tokens and request ids, 256-bit device tokens; PIN and
  tokens compared in constant time; the join token is single-use; an approved token is handed
  out once; errors do not reveal whether the PIN was right; device tokens are stored as hashes.
- **Roles:** every action and event checks the current role; file-path actions are local-only;
  host-window tokens and local-only API keys work only from loopback, also over HTTP; relay
  sessions get addresses in `100::/64`, never loopback.
- **Noise and framing:** NK is the right pattern; the prologue binds the version and host id;
  the key comes from the URL fragment; ordering and replay break the session; no panics are
  reachable from frames (all indices are length-checked); reassembly and queues are capped.
- **HTTP:** no `X-Forwarded-For` trust; header values cannot inject CR/LF; only GET and POST are
  tunneled; response headers are allow-listed; CSP with `frame-ancestors 'none'`, `nosniff`,
  `no-referrer`; JSON endpoints require `application/json`.
- **Files:** bundle extraction rejects traversal and caps `show.json`; uploads keep one sanitized
  path component in a generated directory, with a type allow-list and a streamed size limit.
- **Markup:** no `{@html}` or `innerHTML` in the operator, outputs, stage display or remote;
  OpenSlides HTML becomes text blocks on the host.
- **TLS:** handshakes time out after 10 s; the rustls defaults apply; keys are private files.

## Verification

All fixes have tests in the Rust suites (`cargo test --workspace`) except the connection
timeouts, which were checked by hand against the development server: a connection that sends
an incomplete request is closed after 30 s, on the HTTP port and (after the TLS handshake) on
the HTTPS port. The OSC crash was reproduced before the fix with the decoder the OSC service
used (about 2,500 nested bundles, a 50 KB datagram, aborted a thread with a 2 MB stack; nested
arrays the same way).
