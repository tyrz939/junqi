#!/usr/bin/env bash
# The shippable PSP build (PORT.md §13.12): the EBOOT, the art pack, the presenter tables and
# the audio module in PSP/GAME/jane/, zipped. cargo-psp names its output after the crate once
# Psp.toml sets a title (psp-game.EBOOT.PBP); a stale EBOOT.PBP beside it once shipped an old
# build, so this takes the named file and checks that the fresh .prx is inside the zip.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-dist/TheBellAtNine-PSP.zip}"
stage="target/psp-dist"
rel="spikes/psp-game/target/mipsel-sony-psp/release"

(cd spikes/psp-game && cargo +nightly psp --release)
rm -f "$rel/EBOOT.PBP"
cargo run --release -q -p jane-cli -- bake --target psp --out "$stage/bake"

rm -rf "$stage/PSP" && mkdir -p "$stage/PSP/GAME/jane"
cp "$rel/psp-game.EBOOT.PBP" "$stage/PSP/GAME/jane/EBOOT.PBP"
python -I spikes/psp-game/memsize.py "$stage/PSP/GAME/jane/EBOOT.PBP"
cp "$stage/bake/jane-psp.jpk" "$stage/bake/present.jpt" "$stage/bake/jane-psp.jau" "$stage/PSP/GAME/jane/"

rm -f "$out"
(cd "$stage" && (command -v zip >/dev/null && zip -qr "../../$out" PSP || powershell -NoProfile -Command "Compress-Archive -Path PSP -DestinationPath ../../$out -Force"))

python -I - "$out" "$rel/psp-game.prx" <<'EOF'
import sys, zipfile
eboot = zipfile.ZipFile(sys.argv[1]).read("PSP/GAME/jane/EBOOT.PBP")
prx = open(sys.argv[2], "rb").read()
if prx not in eboot:
    sys.exit("psp-dist: the zipped EBOOT does not hold the fresh build")
print(f"psp-dist: {sys.argv[1]} holds the fresh build ({len(eboot)} B EBOOT)")
EOF
