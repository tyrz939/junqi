#!/usr/bin/env bash
# The shippable PSP build (PORT.md §13.12): the EBOOT, the art pack, the presenter tables,
# the audio module, the baked county (assets/world/county.bp) and its baked chunks (county.jck) in PSP/GAME/jane/, zipped. cargo-psp names its output after the crate once
# Psp.toml sets a title (psp-game.EBOOT.PBP); a stale EBOOT.PBP beside it once shipped an old
# build, so this takes the named file and checks that the fresh .prx is inside the zip.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-dist/TheBellAtNine-PSP.zip}"
stage="target/psp-dist"
rel="spikes/psp-game/target/mipsel-sony-psp/release"

# The county from its source (data/world/county), proved; unchanged if it is already its source baked.
cargo run --release -q -p jane-cli -- bake-world
(cd spikes/psp-game && cargo +nightly psp --release)
rm -f "$rel/EBOOT.PBP"
cargo run --release -q -p jane-cli -- bake --target psp --out "$stage/bake"
# The county's console chunks baked (PLAN-PSP-SPEED.md §5, about 95 MB, never committed): the county
# is painted (a few seconds) and the pack baked again (about a minute) only when the painter's output
# or the codec is not the one it holds. The PSP's loader reads it beside the program (P1c). lz4, not
# zstd: on the owner's PSP a chunk's albedo decodes in 2.9 ms against zstd's 15 ms (bench, 2026-10-21).
cargo run --release -q -p jane-cli -- bake-chunks --if-stale --codec lz4 --out "$stage/bake/county.jck"

rm -rf "$stage/PSP" && mkdir -p "$stage/PSP/GAME/jane"
cp "$rel/psp-game.EBOOT.PBP" "$stage/PSP/GAME/jane/EBOOT.PBP"
python -I spikes/psp-game/memsize.py "$stage/PSP/GAME/jane/EBOOT.PBP"
cp "$stage/bake/jane-psp.jpk" "$stage/bake/present.jpt" "$stage/bake/jane-psp.jau" "$stage/bake/county.jck" assets/world/county.bp "$stage/PSP/GAME/jane/"

mkdir -p "$(dirname "$out")" && rm -f "$out"
python -I - "$stage" "$out" <<'EOF2'
import os, sys, zipfile
stage, out = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for root, _, files in os.walk(os.path.join(stage, "PSP")):
        for f in sorted(files):
            full = os.path.join(root, f)
            # The chunk pack is zstd already: stored, not deflated again.
            how = zipfile.ZIP_STORED if f.endswith(".jck") else zipfile.ZIP_DEFLATED
            z.write(full, os.path.relpath(full, stage).replace(os.sep, "/"), compress_type=how)
EOF2

python -I - "$out" "$rel/psp-game.prx" <<'EOF'
import sys, zipfile
eboot = zipfile.ZipFile(sys.argv[1]).read("PSP/GAME/jane/EBOOT.PBP")
prx = open(sys.argv[2], "rb").read()
if prx not in eboot:
    sys.exit("psp-dist: the zipped EBOOT does not hold the fresh build")
print(f"psp-dist: {sys.argv[1]} holds the fresh build ({len(eboot)} B EBOOT)")
EOF
