# Security model

midnightsnack runs a network server on the presenting laptop. This document describes what it
protects, against whom, and the known limits.

## Assets

- **Control of the audience screen** — the most important asset; an attacker showing arbitrary
  content or blacking out a live event is the main threat.
- **Files on the host** — shows, media, and anything else the app can read.
- **Show content** — may be confidential before the event (e.g. award winners).

## Trust boundaries

- The host machine and its operator are trusted.
- Everything reaching the server over the network is untrusted until paired.
- Paired devices are trusted only as far as their role allows.

## Controls

| Threat                                        | Control                                                                                      |
| --------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Stranger on the Wi-Fi controls the show       | Pairing needs the one-time join token (QR), the 6-digit PIN shown on the host, **and** operator approval (unless auto-approve is enabled). |
| PIN brute force                               | 5 failures per address → 60 s lockout; 20 failures overall per minute → global 60 s lockout; join token required in addition. |
| Stolen/old QR code                            | Join token rotates after every successful submission; "Disconnect all" also rotates the PIN. |
| Lost or compromised phone                     | Revoke the device in the operator view; "Disconnect all" forgets every remote device.        |
| Paired remote reads/writes host files         | File-system actions (`add_files`, `open_show`, `save_show` with a path) are local-only and refused from the network regardless of role. Media is only served for slides in the current show. |
| Host window token reused from the network     | Host window tokens are in-memory, regenerated on every start, and only accepted from loopback addresses. |
| Session tokens leaked from disk               | Only SHA-256 hashes of device tokens are stored (`devices.json`).                             |
| Slide images fetched without pairing          | Requires a per-connection random media key, valid only while that connection is open.        |
| Malicious show bundle                         | Bundled media names are restricted to a single path component (no traversal); the `show.json` size is capped. |
| Web page cue attacks the host                 | Web pages run in their own webview without IPC access, incognito unless "keep logins" is set (then with a data directory per cue), and can be kept on their site. |
| Screen contents leak to the network           | Capture streams need the media key of a live connection; listing windows (titles) is admin-only. Capture only runs while a capture cue is viewed. |
| Malicious presentation file                   | Office files are converted by LibreOffice in a separate process with a private profile and a timeout; notes are parsed with a size-limited XML reader. |
| Control surface keys leaked or misused         | API keys are devices with a role: hashed on disk, revocable, shown once. By default they (and OSC) only work from the host itself; opening them to the network is an explicit setting. OSC senders on other computers must authenticate with a key; failed attempts are throttled. "Disconnect all" keeps API keys (configured integrations); revoke them individually. |
| Uploads used to fill the disk or plant files  | Uploads need a paired device of presenter role or higher, are limited to 2 GB and to media/presentation types, are stored under a generated directory with a sanitized single-component name, and wait in the inbox until an admin accepts them. Pending uploads are deleted on restart. |
| Pointer spam                                  | Pointer messages need the presenter role and are capped at 60 per second per device; drawings are validated (points, width, color) and capped per slide. |
| PIN guessing from a second computer           | Pairing without the QR code's join token is only accepted while every request needs the operator's approval (auto-approve off); the PIN lockouts apply. |
| Relay operator reads or alters the show       | Remotes and hosts talk Noise NK end to end through the relay; the host's static key comes from the QR code's URL fragment (never sent to servers). The relay sees only ciphertext and connection metadata; tampering breaks the session. |
| Someone else registers this host on a relay   | The host id is derived from a 256-bit host secret by the relay; only the secret's owner can register it. Relays can require an access token from hosts. |
| Host-only tokens used through the relay       | Tunneled requests and sessions get a synthetic address (`100::/64`) that is never loopback, so host-window tokens, local-only API keys and local-only actions are refused exactly as from the LAN. Video, audio and capture streams are not served through the relay. |
| Passive sniffing on the LAN                   | Optional HTTPS with a generated certificate; its SHA-256 fingerprint is shown in the Connect tab for comparison on the browser's warning page. |
| Relay or join links shared too widely         | Pairing still needs the PIN and approval. "Reset relay identity" changes keys and host id; old relay links stop working. |
| OpenSlides credentials leak                   | The OpenSlides password is stored in its own file in the data directory, readable only by the user, changeable only by admins, and never sent to clients (they see whether one is set). |
| OpenSlides content injects markup             | Motion and topic HTML is reduced to plain text blocks on the host; clients render text only. |
| Remote page embedded/clickjacked              | `Content-Security-Policy` with `frame-ancestors 'none'`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`. |

## Known limitations

- **Plain HTTP on the LAN by default.** Browsers do not trust self-signed certificates, so
  HTTPS is opt-in (Connect tab) and plain HTTP keeps running for host windows and devices that
  cannot accept the warning. On plain HTTP, someone who can passively sniff the network can read
  session tokens and impersonate a paired device. Use HTTPS, a trusted network or the host's own
  hotspot for events where this matters.
- **The relay serves the web remote.** Phones that only reach the relay load the remote's code
  from it. Encryption protects against a passive or curious relay, its logs and the network, but
  an operator who deliberately serves modified code could read what that phone does. Only use
  relays you or people you trust run.
- **Token handover in URLs.** When a remote switches between the LAN and the relay, its device
  token travels in the URL fragment (not sent to servers) and is removed from the address bar
  and history immediately.
- Relay channels each get their own address for per-client PIN lockouts; the global lockout
  (20 failures per minute) bounds attempts through the relay as a whole.
- Rate limiting is per IP address; many devices behind one NAT share a lockout.
- The operator window's Content-Security-Policy allows connections to any host on the network,
  because controller windows (one host running another host's show) load slides and the
  WebSocket from the other computer. The window only runs midnightsnack's own code.
- MIDI input acts with the operator role and is not authenticated: anyone who can plug a MIDI
  device into the host can run the show (they could also press its keys).
- Everything the configured OpenSlides account can read about the meeting's agenda, motions and
  speakers is sent to every paired device of the show. Use an account with audience-level
  visibility.
- The PIN is stable for a host session until "Disconnect all"; the one-time join token is the
  per-pairing secret.

## Reporting

See [SECURITY.md](../SECURITY.md).
