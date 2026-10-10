# Hosting a relay

A relay lets phones that cannot reach the presenting laptop directly (other network, mobile
data) control it over the internet. It is optional and self-hosted: DECK does not run
one, and hosts never use a relay you did not configure.

The relay is a small program (`deck-relay`) that forwards end-to-end encrypted traffic
between remotes and hosts; it cannot read slides, notes, tokens or actions. It also serves the
web remote to phones. It needs very little: any small Linux server or VPS with a public DNS name.

## What you need

- A server reachable from the internet with ports 80 and 443 open.
- A DNS name pointing at it (e.g. `relay.example.org`).
- TLS: serve the relay over `https://` (phones keep the screen on and treat the page as secure
  only there). The examples below get a free certificate from Let's Encrypt automatically.

## Docker Compose (recommended)

The repository contains a ready setup in [`deploy/relay`](../../deploy/relay): the relay image
(`ghcr.io/kirikakaese/deck-relay`) behind [Caddy](https://caddyserver.com), which
handles TLS.

```sh
mkdir deck-relay && cd deck-relay
curl -O https://raw.githubusercontent.com/kirikakaese/midnightsnack/main/deploy/relay/docker-compose.yml
curl -O https://raw.githubusercontent.com/kirikakaese/midnightsnack/main/deploy/relay/Caddyfile
cat > .env <<'ENV'
RELAY_DOMAIN=relay.example.org
# Optional but recommended: only hosts that know this token can use the relay.
MIDNIGHTSNACK_RELAY_ACCESS_TOKEN=change-me-to-something-long
ENV
docker compose up -d
```

Check it with `curl https://relay.example.org/healthz` (answers `ok`) and
`curl https://relay.example.org/relay/v1/info`. In the DECK host, open **Connect →
Relay**, enter `https://relay.example.org` and the access token, and click **Connect**.

Update with `docker compose pull && docker compose up -d`. Keep the relay at the same version as
your hosts (or newer): it serves the web remote to phones.

## Behind your own reverse proxy

Run the container (or the binary) and forward a HTTPS virtual host to port 8080. WebSockets must
be passed through, and timeouts should allow long-lived connections.

**nginx:**

```nginx
server {
    listen 443 ssl http2;
    server_name relay.example.org;
    # ssl_certificate ... (e.g. from certbot)

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_read_timeout 1h;
        proxy_send_timeout 1h;
        proxy_buffering off;
    }
}
```

**Traefik (labels on the relay container):**

```yaml
labels:
  - traefik.enable=true
  - traefik.http.routers.msrelay.rule=Host(`relay.example.org`)
  - traefik.http.routers.msrelay.tls.certresolver=letsencrypt
  - traefik.http.services.msrelay.loadbalancer.server.port=8080
```

The relay must be served at the root of its host name (not under a sub-path).

## Without Docker

Each release has `deck-relay` binaries for Linux (x86-64, ARM64), macOS and Windows.
With systemd:

```ini
# /etc/systemd/system/deck-relay.service
[Unit]
Description=DECK relay
After=network-online.target

[Service]
ExecStart=/usr/local/bin/deck-relay --listen 127.0.0.1:8080
Environment=MIDNIGHTSNACK_RELAY_ACCESS_TOKEN=change-me-to-something-long
DynamicUser=yes
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

Then put a reverse proxy with TLS in front of it as above.

## Settings

| Option                     | Environment variable                 | Default        |
| -------------------------- | ------------------------------------ | -------------- |
| `--listen ADDR`            | `MIDNIGHTSNACK_RELAY_LISTEN`         | `0.0.0.0:8080` |
| `--access-token TOKEN`     | `MIDNIGHTSNACK_RELAY_ACCESS_TOKEN`   | none (open)    |
| `--max-hosts N`            | `MIDNIGHTSNACK_RELAY_MAX_HOSTS`      | 200            |
| `--max-remotes-per-host N` | `MIDNIGHTSNACK_RELAY_MAX_REMOTES`    | 64             |
| `--max-queued-mb N`        | `MIDNIGHTSNACK_RELAY_MAX_QUEUED_MB`  | 256            |

The relay keeps at most 2 MB of messages waiting for each remote and `--max-queued-mb` across
all of them; remotes that stop reading are dropped (they reconnect on their own).

`RUST_LOG=debug` logs every remote connection; the default (`info`) logs hosts connecting and
disconnecting by a shortened id. The relay never logs message contents (it cannot read them) and
keeps no data on disk.

Without an access token anyone can use your relay for their own hosts (they still cannot see or
control yours). Set one if the server is public.

Hosts only accept relays over HTTPS; plain `http://` works for a relay on the same computer or
the local network (for testing).

## What the relay can and cannot do

- It **cannot** read or change slides, notes, device tokens or actions: those are encrypted
  between phone and host (Noise protocol, keys from the QR code). A relay that tampers with
  traffic only breaks the connection.
- It **can** see when hosts and phones are connected, from which IP addresses, and how much data
  flows.
- It **serves the web remote** to phones. A malicious relay operator could serve a modified
  remote that leaks what the phone does. Only use relays run by people you trust — ideally your
  own.

See [the security model](../security.md) for details.
