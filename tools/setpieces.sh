#!/bin/sh
# Every room of every generated dungeon, one frame each (DUNGEONS.md §2.10, the set pieces):
# `tools/setpieces.sh [dir] [backend]` renders them with `jane sheet scene --at ZONE:NODE`
# (she arrives in the middle of the room), wgpu by default; `all` renders soft and wgpu into
# dir/soft and dir/wgpu. Render before and after a pass and lay them side by side with
# `tools/ad-contact.py OUT before=DIR after=DIR`.
set -e
out=${1:-sheets/setpieces/now}
be=${2:-wgpu}
# JANE names another build to render the same set with (a before build, say).
jane=${JANE:-./target/release/jane}
if [ "$be" = all ]; then
  for b in soft wgpu; do "$0" "$out/$b" "$b"; done
  exit 0
fi
mkdir -p "$out"
room() {
  zone=$1
  shift
  for node in "$@"; do
    $jane sheet scene --backend "$be" --ticks 300 --at "$zone:$node" --out "$out/$zone-$node.png" > /dev/null
    echo "$out/$zone-$node.png"
  done
}
room mine entry plate store guard core firstaid office gallery vault arena nook cage
room museum entry atrium history toilets cloakroom maintenance arts science stores magic magic_case natural
room forest gate first_glade hut ring hearth rock runner island guarded hollow stone reward seed_tree lamp_glade
room pipes sump north_run junction chamber valve_house east_run west_run outfall silt screen
room factory yard loading lockers line_a time_office vent generator gen_shutter press_hall office assembly roller_door stores
room burial entry hall alcove rat lockin snake east garden vigil orchard glasshouse web nursery dark parade vault stair
room school boiler hall sick_bay corridor top_corridor woodwork chemistry botany physics domestic ice_house tower top_room staff_room lost_property
room library entry stacks nook shelf
