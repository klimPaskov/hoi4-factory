#!/usr/bin/env sh
# Dependency advisory scan (production readiness PR-032) for the Rust workspace and the browser
# client, on Windows (Git Bash), macOS and Linux. cargo-audit is installed into .work/tools on
# first use (pinned version), so nothing is installed globally. Ignored advisories, each with its
# reason, are in .cargo/audit.toml. The HOI4 Agent Tools npm tree is audited where it is built:
# run `npm audit --omit=dev` in a clone at the pinned commit (see the developer setup guide).
#
# Usage: scripts/audit-deps.sh
set -eu
HERE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$HERE"
AUDIT="$HERE/.work/tools/bin/cargo-audit"
if [ ! -x "$AUDIT" ] && [ ! -x "$AUDIT.exe" ]; then
  CARGO_TARGET_DIR="$HERE/.work/cargo-audit-build" cargo install --locked cargo-audit --version 0.22.1 --root "$HERE/.work/tools"
  rm -rf "$HERE/.work/cargo-audit-build"
fi
[ -x "$AUDIT" ] || AUDIT="$AUDIT.exe"
status=0
echo "== cargo audit (Cargo.lock)"
"$AUDIT" audit || status=1
echo "== npm audit (web, production dependencies)"
web/scripts/docker-npm.sh audit --omit=dev || status=1
exit $status
