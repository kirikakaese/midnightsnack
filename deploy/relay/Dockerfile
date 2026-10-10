# SPDX-License-Identifier: GPL-3.0-or-later
# The midnightsnack relay. Build from the repository root:
#   docker build -f deploy/relay/Dockerfile -t deck-relay .
# The relay serves the web remote, so the image builds it first.

FROM node:22-bookworm-slim AS web
WORKDIR /src
RUN corepack enable
COPY . .
RUN pnpm install --frozen-lockfile --filter "@midnightsnack/remote..." \
 && pnpm --filter @midnightsnack/remote build

FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
COPY --from=web /src/apps/remote/dist apps/remote/dist
RUN cargo build --release --locked -p midnightsnack-relay

FROM gcr.io/distroless/cc-debian12:nonroot
LABEL org.opencontainers.image.title="DECK relay" \
      org.opencontainers.image.description="Self-hostable relay for DECK remotes; forwards end-to-end encrypted traffic only" \
      org.opencontainers.image.licenses="GPL-3.0-or-later" \
      org.opencontainers.image.source="https://github.com/kirikakaese/midnightsnack"
COPY --from=build /src/target/release/deck-relay /usr/local/bin/deck-relay
ENV MIDNIGHTSNACK_RELAY_LISTEN=0.0.0.0:8080
EXPOSE 8080
USER nonroot
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s \
  CMD ["/usr/local/bin/deck-relay", "--healthcheck"]
ENTRYPOINT ["/usr/local/bin/deck-relay"]
