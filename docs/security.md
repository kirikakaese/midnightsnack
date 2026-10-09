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
| Remote page embedded/clickjacked              | `Content-Security-Policy` with `frame-ancestors 'none'`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`. |

## Known limitations

- **No transport encryption on the LAN (yet).** Browsers do not trust self-signed certificates,
  so the LAN server uses plain HTTP/WebSocket. Someone who can passively sniff the network can
  read session tokens and impersonate a paired device. Use a trusted network (or the host's own
  hotspot) for events where this matters. Optional HTTPS with a generated certificate and the
  end-to-end encrypted relay are planned (phase 5).
- Rate limiting is per IP address; many devices behind one NAT share a lockout.
- The PIN is stable for a host session until "Disconnect all"; the one-time join token is the
  per-pairing secret.

## Reporting

See [SECURITY.md](../SECURITY.md).
