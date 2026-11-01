#!/usr/bin/env sh
# Run cargo tests on Linux (the platform the server image runs on) in the test-runner image
# built from docker/test-runner.Dockerfile. Setup's readiness rules differ by platform: an MCP
# route that is not installed blocks readiness on Windows only, so the S02 apply gates run here.
#
# Everything it needs is created on first use: the image `hoi4-factory-test-runner:1`, the docker
# network `hf-finish-net`, a disposable PostgreSQL container `hf-finish-pg` on that network (trust
# authentication, test data only), and the cache volumes hf-finish-target, hf-finish-cargo and
# hf-finish-setupcache. Set FACTORY_TEST_ADMIN_URL to use another PostgreSQL instead.
#
# Usage: scripts/linux-tests.sh -p factory-server --test setup_gates -- --nocapture
# Live tests read ANTHROPIC_API_KEY from the host environment (never printed).
# Stop the database afterwards with: docker rm -f hf-finish-pg
set -eu
HERE="$(cd "$(dirname "$0")/.." && pwd)"
if command -v cygpath >/dev/null 2>&1; then HERE="$(cygpath -m "$HERE")"; fi
IMAGE="${HF_TEST_IMAGE:-hoi4-factory-test-runner:1}"
NET="${HF_TEST_NETWORK:-hf-finish-net}"
PG_IMAGE="postgres:17.6-bookworm@sha256:f3bd19c606e442c3d7bdfa8002e03fe260a1023351e0ea4598032022b68dd6e3"

if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  docker build -f "$HERE/docker/test-runner.Dockerfile" -t "$IMAGE" "$HERE/docker"
fi
docker network inspect "$NET" >/dev/null 2>&1 || docker network create "$NET" >/dev/null
if [ -z "${FACTORY_TEST_ADMIN_URL:-}" ]; then
  if [ -z "$(docker ps -q --filter name=^hf-finish-pg$)" ]; then
    docker rm -f hf-finish-pg >/dev/null 2>&1 || true
    docker run -d --name hf-finish-pg --network "$NET" -e POSTGRES_HOST_AUTH_METHOD=trust "$PG_IMAGE" >/dev/null
    i=0
    until docker exec hf-finish-pg pg_isready -U postgres -q 2>/dev/null; do
      i=$((i + 1)); [ "$i" -gt 60 ] && { echo "PostgreSQL did not start" >&2; exit 1; }; sleep 1
    done
  fi
fi
for v in hf-finish-target hf-finish-cargo hf-finish-setupcache; do docker volume create "$v" >/dev/null; done

KEY_ARGS=""
if [ -n "${ANTHROPIC_API_KEY:-}" ]; then KEY_ARGS="-e ANTHROPIC_API_KEY"; fi
# Recorded model replies are written back to the checkout when FACTORY_RECORD_REPLIES is set.
mkdir -p "$HERE/crates/server/tests/fixtures/recorded"
REC_ARGS="-v $HERE/crates/server/tests/fixtures/recorded:/src/crates/server/tests/fixtures/recorded"
if [ -n "${FACTORY_RECORD_REPLIES:-}" ]; then REC_ARGS="$REC_ARGS -e FACTORY_RECORD_REPLIES"; fi
# shellcheck disable=SC2086
MSYS_NO_PATHCONV=1 exec docker run --rm --init --network "$NET" $KEY_ARGS $REC_ARGS \
  -v "$HERE:/src:ro" -v hf-finish-target:/target -v hf-finish-cargo:/cargo \
  -v hf-finish-setupcache:/setupcache \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 \
  -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_TERM_COLOR=never \
  -e FACTORY_TEST_ADMIN_URL="${FACTORY_TEST_ADMIN_URL:-postgres://postgres@hf-finish-pg:5432/postgres}" \
  -e FACTORY_TEST_SETUP_CACHE=/setupcache -e TMPDIR=/tmp \
  -w /src "$IMAGE" cargo test --locked "$@"
