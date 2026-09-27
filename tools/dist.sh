#!/usr/bin/env bash
# The shippable game: one exe in dist/Jane/, zipped. target/ is cargo's build cache (1 GB+ of
# intermediate objects, SDL's CMake build and the like) and is never shipped.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p jane-app
rm -rf dist/Jane && mkdir -p dist/Jane
cp target/release/jane-app.exe dist/Jane/Jane.exe 2>/dev/null || cp target/release/jane-app dist/Jane/jane
cat > dist/Jane/README.txt <<'TXT'
Jane. Run Jane.exe. Saves and settings live in %APPDATA%\Jane (or beside the exe if a file
named "portable" sits next to it). Keys: WASD move, E use, Space/1-8 spells, Tab bag, M map,
Esc pause, F5/F9 save/load at a bed or fire, F12 screenshot.
TXT
(cd dist && rm -f Jane.zip && (command -v zip >/dev/null && zip -qr Jane.zip Jane || powershell -NoProfile -Command "Compress-Archive -Path Jane -DestinationPath Jane.zip -Force"))
du -sh dist/Jane dist/Jane.zip
