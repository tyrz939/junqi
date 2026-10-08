#!/usr/bin/env bash
# The shippable game: one exe in "dist/The Bell at Nine/", zipped as dist/TheBellAtNine.zip.
# target/ is cargo's build cache (1 GB+ of intermediate objects, SDL's CMake build and the like)
# and is never shipped. The save folder keeps the game's old name, %APPDATA%\Jane, so saves made
# before the rename still load (PORT.md §13.13).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p jane-app
name="The Bell at Nine"
rm -rf "dist/$name" dist/Jane dist/Jane.zip && mkdir -p "dist/$name"
cp target/release/jane-app.exe "dist/$name/$name.exe" 2>/dev/null || cp target/release/jane-app "dist/$name/the-bell-at-nine"
cat > "dist/$name/README.txt" <<'TXT'
The Bell at Nine. Run "The Bell at Nine.exe". Saves and settings live in %APPDATA%\Jane (or
beside the exe if a file named "portable" sits next to it). Keys: WASD move, Shift sprint,
Space hop, E use or talk, right-click walk there, Tab target, Ctrl free aim, 1-8 the bar,
I bags, J quests, K book, M map, Esc pause, F5/F9 save/load at a bed or fire, F12 screenshot.
A pad works too.
TXT
(cd dist && rm -f TheBellAtNine.zip && (command -v zip >/dev/null && zip -qr TheBellAtNine.zip "$name" || powershell -NoProfile -Command "Compress-Archive -Path '$name' -DestinationPath TheBellAtNine.zip -Force"))
du -sh "dist/$name" dist/TheBellAtNine.zip
