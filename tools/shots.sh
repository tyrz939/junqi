#!/bin/sh
# Renders the atmosphere review frames (PRESENTATION.md §6): `tools/shots.sh <dir> [backend]`.
# Each line is one `jane sheet scene` frame; the names say what the frame is for.
set -e
out=${1:-sheets/atmos}
be=${2:-wgpu}
jane=./target/release/jane
mkdir -p "$out"
$jane sheet scene --backend "$be" --ticks 600 --hour 17 --out "$out/town-1700.png"
$jane sheet scene --backend "$be" --ticks 600 --hour 18:40 --out "$out/town-1840.png"
$jane sheet scene --backend "$be" --ticks 600 --hour 22 --out "$out/town-2200.png"
$jane sheet scene --backend "$be" --ticks 600 --hour 6 --out "$out/town-0600.png"
