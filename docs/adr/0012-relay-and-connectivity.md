# 0012. Relay and connectivity

- **Status:** accepted
- **Date:** 2026-10-09

## Context

Remotes normally reach the host on the local network. Some venues isolate Wi-Fi clients, have
no network at all, or the presenter's phone stays on mobile data. Phase 5 adds the laptop's own
hotspot, optional HTTPS on the LAN and an optional relay on the internet. The relay must be
self-hostable (no default third-party server) and must not be able to read or control the show.

## Decision

- **Relay as a dumb pipe.** `crates/relay` is a separate binary. The host keeps one outbound
  WebSocket to it; each remote opens its own WebSocket and the relay multiplexes them as numbered
  channels over the host's connection. It forwards opaque binary frames and enforces limits; it
  knows nothing about the protocol. Host ids are derived from a host secret
  (`base64url(SHA-256("midnightsnack relay host id" ‖ secret)[..16])`), so nobody else can
  register them.
- **Noise NK end to end.** Remote and host run `Noise_NK_25519_ChaChaPoly_BLAKE2s`. The host's
  static public key travels in the join link's URL fragment, which browsers never send to the
  server. NK authenticates the host to the remote; remotes authenticate as before, with pairing
  and device tokens inside the encrypted channel. The prologue binds the session to the host id
  and a version. Rust uses `snow`; the browser uses a small implementation on `@noble` curves,
  ciphers and hashes (WebCrypto's X25519 is not available on all phones in use), tested against
  `snow` with fixed vectors.
- **A tunnel, not a second protocol.** Inside Noise a small framing layer carries the normal
  WebSocket session and HTTP requests. The host answers tunneled HTTP by calling its own router
  in-process with a synthetic address from the IPv6 discard prefix (`100::/64`, unique per
  channel), never loopback. All existing rules apply unchanged: local-only actions and tokens,
  local-only API keys, PIN lockouts, media keys. `ws.rs` works over a socket trait so the same
  session code serves real and tunneled WebSockets.
- **The relay serves the remote.** A phone that cannot reach the host cannot load the remote
  from it either, so the relay embeds the same web remote build and serves it at
  `/r/{host_id}`. This means the relay operator is trusted to serve unmodified code — the
  encryption protects against a passive or curious relay, logs, and the network, not against an
  operator who ships a modified remote. This is acceptable because the relay is self-hosted by
  the people running the show, and is documented in the security model.
- **Media over the relay is fetched, not streamed.** Slide images and assets are requested
  through the tunnel and shown from blob URLs. Video/audio and capture previews are not offered
  over the relay (bandwidth, range requests); remotes show a placeholder. Uploads work through
  the tunnel.
- **Fallback in the remote.** A browser cannot probe the LAN from an HTTPS relay page (mixed
  content), so fallback is driven by the remote: paired devices receive the host's routes; when
  the LAN connection stays lost and a relay exists, the remote moves to the relay with its token
  in the URL fragment, and offers to go back. The QR code shows one chosen route.
- **HTTPS on the LAN with a self-signed certificate** on a separate port (`rustls` with the
  `ring` provider, `rcgen`), keeping plain HTTP for the host's own windows and for devices where
  certificate warnings are not acceptable. The fingerprint is shown in the host UI.
- **Hotspots use the OS.** Only NetworkManager is scriptable without elevated helpers; other
  systems get guided instructions and a shortcut to the right settings page. Hotspot interfaces
  are recognized by their typical names and addresses and preferred for the QR code.

## Consequences

- The relay must be updated together with hosts when the web remote changes (it serves the
  remote). The relay and the tunnel framing are versioned (`/relay/v1`, prologue version).
- Resetting the relay identity invalidates relay links: remotes must scan again.
- The host now has a TLS stack (`rustls`, `ring`) for HTTPS and for connecting to `wss://`
  relays, using the operating system's certificate store.
- Controller mode (a host controlling another host) stays LAN-only.
