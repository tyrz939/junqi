#!/bin/sh
# The atmosphere's review frames (PRESENTATION.md §1.9, §6): each scene with every atmosphere row
# on (after) and off (before: the renderer as it was, the weather held but not drawn).
# `tools/atmos-review.sh [dir] [backend]`.
set -e
out=${1:-sheets/review}
be=${2:-wgpu}
jane=./target/release/jane
off="weather=off,fog=off,water=off,wet=off,god_rays=off,sky=off,max_particles=0"
mkdir -p "$out"
shot() {
  name=$1
  shift
  $jane sheet scene --backend "$be" "$@" --out "$out/$name-after.png" > /dev/null
  $jane sheet scene --backend "$be" "$@" --rows "$off" --out "$out/$name-before.png" > /dev/null
  echo "$out/$name-before.png $out/$name-after.png"
}
shot town-rain-night --ticks 600 --hour 22 --weather rain
shot water-mist-dawn --ticks 300 --hour 6 --at reed_camp_gate --weather mist
shot lake-dusk-school --ticks 300 --hour 18:40 --at reed_camp_gate
shot spell-night --ticks 600 --hour 22 --cast fireball:10
shot storm-reeds --ticks 600 --hour 21 --at reed_camp_gate --weather storm
shot wood-low-sun --ticks 600 --hour 17:30 --at car_wood
shot town-dusk --ticks 600 --hour 18:40
shot yard-fire-night --ticks 300 --hour 22 --at house_front
shot mine --ticks 300 --at mine:entry
shot burial --ticks 300 --at burial:entry
