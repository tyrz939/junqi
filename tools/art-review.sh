#!/bin/sh
# The art director's whole-frame review set (PORT.md §7.1 step 6, ART.md §3.1): every family in
# one frame, at the hours and places the look is judged by. `tools/art-review.sh [dir] [backend]`
# renders the set on one backend (wgpu by default, T2, the owner's desk, the reference look);
# `all` renders it on soft, gl2 and wgpu into dir/soft, dir/gl2 and dir/wgpu, so the three tiers
# can be laid side by side (`tools/ad-contact.py OUT T0=dir/soft T1=dir/gl2 T2=dir/wgpu`).
# Re-run it before and after a pass and compare the directories.
set -e
out=${1:-sheets/art-review}
be=${2:-wgpu}
# JANE names another build to render the same set with (a before build, say).
jane=${JANE:-./target/release/jane}
mkdir -p "$out"
shot() {
  name=$1
  shift
  $jane sheet scene --backend "$be" "$@" --out "$out/$name.png" > /dev/null
  echo "$out/$name.png"
}
if [ "$be" = all ]; then
  for b in soft gl2 wgpu; do "$0" "$out/$b" "$b"; done
  exit 0
fi
shot town-1700 --ticks 300 --at town_square --hour 17
shot town-1840 --ticks 300 --at town_square --hour 18:40
shot town-2200 --ticks 300 --at town_square --hour 22
shot town-noon --ticks 300 --at town_square --hour 12
shot field-edge --ticks 300 --at hedge_stile_farm --hour 15
shot lake-dusk --ticks 300 --at reed_camp_gate --hour 18:40
shot wood-low-sun --ticks 300 --at car_wood --hour 17:30
shot reed-camp --ticks 300 --at reed_camp_gate --hour 11
shot works --ticks 300 --at canteen_gate --hour 14
shot yard-dusk --ticks 300 --at house_front --hour 18:40
shot town-rain-night --ticks 300 --at town_square --hour 22 --weather rain
shot mist-dawn --ticks 300 --at reed_camp_gate --hour 6 --weather mist
shot mine --ticks 300 --at mine:entry
shot burial --ticks 300 --at burial:entry
shot museum --ticks 300 --at museum:entry
