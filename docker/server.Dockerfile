# HOI4 Factory server image. Base images are pinned by digest (ADR-013).
# The image must never contain game files, credentials, user data or a coding CLI.
FROM rust:1.97.1-slim-bookworm@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3 AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
# Every workspace member must be present for Cargo to load the workspace, even when only the
# server is built: crates/* (the server and its libraries) and the runtime probe. The SQL files
# are embedded into the binary at compile time (sqlx::migrate!).
COPY crates ./crates
COPY migrations ./migrations
COPY probes/runtime/Cargo.toml ./probes/runtime/Cargo.toml
COPY probes/runtime/src ./probes/runtime/src
RUN cargo build --release --locked -p factory-server

# Extension point for the browser client (built in web/ by its own stage):
#   FROM node:<pinned digest> AS web
#   ... npm ci && npm run build ...
# then in the runtime stage:
#   COPY --from=web /web/dist /app/web
#   ENV FACTORY_WEB_DIST=/app/web

FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8
# git: the workspace crate drives the git CLI. ca-certificates: TLS to GitHub and mail relays.
# nodejs: the Node runtime Setup writes into the generated coding-client configuration. On Linux
# Setup accepts only /usr/bin/node installed by Debian's own `nodejs` package (it checks the
# root-owned dpkg checksum receipt), so a binary copied from a node image is refused. The version
# is pinned to what the digest-pinned base image's archive offers; raise it deliberately.
ARG NODEJS_VERSION=18.20.4+dfsg-1~deb12u3
RUN apt-get update \
 && apt-get install -y --no-install-recommends git ca-certificates nodejs=${NODEJS_VERSION} \
 && rm -rf /var/lib/apt/lists/* \
 && /usr/bin/node --version
RUN useradd --system --uid 10001 --no-create-home factory \
 && install -d -o 10001 -g 10001 -m 0755 /data \
 && install -d -m 0755 /game
COPY --from=build /src/target/release/factory-server /usr/local/bin/factory-server
ENV FACTORY_LISTEN=0.0.0.0:8080 \
    FACTORY_DATA_ROOT=/data \
    FACTORY_GAME_REF=/game \
    FACTORY_NODE_EXECUTABLE=/usr/bin/node \
    HOME=/tmp
USER 10001
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=5s --start-period=20s --retries=6 \
  CMD ["/usr/local/bin/factory-server", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/factory-server"]
