#!/usr/bin/env bash
# One-step local setup for Windows (Git Bash), macOS and Linux. Asks for each secret that is
# missing, saves it where it is needed, then starts the stack and checks that it is ready.
# Run it again at any time: press Enter at a prompt to keep the value already saved.
#
#   .env                          FACTORY_ANTHROPIC_API_KEY, FACTORY_GITHUB_CLIENT_SECRET
#   shell environment             ANTHROPIC_API_KEY for live tests (~/.zshrc on macOS,
#                                 ~/.bashrc on Linux, a Windows user variable on Windows)
#   .work/github-app/private-key.pem   the GitHub App private key
#
# Secrets are read without echo and never printed. The game folder is found by
# scripts/compose.sh. Usage: scripts/setup-local.sh [--no-start]
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$HERE"

START=1
[ "${1:-}" = "--no-start" ] && START=0

case "$(uname -s)" in
  Darwin) OS=macos ;;
  MINGW* | MSYS* | CYGWIN*) OS=windows ;;
  *) OS=linux ;;
esac

say() { printf '%s\n' "$*"; }
ok() { printf '  ok: %s\n' "$*"; }
warn() { printf '  warning: %s\n' "$*" >&2; }

# --- .env helpers (values are never printed) -------------------------------------------------
env_get() {
  [ -f .env ] || return 0
  tr -d '\r' <.env | sed -n "s/^$1=//p" | tail -n 1 \
    | sed -e 's/^"\(.*\)"$/\1/' -e "s/^'\(.*\)'$/\1/"
}

env_set() {
  local tmp=".env.setup-tmp.$$"
  { [ -f .env ] && tr -d '\r' <.env | grep -v "^$1=" || true; } >"$tmp"
  printf '%s=%s\n' "$1" "$2" >>"$tmp"
  mv "$tmp" .env
  chmod 600 .env
}

ask_secret() { # ask_secret <prompt> -> value on stdout (empty when the user presses Enter)
  local value=""
  printf '%s' "$1" >&2
  read -rs value || true
  printf '\n' >&2
  printf '%s' "$value" | tr -d '\r' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//'
}

ask_line() {
  local value=""
  printf '%s' "$1" >&2
  read -r value || true
  printf '%s' "$value"
}

# --- 1. Prerequisites ------------------------------------------------------------------------
say "HOI4 Factory local setup ($OS)"
say ""
say "1. Checking tools"
missing=0
for tool in git docker curl; do
  if command -v "$tool" >/dev/null 2>&1; then ok "$tool"; else warn "$tool is not installed"; missing=1; fi
done
if scripts/py.sh -c 'import sys' >/dev/null 2>&1; then ok "Python 3.11+ (scripts/py.sh)"; else
  warn "no Python 3.11+ found (see docs/07-developer-workflow/DEVELOPER-SETUP.md)"; missing=1
fi
[ "$missing" = 0 ] || { say "Install the missing tools, then run this script again."; exit 1; }

if [ ! -f .env ]; then
  if [ -f .env.example ]; then cp .env.example .env; else : >.env; fi
  chmod 600 .env
  ok "created .env"
fi

# --- 2. Anthropic API key --------------------------------------------------------------------
say ""
say "2. Anthropic API key (Setup analysis and live tests)"
anthropic="$(env_get FACTORY_ANTHROPIC_API_KEY)"
[ -n "$anthropic" ] || anthropic="${ANTHROPIC_API_KEY:-}"
if [ -n "$anthropic" ]; then
  new="$(ask_secret "  Paste a new key, or press Enter to keep the saved one: ")"
else
  new="$(ask_secret "  Paste your Anthropic API key (Enter to skip): ")"
fi
[ -n "$new" ] && anthropic="$new"

if [ -n "$anthropic" ]; then
  case "$anthropic" in sk-ant-*) ;; *) warn "the key does not start with sk-ant-, check that you pasted the whole key" ;; esac
  # Free check: list models. The key goes to curl on stdin, not on its command line.
  code="$(printf 'header = "x-api-key: %s"\nheader = "anthropic-version: 2023-06-01"\n' "$anthropic" \
    | curl -s -o /dev/null -w '%{http_code}' -K - https://api.anthropic.com/v1/models || true)"
  case "$code" in
    200) ok "the key works" ;;
    401 | 403) warn "Anthropic rejected the key (HTTP $code). It is saved anyway, run this script again with the right one" ;;
    *) warn "could not check the key (HTTP ${code:-none}), saved anyway" ;;
  esac
  env_set FACTORY_ANTHROPIC_API_KEY "$anthropic"
  ok "saved in .env"
  case "$OS" in
    windows)
      setx ANTHROPIC_API_KEY "$anthropic" >/dev/null
      ok "saved as a Windows user variable (new terminals get it)"
      ;;
    *)
      profile="$HOME/.zshrc"
      [ "$OS" = linux ] && profile="$HOME/.bashrc"
      touch "$profile"
      tmp="$profile.setup-tmp.$$"
      grep -v '^export ANTHROPIC_API_KEY=' "$profile" >"$tmp" || true
      printf "export ANTHROPIC_API_KEY='%s'\n" "$anthropic" >>"$tmp"
      cat "$tmp" >"$profile" && rm -f "$tmp"
      chmod 600 "$profile"
      ok "saved in $profile (new terminals get it)"
      ;;
  esac
else
  say "  skipped: Setup analysis and live model tests stay unavailable"
fi

# --- 3. GitHub sign-in client secret ---------------------------------------------------------
say ""
say "3. GitHub App client secret (Sign in with GitHub)"
say "   From https://github.com/settings/apps/hoi4-factory-dev > Credentials > Generate a new client secret"
secret="$(env_get FACTORY_GITHUB_CLIENT_SECRET)"
if [ -n "$secret" ]; then
  new="$(ask_secret "  Paste a new secret, or press Enter to keep the saved one: ")"
else
  new="$(ask_secret "  Paste the client secret (Enter to skip): ")"
fi
if [ -n "$new" ]; then
  env_set FACTORY_GITHUB_CLIENT_SECRET "$new"
  ok "saved in .env"
elif [ -n "$secret" ]; then
  ok "kept"
else
  say "  skipped: Sign in with GitHub stays off, email sign-in works"
fi

# --- 4. GitHub App private key ---------------------------------------------------------------
say ""
say "4. GitHub App private key (GitHub import)"
key_file=".work/github-app/private-key.pem"
if [ -s "$key_file" ]; then
  prompt="  Drag a new .pem file here, or press Enter to keep the saved one: "
else
  say "   From https://github.com/settings/apps/hoi4-factory-dev > Generate a private key"
  prompt="  Drag the downloaded .pem file into this window and press Enter (Enter alone to skip): "
fi
path="$(ask_line "$prompt")"
# Terminals quote dragged paths or escape their spaces.
path="$(printf '%s' "$path" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' \
  -e "s/^'\(.*\)'$/\1/" -e 's/^"\(.*\)"$/\1/' -e 's/\\ / /g')"
if [ -n "$path" ] && [ "$OS" = windows ] && command -v cygpath >/dev/null 2>&1; then
  path="$(cygpath -u "$path")"
fi
if [ -n "$path" ]; then
  if [ -f "$path" ] && head -n 1 "$path" | grep -q 'BEGIN .*PRIVATE KEY'; then
    mkdir -p .work/github-app
    cp "$path" "$key_file"
    chmod 600 "$key_file"
    ok "saved as $key_file"
  else
    warn "that is not a private key file, nothing changed"
  fi
elif [ -s "$key_file" ]; then
  ok "kept"
else
  : >"$key_file"
  say "  skipped: GitHub import stays unavailable"
fi

# --- 5. Start and check ----------------------------------------------------------------------
say ""
if [ "$START" = 0 ]; then
  say "Done. Start the stack with: scripts/compose.sh up -d --build --wait"
  exit 0
fi
say "5. Starting the stack"
if ! docker info >/dev/null 2>&1; then
  say "  Docker is not running. Start Docker Desktop, this script waits up to 3 minutes."
  [ "$OS" = macos ] && open -a Docker 2>/dev/null || true
  i=0
  until docker info >/dev/null 2>&1; do
    i=$((i + 1))
    [ "$i" -gt 90 ] && { warn "Docker did not start. Start it, then run: scripts/compose.sh up -d --build --wait"; exit 1; }
    sleep 2
  done
fi
ok "Docker is running"
scripts/compose.sh pull --ignore-buildable --quiet
scripts/compose.sh up -d --build --wait
port="$(env_get FACTORY_HOST_PORT)"
port="${port:-8080}"
ready="$(curl -s "http://127.0.0.1:$port/readyz" || true)"
if printf '%s' "$ready" | grep -q '"status":"ok"'; then
  ok "the server is ready"
else
  warn "the server is not ready yet: $ready"
fi
say ""
say "Done. Open http://127.0.0.1:$port (sign-up emails arrive at http://127.0.0.1:8025)."
say "Open a new terminal, and restart Claude Code from it, so both get ANTHROPIC_API_KEY."
