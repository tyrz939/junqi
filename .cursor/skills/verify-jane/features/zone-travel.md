# Zone travel

Five zones: `county` (Castle), `house`, `cellar`, `mine`, `burial`. Doors carry `to: { zone, mark }`; you arrive at the named mark in front of the door you used. Travel is refused while carrying something.

## Sub-features

- `travel-locked-door` on a new run, E on Julie's door toasts "Julie's door is locked". The key is the dog's quest reward.
- `travel-house` with the key: E unlocks (the toast names the key), E again enters. `.jq-zone-name` reads Julie's House; the kitchen trigger completes "Julie's Kitchen".
- `travel-fog` interiors start black and reveal around Jane as she walks; leaving and returning keeps what was seen.
- `travel-cellar` either kitchen hatch leads to Julie's Cellar; the stairs lead back to the matching hatch.
- `travel-mouths` the mine mouth (north-east of the yard, lamp and sign) and the burial stair (walled graveyard, south-west) both enter and return to their own marks.
- `travel-persist` kill something, loot a chest; leave the zone and come back: the corpse, the open chest and any ground drops are as you left them.
- `travel-light` the house is warmly lit, the cellar dim, the mine dark with torches, the burial darker with blue torches that only light when an Icebolt passes them.

## Recipe

The honest path for `travel-house` is the first five minutes: dog, quest, yard skeleton (Space), dog again, door. `jane/test/sim.test.ts` plays exactly this headless; the browser run proves input, prompts ("[E] Talk", "[E] Unlock", "[E] Enter") and drawing.
