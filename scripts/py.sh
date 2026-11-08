#!/usr/bin/env sh
# Run a repository Python script with Python 3.11 or newer and UTF-8 I/O, on Windows (Git Bash),
# macOS and Linux. The default `python` on both development machines is 3.9, and the Windows
# locale encoding is cp1252, so plain `python` breaks the validator.
#
# Order: the Windows launcher `py -3.13`, then python3.13 down to python3.11 or a python3 that is
# new enough, then `uv run --python 3.13` (uv downloads a managed Python on first use).
#
# Usage: scripts/py.sh scripts/validate_package.py --root . --working-tree
#
# It refuses to start an interactive interpreter. Python 3.13's REPL started without a console
# prints the same traceback forever, and one background task filled C: with a 50 GB log that way.
set -eu
export PYTHONUTF8=1
export PYTHON_BASIC_REPL=1
if [ "$#" -eq 0 ]; then
  echo "scripts/py.sh: give a script, -c or -m. It does not start an interactive interpreter." >&2
  exit 2
fi
if command -v py >/dev/null 2>&1 && py -3.13 -c '' >/dev/null 2>&1; then exec py -3.13 "$@"; fi
for p in python3.13 python3.12 python3.11 python3 python; do
  if command -v "$p" >/dev/null 2>&1 \
    && "$p" -c 'import sys; sys.exit(sys.version_info < (3, 11))' >/dev/null 2>&1; then
    exec "$p" "$@"
  fi
done
if command -v uv >/dev/null 2>&1; then exec uv run --quiet --no-project --python 3.13 python "$@"; fi
echo "scripts/py.sh: no Python 3.11 or newer found. Install Python 3.13 (Windows: the python.org" \
  "installer with the py launcher; macOS: 'brew install python@3.13' or 'uv python install 3.13')." >&2
exit 127
