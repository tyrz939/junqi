#!/bin/sh
# One scene frame per mark: `tools/at.sh <dir> <hour> <backend> mark...` (PRESENTATION.md §6).
set -e
out=$1
hour=$2
be=$3
shift 3
mkdir -p "$out"
for m in "$@"; do
  ./target/release/jane sheet scene --backend "$be" --ticks 300 --hour "$hour" --at "$m" --out "$out/$m-$be.png" $EXTRA
done
