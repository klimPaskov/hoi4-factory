#!/usr/bin/env sh
# Run cargo tests on Linux (the platform the server image runs on) inside the local
# `hf-finish-test:node` image: `hf-ws-test:git` (rust 1.97.1 + git) plus Debian's `nodejs` package,
# built once with
#   printf 'FROM hf-ws-test:git
RUN apt-get update && apt-get install -y --no-install-recommends nodejs
' | docker build -t hf-finish-test:node -
# (Setup accepts a Node runtime on Linux only when /usr/bin/node comes from that package.) Setup's readiness rules differ by platform: an MCP
# route that is not installed blocks readiness on Windows only, so the apply gates of S02 run here.
#
# Needs: a PostgreSQL superuser URL reachable from the container in FACTORY_TEST_ADMIN_URL
# (default: container `hf-finish-pg` on docker network `hf-finish-net`, trust authentication),
# and docker volumes hf-finish-target / hf-finish-cargo / hf-finish-setupcache (created on demand).
# Usage: scripts/linux-tests.sh -p factory-server --test setup_gates -- --nocapture
# Live tests read ANTHROPIC_API_KEY from the host environment (never printed).
set -eu
HERE="$(cd "$(dirname "$0")/.." && pwd)"
if command -v cygpath >/dev/null 2>&1; then HERE="$(cygpath -m "$HERE")"; fi
NET="${HF_TEST_NETWORK:-hf-finish-net}"
for v in hf-finish-target hf-finish-cargo hf-finish-setupcache; do docker volume create "$v" >/dev/null; done
KEY_ARGS=""
if [ -n "${ANTHROPIC_API_KEY:-}" ]; then KEY_ARGS="-e ANTHROPIC_API_KEY"; fi
# Recorded model replies are written back to the checkout when FACTORY_RECORD_REPLIES is set.
mkdir -p "$HERE/crates/server/tests/fixtures/recorded"
REC_ARGS="-v $HERE/crates/server/tests/fixtures/recorded:/src/crates/server/tests/fixtures/recorded"
if [ -n "${FACTORY_RECORD_REPLIES:-}" ]; then REC_ARGS="$REC_ARGS -e FACTORY_RECORD_REPLIES"; fi
# shellcheck disable=SC2086
MSYS_NO_PATHCONV=1 exec docker run --rm --network "$NET" $KEY_ARGS $REC_ARGS \
  -v "$HERE:/src:ro" -v hf-finish-target:/target -v hf-finish-cargo:/cargo \
  -v hf-finish-setupcache:/setupcache \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 \
  -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_TERM_COLOR=never \
  -e FACTORY_TEST_ADMIN_URL="${FACTORY_TEST_ADMIN_URL:-postgres://postgres@hf-finish-pg:5432/postgres}" \
  -e FACTORY_TEST_SETUP_CACHE=/setupcache -e TMPDIR=/tmp \
  -w /src hf-finish-test:node cargo test --locked "$@"
