# Linux test runner for `scripts/linux-tests.sh`. The server image runs on Linux, and Setup's
# readiness rules differ by platform, so the S02 apply gates are verified here.
# Setup accepts a Node runtime on Linux only when /usr/bin/node comes from Debian's own
# `nodejs` package, so Node is installed with apt and pinned like the server image.
FROM rust:1.97.1-slim-bookworm@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3
ARG NODEJS_VERSION=18.20.4+dfsg-1~deb12u3
RUN apt-get update \
 && apt-get install -y --no-install-recommends git ca-certificates nodejs=${NODEJS_VERSION} \
 && rm -rf /var/lib/apt/lists/* \
 && /usr/bin/node --version \
 && git config --system core.autocrlf false
