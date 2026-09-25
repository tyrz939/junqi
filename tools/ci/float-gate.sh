#!/usr/bin/env bash
# PORT.md §3.4: the eight float-free crates never name a float type and never cast to one.
# The one exception is jane-schema/src/fraction.rs: the build-side reader that turns a JSON
# fraction (0.16) into Permille or Q16 on the host, before any Rust data exists.
set -euo pipefail
cd "$(dirname "$0")/../.."
dirs=()
for c in core schema data world sim net bot art; do dirs+=("crates/jane-$c/src" "crates/jane-$c/build.rs"); done
existing=()
for d in "${dirs[@]}"; do [ -e "$d" ] && existing+=("$d"); done
hits=$(grep -rnE '\bf(32|64)\b|as f(32|64)' "${existing[@]}" --include='*.rs' \
  | grep -v '^crates/jane-schema/src/fraction.rs:' || true)
if [ -n "$hits" ]; then
  echo "float gate: floats named in a float-free crate:"
  echo "$hits"
  exit 1
fi
echo "float gate: clean"
