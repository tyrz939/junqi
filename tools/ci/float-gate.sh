#!/usr/bin/env bash
# PORT.md §3.4: the eight float-free crates never name a float type and never cast to one.
# No exception: jane-schema/src/compile/fraction.rs reads a JSON decimal (0.16) as digits and a
# power of ten, and scales it to Permille or Q16 with integer ratios (Grok #7).
set -euo pipefail
cd "$(dirname "$0")/../.."
dirs=()
for c in core schema data world sim net bot art; do dirs+=("crates/jane-$c/src" "crates/jane-$c/build.rs"); done
existing=()
for d in "${dirs[@]}"; do [ -e "$d" ] && existing+=("$d"); done
hits=$(grep -rnE '\bf(32|64)\b|as f(32|64)' "${existing[@]}" --include='*.rs' || true)
if [ -n "$hits" ]; then
  echo "float gate: floats named in a float-free crate:"
  echo "$hits"
  exit 1
fi
echo "float gate: clean"
