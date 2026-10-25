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

# The browser client, built from the committed lockfile in the same pinned Node image that
# web/scripts/docker-npm.sh uses. Only the static build output reaches the runtime image.
FROM node:22-bookworm-slim@sha256:de170b39439eb0e5a3444ad7d60b015614a381e10fecae2e30ec9e735122f38d AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web ./
RUN npm run build

# HOI4 Agent Tools, the MCP server behind the agent's read-only HOI4 tools (S03). Built from the
# exact upstream commit; `git rev-parse` must equal the pin, so the commit hash is the content
# check. It needs Node 22 or 24, while Setup needs Debian's own /usr/bin/node (18), so the tools
# get this image's Node at /opt/node22 and Setup keeps its own. Raise AGENT_TOOLS_REV deliberately
# and rerun the hoi4-tools integration tests: the profile pins the package version.
FROM node:22-bookworm-slim@sha256:de170b39439eb0e5a3444ad7d60b015614a381e10fecae2e30ec9e735122f38d AS agent-tools
ARG AGENT_TOOLS_REV=731e64f5b5c964bd1b33adad9c085bd3168b1940
RUN apt-get update \
 && apt-get install -y --no-install-recommends git ca-certificates \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/hoi4-agent-tools
RUN git -c core.autocrlf=false init -q . \
 && git -c core.autocrlf=false fetch -q --depth 1 https://github.com/klimPaskov/hoi4-agent-tools.git "${AGENT_TOOLS_REV}" \
 && git -c core.autocrlf=false checkout -q FETCH_HEAD \
 && test "$(git rev-parse HEAD)" = "${AGENT_TOOLS_REV}" \
 && rm -rf .git
RUN npm ci --no-audit --no-fund \
 && npm run build \
 && npm prune --omit=dev --no-audit --no-fund \
 && rm -rf src tests fixtures examples scripts \
 && node -e "const p=require('./package.json'); if (p.version !== '4.4.0') { throw new Error('unexpected Agent Tools version ' + p.version) }"

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
COPY --from=web /web/dist /app/web
COPY --from=agent-tools /usr/local/bin/node /opt/node22/bin/node
COPY --from=agent-tools /opt/hoi4-agent-tools /opt/hoi4-agent-tools
ENV FACTORY_LISTEN=0.0.0.0:8080 \
    FACTORY_DATA_ROOT=/data \
    FACTORY_GAME_REF=/game \
    FACTORY_NODE_EXECUTABLE=/usr/bin/node \
    FACTORY_WEB_DIST=/app/web \
    FACTORY_AGENT_TOOLS_NODE=/opt/node22/bin/node \
    FACTORY_AGENT_TOOLS_ENTRY=/opt/hoi4-agent-tools/dist/bin/stdio.js \
    HOME=/tmp
USER 10001
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=5s --start-period=20s --retries=6 \
  CMD ["/usr/local/bin/factory-server", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/factory-server"]
