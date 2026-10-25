#!/usr/bin/env sh
# Run `docker compose` for this repository with HOI4_GAME_DIR resolved for this machine, so one
# .env works unchanged on Windows (Git Bash), macOS and Linux.
#
# HOI4_GAME_DIR is taken from the shell, then from .env, and used only when that folder holds a
# Hearts of Iron IV install here (launcher-settings.json). Otherwise the game is looked up in this
# machine's Steam libraries, including extra libraries listed in libraryfolders.vdf. A value found
# here is exported, and the shell environment takes precedence over .env in Compose.
#
# Usage: scripts/compose.sh up -d --build --wait | down | logs server | ...
set -eu
HERE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$HERE"

GAME_SUBDIR="steamapps/common/Hearts of Iron IV"

is_game_dir() { [ -n "$1" ] && [ -f "$1/launcher-settings.json" ]; }

steam_roots() {
  case "$(uname -s)" in
    Darwin) printf '%s\n' "$HOME/Library/Application Support/Steam" ;;
    MINGW* | MSYS* | CYGWIN*) printf '%s\n' "C:/Program Files (x86)/Steam" "C:/Program Files/Steam" ;;
    *) printf '%s\n' "$HOME/.local/share/Steam" "$HOME/.steam/steam" \
      "$HOME/.var/app/com.valvesoftware.Steam/.local/share/Steam" ;;
  esac
}

# Every Steam library on this machine: each root, then the "path" entries of its
# libraryfolders.vdf (VDF escapes a backslash as two, so D:\\SteamLibrary becomes D:/SteamLibrary).
steam_libraries() {
  steam_roots | while IFS= read -r root; do
    printf '%s\n' "$root"
    vdf="$root/steamapps/libraryfolders.vdf"
    if [ -f "$vdf" ]; then
      tr -d '\r' <"$vdf" | sed -n 's/^[[:space:]]*"path"[[:space:]]*"\(.*\)"[[:space:]]*$/\1/p' \
        | sed 's/\\\\/\//g'
    fi
  done
}

find_game_dir() {
  steam_libraries | while IFS= read -r lib; do
    if is_game_dir "$lib/$GAME_SUBDIR"; then
      printf '%s\n' "$lib/$GAME_SUBDIR"
      break
    fi
  done
}

dir="${HOI4_GAME_DIR:-}"
if ! is_game_dir "$dir" && [ -f .env ]; then
  dir="$(tr -d '\r' <.env | sed -n 's/^HOI4_GAME_DIR=//p' | tail -n 1 \
    | sed -e 's/^"\(.*\)"$/\1/' -e "s/^'\(.*\)'$/\1/")"
fi
if ! is_game_dir "$dir"; then dir="$(find_game_dir)"; fi
if is_game_dir "$dir"; then
  export HOI4_GAME_DIR="$dir"
else
  echo "scripts/compose.sh: no Hearts of Iron IV install found in HOI4_GAME_DIR or the Steam" \
    "libraries. Set HOI4_GAME_DIR to the game folder (the one with launcher-settings.json)." >&2
fi
exec docker compose "$@"
