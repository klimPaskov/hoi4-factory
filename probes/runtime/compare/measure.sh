#!/usr/bin/env bash
# Clean release build time, dependency count and stripped binary size for the
# two comparison crates. Target dirs are fresh per crate. Sources are already
# downloaded, so time excludes network.
set -uo pipefail
cd "$(dirname "$0")"
OUT=${1:?output dir}
mkdir -p "$OUT"
for c in ${ORDER:-thin async-openai}; do
  T="$OUT/target-$c"
  rm -rf "$T"
  (cd "$c" && cargo tree -e normal --prefix none --target x86_64-pc-windows-msvc | sed 's/ (\*)//' | sort -u > "$OUT/tree-$c.txt")
  s=$(date +%s.%N)
  (cd "$c" && CARGO_TARGET_DIR="$T" cargo build --release -j 12 > "$OUT/build-$c.log" 2>&1); rc=$?
  e=$(date +%s.%N)
  exe=$(ls "$T"/release/*.exe)
  echo "{\"crate\": \"$c\", \"exit\": $rc, \"clean_release_build_s\": $(python -c "print(round($e - $s, 1))"), \"unique_normal_deps_including_root\": $(wc -l < "$OUT/tree-$c.txt"), \"binary_bytes\": $(stat -c %s "$exe"), \"binary_sha256\": \"$(sha256sum "$exe" | cut -d' ' -f1)\"}" | tee "$OUT/measure-$c.json"
done
