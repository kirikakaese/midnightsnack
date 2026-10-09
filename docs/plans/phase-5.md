# Phase 5 — Connectivity

Goal: remotes reach the host however the room is set up — the local network, the laptop's own
hotspot, HTTPS on the LAN, or an optional, self-hosted relay on the internet that only ever sees
ciphertext. The operator picks the connection modes; remotes fall back to the relay when the
local network fails.

## Tasks

### Connection modes (Connect tab → Connectivity)
- [x] **LAN** (always on): every usable interface is listed with its name; hotspot interfaces are
      recognized and preferred for the QR code. The operator chooses which address the QR code
      uses.
- [x] **HTTPS on the LAN** (optional): a self-signed certificate generated on first use (all LAN
      addresses and `localhost` as names), served on its own port (default 4749) next to plain
      HTTP. The certificate's SHA-256 fingerprint is shown so people can check the browser's
      warning page; "new certificate" replaces it.
- [x] **Relay** (optional, off by default, no default server): relay URL and optional access
      token; status (connecting, connected, error with reason), number of remotes via the relay,
      "reset relay identity". The join QR code can point at the relay.
- [x] Join links carry their kind (`lan`, `https`, `relay`) and a label; the pairing panel shows
      one QR code at a time with a selector.

### Relay server (`crates/relay`)
- [x] A small `axum` binary: hosts connect with a WebSocket (`/relay/v1/host`, bearer secret,
      optional access token required by the relay operator); remotes connect to
      `/relay/v1/remote/{host_id}`. The relay multiplexes remote channels over the host's one
      connection and forwards opaque binary frames.
- [x] Serves the web remote at `/r/{host_id}` (same build as the host embeds) and `/healthz`.
- [x] Limits: hosts, remotes per host, frame size, per-remote message rate, slow consumers
      dropped, idle pings. Host ids are derived from the host's secret, so they cannot be
      claimed by someone else; a reconnecting host replaces its old link.
- [x] Configuration by flags and environment variables; logs without payloads.
- [x] Docker image (multi-stage, non-root, health check) published to GHCR on release;
      `docker-compose.yml` example with Caddy for TLS; relay binaries attached to releases.

### End-to-end encryption
- [x] Noise `NK` (`Noise_NK_25519_ChaChaPoly_BLAKE2s`) between remote (initiator) and host
      (responder). The remote knows the host's static public key from the join link's fragment
      (never sent to the relay); the relay cannot read or forge traffic.
- [x] Inside the encrypted channel, a small framing layer carries the WebSocket session (stream
      0, fragmented messages, pings for latency) and HTTP requests/responses (pairing, slide
      images, assets, uploads) on further streams. The host answers HTTP requests by calling its
      own router in-process, with a per-channel synthetic, non-loopback address, so every
      existing rule (roles, local-only actions, local-only API keys, PIN lockouts) applies.
- [x] Rust side with `snow`; browser side implemented on audited `@noble` primitives, tested
      for interoperability.

### Remote: transport and fallback
- [x] The web remote works over a "transport": direct (LAN/HTTPS) or relay. Over the relay,
      slide images and assets are fetched through the tunnel and shown from a small blob cache;
      video/audio previews and capture previews are not streamed (a placeholder is shown).
- [x] Paired devices learn the host's other routes (LAN addresses, relay URL and key). If the
      LAN connection is lost for a while and a relay is configured, the remote switches to the
      relay automatically (with a cancel button), carrying its device token in the URL
      fragment; on the relay a "use local network" button goes back.

### Hotspot
- [x] Guided setup per OS in the Connectivity panel: on Linux with NetworkManager the host can
      start and stop a hotspot (`nmcli`); on Windows and macOS the panel opens the system
      settings page with step-by-step instructions. A "join this Wi-Fi" QR code is generated
      from SSID and password.
- [x] User guide per OS (Windows Mobile Hotspot, macOS Internet Sharing, Linux NetworkManager).

### Device management polish
- [x] Devices show how they are connected (this computer, LAN, HTTPS, relay) and from which
      address, last seen time for offline devices; rename devices; "forget offline devices".
- [x] Pending pairing requests over the relay are labeled as such.

### Docs and tests
- [x] ADR for relay and connectivity; security model update (relay threat model, HTTPS,
      fragment handling); user guides for connectivity, hotspot and relay hosting (Docker,
      Caddy/nginx, systemd); protocol spec for the relay and tunnel framing.
- [x] Unit tests: tunnel framing, Noise interop vectors (Rust ↔ TypeScript), relay limits,
      hotspot interface detection; server tests: tunneled HTTP and WebSocket through a real
      relay, local-only rules over the relay, HTTPS listener; E2E: pairing and running the
      show from a phone through the relay.

## Acceptance criteria

1. With a relay running in Docker behind TLS, a phone on mobile data scans the relay QR code,
   pairs with PIN and approval, sees the current slide and advances the show.
2. The relay's logs and memory never contain plaintext: a packet capture on the relay shows only
   Noise ciphertext after the handshake.
3. A remote paired on the LAN switches to the relay when the Wi-Fi drops, and back to the LAN
   with one tap.
4. With HTTPS enabled, a phone opens `https://<lan-ip>:4749/`, accepts the certificate (whose
   fingerprint matches the one shown on the host) and works as on plain HTTP.
5. On Linux with NetworkManager the host starts a hotspot; the hotspot address is preferred in the
   QR code and a phone joins via the Wi-Fi QR code.
6. API keys restricted to this computer and host-window tokens are refused through the relay.

## Results

- Rust: protocol 75, core 70, server 22 unit + 20 integration + 5 relay + 2 HTTPS + 1 Noise
  vector tests, relay 3, render 13, capture 3, control 8, host 7 (plus ignored ones that need LibreOffice, a sample deck or a
  display). Frontend: UI 14 (new: Noise NK against `snow`'s vectors), host 6 (new: Wi-Fi QR
  codes), Companion 7; Playwright 13 (new: pairing and running the show through a relay;
  falling back from the LAN to the relay and back; staying on the LAN).
- The relay integration tests run a real relay, a host connected to it and a Noise client in
  the role of a phone: pairing through the tunnel (requests labeled as relay requests),
  WebSocket session, actions, routes, device path; host-window tokens and local-only API keys
  refused through the relay; a wrong host key and an offline host fail cleanly; relays with an
  access token refuse hosts without it.
- Verified in the real host on Linux (WebKitGTK, X11) with a local relay: HTTPS turned on from
  the Connect tab (port 4749, fingerprint shown, persists across restarts); relay connected from
  the Connect tab; the QR selector offers local network, HTTPS and relay links; a device paired
  with the PIN and approved in the app opened the relay page with a handed-over token, ran as a
  presenter through the relay and appeared as "Through the relay" in the device list; hotspot
  guidance shown without NetworkManager.
- The TypeScript Noise implementation produces byte-identical handshake and transport messages
  to `snow` for fixed keys.

## Known limitations and deviations

- **Docker image** is built and smoke-tested in CI (`relay-image` job); it could not be built in
  the development container (no Docker daemon). Multi-arch images are pushed to GHCR on release.
- **Hotspot:** starting one from the app needs NetworkManager (Linux); it is untested on real
  Wi-Fi hardware here (no Wi-Fi in the container). Windows and macOS get instructions and a
  shortcut to their settings, because their hotspots cannot be started without elevated helpers.
- **Through the relay** phones do not get video/audio or capture previews (placeholder instead)
  and uploads go through the tunnel (paced to the connection). Controller mode stays LAN-only.
- **The relay serves the web remote**, so it must run the same or a newer version than the hosts
  and be trusted not to serve modified code (documented in the security model). It must be
  hosted at the root of its domain.
- **Fallback** is driven by the phone (browsers cannot probe the LAN from an HTTPS relay page):
  after about 8 s without the LAN it moves to the relay after a 5 s countdown. In the E2E test
  the way back is checked up to the handover, because the test machine's network does not let
  the browser reach the host's LAN address.
- Acceptance criteria 1, 2 and 5 (a phone on mobile data, packet capture on a public relay, a
  real hotspot) need hardware and a public server; they are covered by the tests above but were
  not run end to end here.
