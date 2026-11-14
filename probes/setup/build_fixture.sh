#!/bin/sh
# Build the F-SETUP-IMPORT working copy from the original synthetic sources.
# Usage: build_fixture.sh <fixture_source_dir> <destination_project_dir>
# Produces CRLF, UTF-8 BOM, binary and executable variants deterministically,
# then creates a Git history with a dirty index and untracked files.
set -eu
PY="${PYTHON:-python3}"
SRC="$1"
DST="$2"
mkdir -p "$DST"
cp -R "$SRC/." "$DST/"
cd "$DST"
# CRLF focus file and UTF-8 BOM localisation, generated from LF sources.
"$PY" -I -c 'import sys;p=sys.argv[1];b=open(p,"rb").read().replace(b"\r\n",b"\n").replace(b"\n",b"\r\n");open(p,"wb").write(b)' common/national_focus/exp01_synthetic_focus.txt
"$PY" -I -c 'import sys;p=sys.argv[1];b=open(p,"rb").read();open(p,"wb").write(b"\xef\xbb\xbf"+b if not b.startswith(b"\xef\xbb\xbf") else b)' localisation/english/exp01_synthetic_l_english.yml
# Deterministic binary custom asset, 4096 bytes from a SHA-256 chain.
# Optional variant for a completed-apply control: a descriptor that already
# names the conventional thumbnail, which import readiness requires.
if [ "${EXP01_DESCRIPTOR_PICTURE:-0}" = "1" ]; then
  echo 'picture="thumbnail.png"' >> descriptor.mod
fi
mkdir -p gfx
"$PY" -I -c 'import hashlib;s=b"EXP01 synthetic asset";o=b"";
while len(o)<4096:
    s=hashlib.sha256(s).digest();o+=s
open("gfx/exp01_custom_asset.bin","wb").write(o[:4096])'
find . -type f -exec chmod 0644 {} +
find . -type d -exec chmod 0755 {} +
chmod 0755 tools/exp01_check.sh
export GIT_CONFIG_NOSYSTEM=1 HOME=/nonexistent GIT_AUTHOR_NAME=exp01 GIT_AUTHOR_EMAIL=exp01@example.invalid \
  GIT_COMMITTER_NAME=exp01 GIT_COMMITTER_EMAIL=exp01@example.invalid \
  GIT_AUTHOR_DATE="2026-10-01T00:00:00Z" GIT_COMMITTER_DATE="2026-10-01T00:00:00Z"
G="git -c init.defaultBranch=main -c core.hooksPath=/dev/null -c commit.gpgsign=false -c core.autocrlf=false"
$G init -q .
$G add descriptor.mod common localisation AGENTS.md .agents README.md tools exp01_setup gfx
$G commit -q -m "EXP01 synthetic baseline"
# Dirty index: a staged localisation edit.
printf ' EXP01_staged_line:0 "Staged but not committed"\r\n' >> localisation/english/exp01_synthetic_l_english.yml
$G add localisation/english/exp01_synthetic_l_english.yml
# Unstaged worktree edit of a tracked file.
printf '\nLocal unstaged edit by an author.\n' >> README.md
# Untracked files. notes/untracked-idea.txt was copied but never added.
printf 'scratch bytes\n' > gfx/exp01_scratch.tmp
chmod 0644 gfx/exp01_scratch.tmp
$G --no-optional-locks status --porcelain=v2 --untracked-files=all
