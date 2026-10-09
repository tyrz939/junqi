# The Bell at Nine: the Map

> **A design for review (9 October 2026), not yet built.** The next worldgen: the county gains walkable high ground, cliffs, stairs, ramps, ladders, one-way ledges, bridges and walkways, and is laid out as regions joined only at a few authored gates. The reference is *A Link to the Past*. Story places, regions and quests stay; how they are laid out is regenerated. One worldgen for the PC and the PSP. Pair with `WORLDGEN.md` (the contract and the solver), `PLAN.md` §2 (the skeleton, density, threat), `NIGHT.md` §5 (whose R4 "places" this map must carry), `ARCHITECTURE.md` (the sim), `ART.md` §3.1 and `PRESENTATION.md` (the look), `PORT.md` §13 (the PSP budgets). Owner questions are in §10, each with a recommended default.
>
> Concept frames (a script's sketches, not the game): `progress/2026-10-09_68_map/county-levels-gates.png` (the county on its macro grid, levels shaded, gates marked) and `escarpment-closeup.png` (one region edge at cell scale in the ALttP projection), in the main checkout's `progress/` (gitignored); the script that drew them, `mapsketch.py`, is beside them.

---

## 1. The vision

### 1.1 What is wrong now

The county is a 2 km plain. Every place is reachable from every other in a straight line, so the roads are a suggestion, nothing is ever *above* you, nothing is seen before it is reached except the School, and a short cut is any line across a field. Danger is a field of numbers the player cannot see the shape of. It plays as "open in all directions", which is the owner's complaint, and it is why the county still reads as generated even where its places are authored.

### 1.2 What ALttP does, and what Jane takes

| ALttP | Jane takes it as |
| --- | --- |
| **The land has levels.** Plateaus, cliffs and terraces, two or three steps high, every face seen from the south | Four levels (§2), faces two cells tall a level, painted in the solid cells at a plateau's south edge with no draw offset: the sim stays a flat grid |
| **Routes are funnelled.** You cannot climb a cliff; you find the stair, the ramp or the cave | A region is open inside and walled at its edge; it joins its neighbours only at **gates** (§4.3): a ramp road up a cutting, a stone stair, a bridge, an underpass, a Company gate |
| **You see it before you can reach it.** The heart piece on the ledge, the cave mouth across the river | Every plateau with something on it is seen from a road below it; the way up is round the back. Archers on rises (§3.4) |
| **Ledges are one-way, and they point home.** Hop off a ledge and the long walk out becomes a short walk back | One-way ledges as shortcuts toward hubs and fires (§4.5): the long way out, the short way home |
| **Landmarks.** Death Mountain is always north; Hyrule Castle is the middle | The School on the crown, the highest ground, seen from the station (as built); the viaduct, the pithead, the steeple and the waterfalls as mid landmarks (§4.6) |
| **Bridges and layers.** Walk over a bridge, then under it | Spans (§2.5): the Castle viaduct over the towpath, a footbridge over the railway cutting, the pipe walkways over the slag |
| **Verb gates re-open the map** | Already `PLAN.md` §2.6; here they gain geography: the collapsed footbridge (Repair), the rockfall in a pass (Explosion), vines up a face (Grow), the Company lift (Electric) |

**Left alone:** ALttP's dark world, its screen-by-screen scrolling, its hookshot and its swimming. Feet still never cross water.

### 1.3 The tenets (each one a test in §9)

1. **Open inside, walled between.** A region is a handful of open districts; it touches another region only at 3 to 4 gates.
2. **Seen, then reached.** Anything on a plateau is in sight of a road for at least a screen before its way up is found.
3. **Loops, not dead ends.** Every district is on a loop; every ledge is a shortcut whose walk round is longer.
4. **Never a pit.** Wherever she can get to, she can get back (§4.7): one-way never means stuck.
5. **The land reads at a glance.** A face is lit on its lip and dark at its foot; a stair is a stair; a ledge looks hoppable from above and unclimbable from below.
6. **The walk stays the length it was.** The county shrinks (§4.9) so that funnelled routes take the time the open ones did.

### 1.4 The first hour, walked on the new map

Times as `EXPERIENCE.md` §2; every check there stands, with distances by route rather than metres (§8).

- **0:00 · The platform.** Castle Halt on the west edge, on the Lowfields' terrace (level 1). The railway runs in a cutting below the platform's far side; a footbridge crosses it (a span: she can walk over the line, and the line runs under her). North across the fields the escarpment's long brown face, and above it the hill, and on the hill one lit window. The terrace runs east; the station road goes with it.
- **0:45 · The lit road.** The road keeps to the terrace. To the north a low rise, the Long Hedge along its lip, crows over the Top Field on top (threat 3, a plateau pocket: she sees the scarecrows up there and there is no way up from the road). The road dips by a short ramp to a brook (level 0) and a plank bridge, climbs again past the well, the signpost, the overturned cart. Near Julie's gate a **ledge** drops from the Top Field onto the road: from below it is a turf lip she cannot climb, which teaches what a ledge is before she can use one.
- **2:00 · Julie's gate and stoop.** Unchanged: the yard is flat, fenced, a haven. Behind the house the ground falls away a level to Sallow Bottom; a stile at the back of the yard opens on a stair down (the dog's "rats under the house" are under the house, not down there).
- **The night, the cellar, the first rest.** Unchanged. The station road's lamps hold along the terrace; from the stoop at night she can see the wrong-coloured lamps on the Cutting's ramp, up on the escarpment, coming on at nine.
- **22:00 · Through Castle.** Castle stands on the terrace where it meets the river valley. Its east side ends in a face down to the quay (level 0), reached by the Quay Steps. The square, the fire, the Arms as now. North of the square the escarpment rises over the roofs: the Castle road climbs it by **the Cutting** (a ramp road through a cleft, the main gate to the Works), and Church Lane goes straight up by **the Church steps** to the graveyard. East, the **Castle viaduct** carries the Museum road across the valley at the terrace's level, and the towpath runs under it beside the river.
- **26:00 · The mine road.** South out of Castle, down to the brook, then up the **foothill ramp** to level 2. The carter's cart stands at the ramp's foot ("Horse would not go past the quarry turning": the horse would not take the climb). The quarry turning at the ramp's top; **the Quarry Steps** go up again to the quarry's benches, a POI on a terrace of its own. The mine mouth is a door in the foothills' face, framed in timber, ALttP's cave.
- **After the mine.** From the mouth a ledge drops off the foothills onto the Lowfields' terrace a minute above Julie's gate. The walk out was five to seven minutes; the way home is one hop and two minutes. This is the first ledge she *uses*, and it is placed so the yard's kept lamp is on screen below it.

---

## 2. The elevation model

### 2.1 The projection: ALttP's, which the engine already uses

The sim stays a 2D grid. **A plateau's top is drawn at its own cells, with no offset**; its south face is drawn up into the solid cells along its south edge, which is exactly the existing rule that a wall's face is painted up into its solid cells (`ART.md` §2.6, `PRESENTATION.md` "Behind the terrain"). So collision needs no level logic at all: the face cells are solid, the plateau is walkable, and the only ways between levels are the walkable join cells (stairs, ramps, ladders) and the one-way ledges. Height matters to sight, shots, fights, the AI, the draw order and the light, and nothing else.

| Number | Value | Why |
| --- | --- | --- |
| Levels | **4**: 0 valley, water, marsh; 1 terrace (most ground); 2 upland, foothills, the Works' plateau; 3 the crown | ALttP's overworld uses three to four. More is drawn as stacked faces, never needed |
| A level's height | **40 true px**, drawn `rows_up(40) = 32` rows: **two cells of face** at 16 canvas px a cell | About a person tall: a face reads as a cliff, not a kerb; a two-level face (the crown) is four cells |
| Face cells | The upper ground's last two rows above a south drop (four for two levels), solid, tile `Cliff`, level = the upper level | The face belongs to the plateau, so its cells carry the upper level in sight and light |
| Rim cells | One solid `Cliff` cell on a plateau's east, west and north edges | Seen edge-on: a sliver of face on the sides, a lit lip and a dark drop line on the north |
| Shortest face run | 6 cells; no plateau narrower than 8 cells | No 1-cell spikes: the land reads as authored |

### 2.2 How it is stored

- **`Blueprint.level: Option<Plane>`**, a byte a cell packed by `jane_core::plane::Plane` (16-cell chunks, palette of 1, 2, 4 or 16 values). Most chunks hold one level (0 bits a cell, the 4-byte descriptor only); an edge chunk holds two (1 bit a cell, 34 bytes). On a 1,536-square county (§4.9): 9,216 chunks, about a quarter on an edge: **about 120 KB**, against the tiles plane's megabytes. `None` means flat at level 1, and a blueprint with `None` hashes as today's, so every interior and every dungeon that gains no height moves no hash.
- **Levels never change in play.** Nothing in the sim writes the plane. What changes (a repaired footbridge, Grow's vines, Company Row's steps at night) is a tile delta or a span's state, as today. So the level plane is **never saved**: it is the seed's, like the terrain (`ARCHITECTURE.md` §10, "terrain never saved").
- **New tiles** (appended ids, nothing renumbered): `Stair` (walkable, `F_NOPUSH`), `Ladder` (walkable, `F_NOPUSH`, half speed, no swing or cast on it), `LedgeN`, `LedgeE`, `LedgeS`, `LedgeW` (solid; §2.4), `Waterfall` (`F_SOLID | F_WATER`, a face cell with a river down it). A ramp is not a tile: it is ordinary ground (road, dirt) whose cells step from level L to L+1 across its length, its sides bounded by face cells. The flags byte is full (eight bits used); none of this needs a new flag.
- **Each tile row gains `top: u8`**, its height over its own ground in **rungs** (a quarter level, 10 px): open ground and water 0, fence and low wall 1, bush 2, hedge 5, tree 10, house wall, roof and dungeon walls 12, `Cliff` 0 (its height is its level). A prop with `F_PROP_LOS` counts 12. A rule holds the table honest: **every tile with `F_BLOCK_LOS` has `top > 2` and every tile without has `top <= 2`**, which is what makes the new sight rule equal the old one on flat ground (§3.2).

### 2.3 Joins: stairs, ramps, ladders

| Join | Shape | Who | Where |
| --- | --- | --- | --- |
| **Stair** | 3 to 5 cells wide, through the face (2 cells a level), a landing cell at each end | Feet; pushables refuse it (`F_NOPUSH`) | Footpaths, churchyards, quays, the quarry, Company Row. A stair carries no road |
| **Ramp** | 4 to 6 wide, 6 to 10 cells a level along its run, face cells on both sides (a cutting) or one (a shelf on a face) | Everything, carts in story | Roads only climb by ramps: the Cutting, the foothill ramp, the crown's switchback |
| **Ladder** | 1 cell wide, through a face | Feet, slowly; no fighting on it | Rare: a well shaft, the quarry, the Works' gantries; Grow's vines are a ladder that appears |

A stair or ramp cell's level is the level of the ground it leads to from that cell's half of the run (the upper half upper, the lower half lower), so a unit's level changes once, mid-run, and nothing reads a fraction.

### 2.4 Ledges (one-way)

A ledge is a run of face cells (`LedgeS` facing south, and so on) drawn as a turf lip over a short drop instead of rock. It is solid. A walking unit on the upper side that moves into a ledge cell **in the ledge's direction** hops: a `Hop { from, to, t }` motion of about 0.4 s carries it over the face cells to the first walkable cell below (the landing strip, at least 2 cells deep, which the generator guarantees), with the art's arc and a dust puff. No damage, no control, no swing or cast mid-hop; she can be hit. Into a ledge from any other side, or from below, it is a wall. Knocked into a ledge in its direction, a unit falls over it (ALttP's push off a ledge): a tactic, and the solver ignores it.

Pushables pushed into a ledge in its direction drop over it the same way (a barrel off a ledge onto a plate below is a fine puzzle); carried rocks thrown over one land below.

### 2.5 Bridges, walkways and layers: spans

Two kinds of crossing, and only the second is layered:

1. **A bridge over water or a drop that nothing walks under** (the plank bridges, the river bridges where the valley floor is river, a footbridge over a chasm). One walkable surface: ordinary walkable cells at the deck's level. No layers. This is most bridges.
2. **A span over walkable ground**: the Castle viaduct over the towpath, the station footbridge over the railway cutting, the pipe walkways over the slag yard, the underpass arches through the railway embankment. **Two walkable surfaces on one cell.**

A span is a row in the blueprint: `Span { rect, axis, deck_level, ends: [CellIx; 2], state }`, a straight run 2 to 4 cells wide and at most 40 long, 10 to 30 in a county. Cells under it keep their own tile and level (the road, the towpath, the rails). The deck is a second layer:

- **Units carry a layer bit** (`Unit.on_span: Option<SpanIx>`), set when a unit steps from a deck end onto the span and cleared when it steps off an end. Off a span the bit is always clear, so everything that is not near a span pays nothing.
- **Collision:** on the deck a unit moves along the axis only (the deck's parapets are walls to it); under, the ground's own tiles rule and the deck's piers (its end cells' columns) are solid to ground feet. Hops never start or land on a deck.
- **Occupancy:** the sparse `occ` lookup keys a deck cell as `cell | DECK_BIT` (the top bit of `CellIx`), so a unit on the viaduct and one on the towpath under it never block each other.
- **Sight and shots:** a unit on the deck is at `deck_level`; a unit under is at its ground's level. The ordinary height rule (§3.2) then applies, with the deck's own cells counted as solid ground at `deck_level` for anything under it (you cannot see up through the deck; from the deck you see over the parapet and down past it).
- **Melee:** never across layers.
- **Lookup:** spans bucketed by 16-cell block in a sorted `Vec<(block, SpanIx)>`; a move or a sight line checks a span only when its block has one.
- **State:** a span may be `broken` (the collapsed footbridge, a verb gate) and become whole by a consequence; that bit is world state, saved and hashed. A night barrier on a span end is an ordinary prop.

### 2.6 Water and height

Water lies on one level per body: rivers in the valley (level 0), the lake and the marsh at 0, a tarn on the Works' plateau at 2. Where a river crosses a level step it falls: `Waterfall` cells in the face, a landmark with its own sound bed and mist (the river leaving the Works by the slag cliffs; a brook off the foothills). Feet never cross water, bolts do (as built). No river runs uphill: the generator orders river cells by non-increasing level along their flow.

---

## 3. The sim

### 3.1 Movement and collision

Unchanged in its core: the 6 x 6 body box, axis-separated sliding, solid cells by flags. Additions: the ledge hop (§2.4), the ladder's half speed and its no-fighting rule (a status the move step sets while the feet are on `Ladder`), the deck's axis rule and layer bit (§2.5). The unit's **level** is derived each move from the cell under its feet (or its span), kept in the unit as a `u8` because sight, shots and the AI read it every tick; it is state, saved and hashed (it is a function of position, so it can never disagree with a rebuild).

### 3.2 Sight across height

One rule, integer and symmetric, replacing the flags test inside the same supercover walk (`jane_sim::los`):

```
ground(c) = level(c) * 4          (rungs; a level is 4)
height_at_end = ground + EYE       (EYE = 2 rungs, half a person)
the line runs from a's height to b's, linearly in the walk's step count;
a visited cell blocks when ground(c) + top(c) > the line's height at that cell
```

What it means to a player, and why it is the right rule for this game:

- **On flat ground nothing changes.** Both ends at one level, the line sits at `ground + 2`; a cell blocks iff `top > 2`, which by the table's rule (§2.2) is iff it has `F_BLOCK_LOS`. A test proves the two rules equal over every flat zone and a soak of random segments, so every existing sight test, golden and replay stays byte-identical until a level differs.
- **From below you see up to the edge.** A unit at the foot of a one-level face, `a` cells out, sees a unit on the plateau `b` cells back from the face iff `b < a`. *Further from the edge than they are from the foot, and you are hidden.* That is the whole rule a player needs; "step up to the edge to shoot down, step back to hide" falls out of it.
- **High ground sees over low walls.** From a rise, a hedge or a fence between two lower places no longer blocks: the line passes over its top. Archers on rises see over the hedges the road is lined with.
- **Cliffs block.** Two units on either side of a plateau they both stand below never see each other through it.

Cost: one read of the level plane a visited cell, beside the flags read already made. A chunk with one level answers from its descriptor with no index read. **Fast path:** when both ends are in single-level chunks of the same level and the segment's bounding box touches no edge chunk (one check against a per-chunk "edge" bit the runtime derives on load, 9,216 bits, 1.2 KB), the old flags test runs as is.

### 3.3 Shots across height

- **Targeted shots** (a target held, `PLAY-PLAN.md`'s hybrid targeting): the bolt flies the 3D line of §3.2 from caster to target and stops at the first cell that blocks it. A cast at a target needs that line clear when it starts, as a cast needs sight today. So she **can shoot up** at a unit standing near an edge, and cannot at one standing back.
- **Free-aimed shots** follow the ground: the bolt's height is its cell's ground plus `EYE`. Over lower ground it **falls** with the ground (a shot off a cliff top carries on into the field below); into higher ground it **stops** at the face. *Shots fall off cliffs and never climb them.* A free-aimed bolt hits only units on its own level at the cell it is in.
- **Ground spells** (grounds, splashes) touch only units at the level of the cell they land on, or the deck if they land on a span.
- **Melee** reaches only a unit at its own level (or across a join, where the two cells are a stair or ramp step apart). A face, a ledge or a deck between two units is never reached across.

### 3.4 The AI: enemies on ledges, and the stair foot

- **Perch rows.** A hostile row may say `"holds": "level"`: it never leaves its plateau (its path search treats joins and ledges as walls). Ranged perch rows (the archer on the rise, the Works' spitters on a gantry) shoot down at her while she is in sight, and are reached only by finding their stair. Melee perch rows guard a plateau's reward.
- **Unreachable targets.** A chaser whose path to her costs more than three times the straight distance plus 40 cells (she is above it, the stair is round the back) does not wall-hug: it **holds** at the foot below her for up to 6 s, then **evades** home whole, as a leash does (`PLAN.md` §2.6, `CombatState::Evade`). So shooting things from a cliff they cannot climb earns nothing: they go home healed, as in WoW.
- **Hopping after her.** A chaser may hop a ledge she hopped (ledges are one-way edges in its path) if the landing is inside its leash. Bosses, big rows and perch rows never hop.
- **Leash and threat rings by route.** Leash distance stays as built (straight from home; a hostile pulled down a ledge evades if the walk home is past its leash). Dungeon rings in the threat field are laid by route distance over the gate graph, not by radius (§7.2), so a mouth on a plateau does not raise threat on the road under its face.
- **Aggro** needs the §3.2 sight, as now; at night its reach grows as built.

### 3.5 Paths

The windowed A* (256 x 256, 10/14 costs, budget 6,000) gains:

1. **Ledge edges.** When a neighbour is a solid cell whose tile is a ledge entered in its direction, the search adds a jump edge to its landing cell (cost: the hop's length x 10 plus 20). One-way: there is no reverse edge. The tile is read only for solid neighbours, which are a small share of expansions.
2. **Deck nodes.** A span's deck cells are extra nodes indexed after the window's cells (at most 2,048 a window; 24 KB more scratch), joined to each other along the axis and to the span's end cells. Searches that never touch a span never touch them.
3. **Ladder cost** 20 a cell; stairs and ramps cost as ground.

The bots' coarse planner (`jane-bot::coarse`) joins its pieces by the same directed ledge links and the span ends, so a bot plans a hop as a road.

### 3.6 Co-op

The sim is lockstep and shared, so a hop, a layer bit and a level are the same at every seat. A ledge can split the party; tenet 4 (§1.3) and a test (§9.1) keep the walk round from the landing back to the top under 90 s for every ledge, so nobody waits long. The party penalty, the freeze-when-alone rule and one heroine are unchanged. Two seats on the viaduct and the towpath see each other drawn through the deck on the checker (`Tint::Seen`, as built).

### 3.7 Cost on the PSP

| Item | RAM | CPU |
| --- | ---: | --- |
| Level plane (1,536-square county) | ~120 KB, part of the blueprint | O(1) read; fast path skips it on flat ground |
| Per-chunk edge bits (runtime) | 1.2 KB | read once a sight line |
| Spans (30) and their block index | < 2 KB | checked only in blocks that have one |
| Unit level and span bit | 2 B a unit (~6 KB for 3,000) | set in `move_unit` |
| A* deck nodes | 24 KB scratch | only when a span is in the window |
| Tile `top` table | 64 B static | |
| **The county shrinking to 1,536** (§4.9) | **saves about 40 % of every county plane**: tiles, paint, the flags pages, the solver's bits | and about 40 % of the build's 33 s on a PSP-1000 |

Net: the county's height costs less than the cells the smaller county gives back, so **sim resident stays under 6 MB and the build peak under 9 MB** with room to spare (held by `tests/heap_budget.rs`, §9.1).

---

## 4. Worldgen

### 4.1 The order: graph first, then terrain carved to it

ALttP's overworld reads authored because its routes were decided first and the land shaped round them. The new skeleton does the same, coarse to fine, every layer a pure function of the seed and the layers above, every stage on its own dice (`steps::dice`, as built):

| Layer | Grid | What it decides |
| --- | --- | --- |
| **K1 Regions** | macro (16 cells) | The three regions as now (Works north on the hill, the Waters east beyond the river, the Lowfields south-west), but their borders are now *walls*: the escarpment, the river valley, the slag cliffs |
| **K2 Districts** | macro | Each region cut into 4 to 5 districts (13 to 15 in all) by a seeded, relaxed Voronoi on macro cells; each district a base level from its region's table (§4.2) |
| **K3 The gate graph** | graph | Districts as nodes; every border classified **open** (same level: hedge lines, wood edges with gaps), **joined** (one level apart: stairs, ramps, ledges), or **wall** (a region border: gates only). A spanning tree plus loops; gates typed (§4.3); ledges laid toward hubs (§4.5); verb gates placed (`PLAN.md` §2.6) |
| **K4 Sites and areas** | macro | The story's places by `sites.json` (now with level and district rules, §4.8), the named patches by `areas.json`, by route over the gate graph, not by line |
| **K5 Roads** | macro, then cells | Routed over the gate graph's ramps and bridges only (a road never takes a stair); the railway on its own grade, through cuttings and over embankments, crossing level changes only by spans |
| **K6 Levels at cell scale** | cells, by row band | District borders turned into faces with integer noise (±4 cells), opened and closed to the shortest-run rules (§2.1), written straight into the level `Plane` a band at a time (`Plane::pack_bands`, no full byte grid) |
| **K7 Joins carved** | cells | Stairs, ramps, ladders, ledges and spans cut where K3 put them; their landing strips cleared and claimed |
| **K8 The county stages** | cells | Today's `STAGES` (`land` to `drop_unreachable`) run on the new ground, each made level-aware where it places things (§4.8) |
| **K9 Validate** | cells | The solver with directed edges and spans, the no-pit proof, the night stages, the density budget, the sightlines (§4.7) |

### 4.2 How elevation reads authored

By region (the concept frame shows one seed):

| Region | Levels | Shapes |
| --- | --- | --- |
| **Lowfields** | Mostly 1. One or two **rises** at 2 (the Top Field, a wooded knoll), one **hollow** at 0 (Sallow Bottom), the **foothills** at 2 along the south edge (the mine in their face), brooks at 0 in shallow valleys | Wide terraces, low faces, the friendliest region: many joins, short ledges |
| **Waters** | 1 terraces stepping down to 0: the river valley, the marsh, the lake, the drowned lanes | Long low faces along the river, reed beds at their feet, islands at 1 in the marsh reached by spans |
| **Works** | 2 (the plateau), 3 (**the crown**, the School alone on it), sunken 1 (the slag mere, the Burial's cutting) | Hard faces: slag banks and brick retaining walls, the crown's four-cell cliffs, one switchback ramp to the top |

Rules that make it read made rather than noisy: a face is never straighter than 12 cells without a jog, and never jaggier than one jog in 4; faces follow district borders, so every face *means* something (a region's edge, a rise's rim); plateaus are at least 8 cells across; a rise always has a reason to go up it (§4.6).

### 4.3 Gates between regions

Each region border has **3 to 4 gates**, at least two that a road takes. The default set (named per seed by the name rows):

| Border | Gates (types) |
| --- | --- |
| Lowfields to Works (the escarpment) | **The Cutting** (ramp road, the main way, the Castle road); **the Church steps** (stair, from Church Lane to the graveyard); **the rail arch** (an underpass span through the railway embankment, west); plus two **ledges down** from the Works toward Castle and a **Grow** vine ladder |
| Lowfields to Waters (the river valley) | **The Castle viaduct** (a span: the Museum road over the valley, the towpath under); **the stepping stones** (stair down, stones across, stair up, south); **the footbridge** (a broken span: Repair, the shortcut home); the railway trestle (walkable as built) |
| Waters to Works (the slag cliffs) | **The Company gate** (a ramp behind a gate in the Works' fence, open by day; one of N3's barriers may close it, §7.1); **the pipe walkway** (a span over the slag yard); a **ledge** down into the Waters by Butterfly Forest |

Inside a region, every district joins its neighbours by **at least two** openings or joins (open borders by gaps at least 6 cells wide, 3 for a footpath), so a region stays open to walk round.

### 4.4 Chokepoints

Gates are where fights are decided (WoW's pull at the pass). A gate is at least 4 cells wide (a road's ramp 6) so a party of four fits; never a 1-cell door in the open county (a ladder is the only 1-wide join, and never on a required route). The approach to each gate is clear for 8 cells (no scatter), and each gate has a lamp or a fire within its district by the existing fire rule, so the night's choke is a lit one or a deliberate dark one (`NIGHT.md` §5.4).

### 4.5 Loops and one-way shortcuts

- **Every district is on a loop** of the gate graph (no district is a dead end except a pocket by design: a plateau with one stair is a reward, not a district).
- **Ledges point home.** For each hub and each kept fire, at least one ledge lands within 60 cells (about 8 s) of it from a district further out. The test of a ledge: walking from its landing back to its top takes at least 40 cells more than the hop (else it is pointless) and at most 90 s (else it splits a party for too long).
- **Verb gates open shortcuts**, never the only way: the footbridge (Repair) cuts the walk from the Museum to Castle by half; the Explosion rockfall opens a second way up the foothills; Grow's vines make a ladder up the escarpment by the allotments; the Company lift (Electric) joins the Works' plateau to the crown's foot.

### 4.6 Landmarks and sightlines

The view is 40 x 22.5 cells. In a top-down game "seen from afar" is mostly the far-landmark layer and what stands at a screen's edge, so:

- **The School** keeps the far-landmark silhouette (as built); it stands on level 3, the highest ground, the one thing north of every view.
- **Mid landmarks**, each placed on high ground or at a gate and each in the far layer when it is just off-screen in its direction: the Castle viaduct, the pithead winding gear on the foothills, St Anne's steeple on the escarpment's lip, the waterfalls, the Works' chimneys on the plateau's edge.
- **Seen, then reached** (tenet 2): every plateau POI (a jar on a rise, a shrine on the quarry bench, an archer camp) has a road cell within 12 cells of its edge from which §3.2's sight sees the POI's mark; its join is at least 30 cells by route from that road cell. A check in K9.
- **Overlooks** (optional, §10 Q6): at a few authored plateau edges a bench or a rail where standing still eases *her own camera* a third of a screen out over the drop. Presentation only, per seat; the sim never knows.

### 4.7 The solver with one-way edges: reachable and softlock-free

The solver's flood stays the fast word-parallel fill (`jane_core::search::fill_words`); ledges and spans are **portals** handled like gates are now:

1. Fill from the entrance with ledge cells solid and deck cells absent.
2. For every ledge whose top cell is reached and whose landing is not, seed the landing; for every span (whole) with an end reached, seed the other end and mark the deck. Fill again. Repeat until nothing changes (a few hundred portals; the fill already runs to a fixpoint for keys and plates).
3. **No pits:** fill **backwards** from the entrance (ledges reversed: the landing reaches the top) to get the set from which the entrance can be reached, and require **forward ⊆ backward** in every state layer (every key and verb combination the solver walks) and at every night stage. Wherever she can stand, she can walk back to the station. A ledge into a place with no way out fails the seed.
4. Everything the solver proves today still holds (every mark, required unit and prop, every gate opens, C1 to C13 in dungeons), now on directed ground.
5. **Night per stage** (`NIGHT.md` §5.6): the same with that stage's barriers solid and its tile deltas (Company Row, the causeway) applied.

A seed that fails is re-rolled (never shown; `WORLDGEN.md`'s rule).

### 4.8 Story places and quest anchors

Every site, area, anchor and POI row keeps its id and its story. Rows gain level and district rules beside their distance rules:

| Site | New rule |
| --- | --- |
| `station` | West edge, level 1, the railway in a cutting beside it with a footbridge span |
| `julie_house` | Level 1, on the station road, a face behind the yard (Sallow Bottom below); the first walk never crosses a level |
| `town` (Castle) | Level 1 where the terrace meets the river valley; the escarpment north of it within 300 cells; the Quay Steps east |
| `farm` | Level 1, its fields on a terrace, a brook at 0 |
| `gold_mine` | Its door in the foothills' face (level 2 over 1), reached by the foothill ramp; a ledge from its district toward Julie's |
| `museum` | Across the viaduct, level 1 on the Waters' terrace |
| `reed_camp`, `lake_statue` | Level 0, on the bank |
| `library` | Level 1, a ruin on a terrace above the marsh; its back to a face |
| `butterfly_forest` | A wooded hollow at 0 in the Waters, walled by faces, one ramp in, a ledge out |
| `graveyard` | On the escarpment's lip (level 2), St Anne's steeple seen from Castle |
| `burial` | In the sunken cutting (level 1 inside 2) below the graveyard, reached by a stair down: "not seen from any road" becomes a fact of the ground |
| `factory`, `canteen` | Level 2, the Works' plateau |
| `school` | Level 3, the crown, the only site on it; the switchback ramp the only road up |
| `car_wood` | Level 1 or 0, in a wood, off the road |

**Chunks gain a `levels` layer** (a digit a cell, `.` for "the site's level") so an authored place can straddle a face: the town (the Quay Steps, Company Row's steps), the mine mouth (its door in a face), the graveyard and the burial stair (lip and cutting). Chunks without one sit on one level, and the stamper refuses a box that crosses a face it does not draw. The lint (`jane check`) gains: a `levels` layer's faces are well formed, its joins connect, its gates land on ground of the level the layer says.

**Placements, stories, POIs, scatter and wildlife** (the K8 stages): a placed thing's footprint must lie on one level and not on a face, join or landing strip; a story's "near the road" means by route; the "something visible from the road every 20 to 30 s" rule is measured with §3.2 sight. **Density carries over** per region (`PLAN.md` §2.4) and is now also checked per district (each district at least a sixth of its region's POIs), so no district is the empty one.

### 4.9 County size

**Recommendation: 1,536 x 1,536 cells** (96 x 96 macro cells), down from 2,000.

- Funnelled routes are longer than straight ones: with gates the road factor rises from about 1.25 to 1.5. At 1,536 the crossing by road is about 5 minutes, what it is now; at 2,000 it would be 6 to 7, which the owner already called too big once (2026-09-24).
- The same density budget on 59 % of the area is the density push the owner asked for (2026-09-23) without inventing content.
- It pays for the height on the PSP with room over (§3.7), and cuts the console build by about 40 %.
- Distance rules in `sites.json`, `anchors.json` and `areas.json` become **route-time bands** (seconds of walking at 60 px/s), so the first walk is still 1 to 2 minutes and the mine road still 5 to 7, whatever the land does.

---

## 5. Dungeons

**Yes, but after the county, and only three.** The mission graphs, C1 to C13 and the generator's steps stand. A dungeon's `.room` template may gain the same `levels` layer as a chunk, and the same tiles (stairs, ledges, a span), with the same sim (§3), no new rule.

| Dungeon | Height | What it is for |
| --- | --- | --- |
| **The Gold Mine** | Galleries one level over the shaft hall; a ledge from the gallery to the hub | The `shortcut` edge C9 already demands, as a one-way drop (the ALttP dungeon shortcut). The broken steps (Repair) become a real stair through a face |
| **The Burial Chamber** | The sunken garden at a level below the halls; statues on ledges | The spitting statues as perch rows shooting down; the garden seen from above before it is reached |
| **The School** | Balconies over the hall; the bell tower's stair | Masters on the balconies (perch), the Timekeeper's climb |

The others stay flat. C9 learns that a ledge is a shortcut edge; C10 (seen from an earlier flood) reads §3.2 sight; a new check C14: **no pit in a dungeon** (§4.7's forward ⊆ backward, with lock-ins released). Dungeons do not turn at night (`NIGHT.md` §10.10), so nothing of §7 touches them.

---

## 6. Art and rendering

### 6.1 What gets painted (all procedural, to `ART.md` §3.1)

The terrain painter (`jane-art::terrain`) gains a `level(cx, cy)` on its `TileSource` and paints, by region style (`RegionStyle`, ramp swaps as built):

| Piece | Lowfields | Waters | Works |
| --- | --- | --- | --- |
| **South face** (2 cells a level) | Sandstone in courses, turf overhanging the lip, roots and ivy, scree and nettles at the foot | Mud and slate banks, stained by the waterline, reed at the foot | Slag banks and brick retaining walls, buttresses, rust weeps, drain spouts |
| **Side rim** (1 cell) | The face's edge-on sliver, shaded east, lit west | as the face | as the face |
| **North lip** | A lit turf edge and a dark drop line (what ALttP draws) | as Lowfields, wetter | a kerb of brick |
| **Stair** | Stone treads with worn middles, a rail on the steeper ones | Timber treads on piles | Iron treads, chequer plate |
| **Ramp** | A road cut through the face: its two cut walls, wheel ruts | a causeway of setts | slag road, a Company sign at the top |
| **Ledge** | A rounded turf lip with a short drop (reads "hop me" from above) | reed lip | a broken kerb |
| **Waterfall** | Animated (the water's shimmer list), white water at the foot, spray | | slag-stained |
| **Spans** | The viaduct in brick arches, the footbridges in timber, parapets | timber on piles | iron girders and the pipe walkways |

Faces carry the region's detail at the art bar: no flat bands, every course its own tone, the lit lip and the foot's AO deep enough to read at 1x (ART §3.1's "depth everywhere"). The critique loop runs on each piece, three passes at most a family a round (`NIGHT.md` §9.2's cap).

### 6.2 Draw order and occlusion

- **Plateaus and faces need nothing new.** In ALttP's projection a unit on a plateau is north of the face and a unit below is south of it; the y-sorted `Standing` list (feet rows) already draws them right, and a tall thing below the face (a tree, a lamp) overlaps the plateau's lip in front, as it should.
- **"Behind the terrain" gets a base.** The height layer becomes absolute (level x 40 + relief). `Foot` gains the unit's own ground height, and `Foot::hides` compares the terrain's px height **less that base**, so a unit on a plateau is not "behind" the ground it stands on, and is still behind a house on the plateau. `light_lift` likewise lifts a lantern by the drawn height *over its base* (`MAX_LIFT` stays 48, now relative).
- **Spans: one new depth.** `Depth::Deck` between `Standing` and `Canopy`: the ground-layer standing list, then the decks (each a strip the painter draws like a chunk), then the deck-layer units (a second, short standing list), then the `Tint::Seen` pass (as built) so a player or a hostile under a deck is seen through it on the checker. The `Frame` gains the depth; every tier draws it as sprites.
- **Shadows.** The faces cast through the existing path: the terrain's blocks come from the height layer, now from **steps** in it (a drop of more than `RELIEF` to the south or east). Long faces are merged into one block per chunk-edge run to keep the caster count where it is. The decks cast onto what is under them (a dark band under the viaduct: the towpath in shade at noon).

### 6.3 The PSP

- **No new VRAM pages.** Faces, stairs, ramps, ledges and waterfalls are painted into the chunks the painter already paints on the PSP (`T8` a chunk through its own CLUT, 30 to 140 colours; a face adds a rock ramp of about 10). A deck strip on screen borrows a slot from the existing chunk-slot pool (12 on the PSP) rather than a page of its own.
- **Shadows on C2.** At *Full*, the faces cast as every block does. At *Balanced* (30 fps, the default), the faces' sun shadow is a short fixed band at the foot painted with the chunk (AO, not a swept polygon), so the GE does no more work for a cliff than for a wall. At *Fast*, the band only.
- **The painter's cost.** A face is painted once per chunk, like a wall; the PSP chunk budget (`LAND_PER_TICK`) is unchanged.
- **Test:** the PSP tour (`tour.sh`) gains the Cutting, the viaduct with a seat under it and a seat on it, and the crown, and must hold 55 fps at Full as today; the resident art stays under 3 MB plus VRAM.

### 6.4 The map

The fogged map draws faces as hachures in the map ink, stairs and ramps as ticks, ledges as a chevron, spans as a parapet line, once seen (`fog_seen` as built). The minimap at 1 px per 4 cells tints by level. No markers: the map shows the land, not the way.

---

## 7. The night and the threat field on this map

### 7.1 NIGHT.md R4, built here

`NIGHT.md` R4 ("the places") is built **on the new county, after it is switched in** (§9), not on the old one: building it twice is the waste to avoid.

| Night place (`NIGHT.md` §5.1) | On the new map |
| --- | --- |
| **The Long Hedge footpath** (N1) | The Long Hedge runs along the lip of a Lowfields rise. By night the fallen limb is gone and the footpath runs the lip to a ledge down onto the station road near the gate: the night shift's way, a short cut, and it ends in a hop. Threat 2, unlit |
| **Company Row** (N2) | **STORY CHANGE (M1):** the bricked gap between two houses opens on **steps down** (stair tiles in the town chunk's night layer, the level plane unchanged: the cells were always at the lower terrace, walled by day) to a sunken lane of six terraced doors under the High Street. "Below the street, where the shift is kept." A dead end inside the town's fence, threat 0, as accepted |
| **The Arms' back room, the vestry, the canteen hatch** | Unchanged (interiors and props) |
| **The Drowned Lane causeway** (N3) | The Drowned Lane is a lane in a level-0 basin in the Waters with a face round three sides. By night the water drops a cell and a causeway (stepping tiles) crosses to Mother's Garden; the basin's only other way is the long ramp round. Julie's jar at the far end |
| **The Works' time clock** (N3) | By the Factory's gate, on the plateau |
| **Two Company barriers** (N3) | Now on **gates** (§4.3), which makes them mean something: two side gates of the region borders (never the Cutting, never the first walk, never a gate whose border would keep fewer than two open), closed ROAD CLOSED BY ORDER OF THE COMPANY. The per-stage solver proves every site reachable by another gate |
| **The shield ranks** | By route distance from Julie's gate over the new roads, as `NIGHT.md` §5.4 (N2 at 700 m, N3 at 400 m), now route-time bands |
| **The night clerk** (N2) | Walks the unlit **ramps** (the Cutting, the foothill ramp): from a ramp's top it sees over the hedges of the road below (§3.2) and rings its bell from above. Height as a night threat |
| **The N3 files** | One file walks the **towpath under the Castle viaduct** while she crosses over it: the night shift passing under her feet, seen through the deck on the checker, and not able to reach her up there |
| **Wrong lamps** (N4) | As designed; the Cutting's lamps go wrong first (they are the first seen from Julie's stoop, §1.4) |

The night's own rules stand: nothing on the spine is night-only, a kept fire within two minutes of every road cell, hubs and the first walk untouched. **Ledges at night:** night creatures hop after her by the §3.4 rule; perch rows of the night (none planned) would not.

### 7.2 The threat field on the new layout

`PLAN.md` §2.6's layers stand, with three changes:

1. **Sub-areas by district.** The 18 named patches are placed by district (each district at most two), so danger has a shape the land explains: the Top Field (3) is a rise, the Drowned Lane (4) a basin, Chapel Rise the escarpment's lip.
2. **Plateau pockets.** A plateau whose only join is one stair is a pocket at its district's threat +1, with its reward on top (a jar, a page, a chest), seen from the road (tenet 2). WoW's elite camp on the hill you can see from the road, and Zelda's heart piece on the ledge, in one place.
3. **Dungeon rings by route.** A ring is laid by route distance from the mouth over the gate graph, not by radius, so threat rises along the way in and not on the far side of a cliff.

The upgrade rule (phase N−1's upgrades reachable without crossing threat N) and the first walk at threat 1 are checked by the solver as now, on directed ground.

---

## 8. Migration

**What breaks, once each, in its own commit, the owner told** (as `NIGHT.md` §9.3 did for its bump):

| What | Why | How |
| --- | --- | --- |
| Every county blueprint and the world hash fixtures (`tests/fixtures/hashes-x86_64.txt`) | A new layout | Re-blessed once at the switch (R5). Interiors and flat dungeons do not move (their `level` is `None`) |
| `SAVE_VERSION` | The unit's level and span bit, the span state, the hop; and a save's county deltas are keyed by cells of a county that no longer exists | One bump in R2 (the fields), and the switch refuses saves from the old county with a plain message ("This save is from an older county"). Pre-release; §10 Q8 |
| The bot hash fixture (`tests/fixtures/bot-hash-x86_64.txt`) and the bots' story runs | New ground, new routes | Re-blessed at the switch; bots learn ledges and spans in R1 and R2 so they are ready |
| `EXPERIENCE.md` checks and the quest audits (`QUEST-TREE.md`) | Distances in metres become route-time bands | Rewritten with the switch; every `[L1:...]` check keeps its meaning |
| `sites.json`, `anchors.json`, `areas.json`, `paths.json`, the 16 chunks | Level and district rules, `levels` layers | Edited in R4; old fields kept until the switch so the old county still builds |
| The PSP | A zone digest and the console build change with the blueprint; the bake changes only for new looks (the span and stair props) | The cache keys on `content_hash` already (`ARCHITECTURE.md` §9); the pack re-baked in R3 |
| `NIGHT.md` R4 | Its places move | Built after the switch, on this map (§7.1) |

**Every round shippable.** The sim's height (R1, R2) is **inert on a flat county**: with `level: None` and no spans, sight equals the old rule (proved by test), paths add no edge, the hash and every golden stay byte-identical, so R1 and R2 merge into the live game and change nothing a player sees. The new layout is built as **`county::terraced`** beside the current builder, selected by one row in `data/tuning/county.json` (`"layout": "plain" | "terraced"`); the seed viewer and the tests run both until the switch (R5) flips the row, re-blesses the fixtures and bumps the save, in one reviewed commit. The old builder is deleted a round after that.

---

## 9. Implementation plan

Owners: **heavy** (Opus, involved work), **light** (Sonnet, mechanical), **grok** (a read-only review: determinism and softlock audits, as `grok-feedback.md`). Sizes S to XL. No more than four agents at once.

| Round | Work | Crates / data | Owner | Size |
| --- | --- | --- | --- | --- |
| **R0. Paper** | The macro skeleton alone (K1 to K5): districts, levels, the gate graph, sites by route; painted in the seed viewer (`jane seed` / viewer: levels, faces, gates, ledges, spans, route times) for 24 seeds. **The owner reviews the layout before a cell is carved** | `jane-world::skeleton`, `jane-cli` (viewer) | heavy | M |
| **R1. Height in the sim** | `Blueprint.level`, the new tiles and `top`, unit level, the hop, ledge edges in A*, the 2.5D sight and the shot rules, perch rows, hold-and-evade, `View::level` / `ledges`; a hand-built test zone (`terraces`, dev only) with every case; the bots' directed coarse links | `jane-core`, `jane-sim`, `jane-schema`, `jane-bot`, `data/units` | heavy | L |
| **R1g. Review** | Determinism of the hop and sight; flat equivalence; zero allocations per tick | read-only | grok | S |
| **R2. Spans** | `Span`, the layer bit, deck occupancy, deck nodes in A*, sight and shots across layers, the solver's portals, `SAVE_VERSION` bump | `jane-core`, `jane-sim`, `jane-world::solve`, `jane-bot` | heavy | M |
| **R3. The look** | Faces, rims, lips, stairs, ramps, ledges, waterfalls, decks for three regions; `Foot` base, absolute heights, step casters; `Depth::Deck` on every tier and C2; the PSP's Balanced band; the map's hachures; art review frames (`tools/art-review.sh`: the Cutting, the viaduct over and under, the crown, the quay) and the critique loop | `jane-art`, `jane-present`, `jane-render-*`, `jane-cli` (sheets, bake) | heavy (art), light (sheets, bake) | L |
| **R4a. Carve** | K6 and K7 at cell scale into `county::terraced`; chunk `levels` layers and their lint; sites' level rules | `jane-world::county`, `jane-schema`, `data/chunks`, `data/sites.json` | heavy | L |
| **R4b. Fill and prove** | The K8 stages level-aware (placements, stories, POIs, scatter, wildlife, perimeters, ways, cut-through), density per district, threat by district and by route, the solver's no-pit proof and per-stage night proof, sightline checks | `jane-world`, `data/*` | heavy | XL |
| **R4g. Review** | Softlock audit: the no-pit proof on 1,000 seeds, the ledge loop rules, the night stages | read-only | grok | S |
| **R5. The switch** | `layout: terraced`, county 1,536, fixtures re-blessed, old saves refused, `EXPERIENCE.md` checks in route time, bots' story runs on seeds 1 to 5, PSP build and tour, heap budgets | everything; `tests/fixtures` | heavy | M |
| **R6. The night's places** (= `NIGHT.md` R4) | §7.1 on the new map | as `NIGHT.md` R4 | heavy | L |
| **R7. Dungeon height** | The mine, then the Burial, then the School (§5); C14 | `jane-world::dungeon`, `data/rooms` | heavy | M each |
| **R8. Play** | Bot playthroughs, the density and threat audits, the owner's playtest, redlines | `jane-bot`, `progress/` | heavy, light, owner | M |

R0 runs now. R1 and R3's art spikes run in parallel after R0 is approved; R2 after R1; R4a after R0 and R1; R4b after R4a and R2; R5 when R3 and R4b are green. `NIGHT.md`'s R2, R3 and R5 are independent of all of this and continue.

### 9.1 Tests

- **Flat equivalence** (R1): over every existing zone and 100,000 random segments, the 2.5D sight equals the flags sight; every golden, replay tape and hash unchanged with `level: None`.
- **Determinism:** a replay across hops, ledge falls, deck crossings and a knock off a ledge hashes the same twice and on two seats in lockstep; `decode(save(s)) == s` with a unit mid-hop and one on a deck; `runtime_rebuild_is_invisible` on a span.
- **Sight and shots** (`terraces` zone): the `b < a` rule at a face; a hedge seen over from a rise and not from beside it; a free bolt falling off a cliff and stopping at a face; a targeted bolt up to an edge-stander and refused for one standing back; melee never across a face or a deck.
- **AI:** a perch row never leaves its plateau; a chaser below her holds, then evades whole; a chaser hops a ledge after her inside its leash and not outside it; bosses never hop.
- **Paths:** ledge edges are one-way; a path that needs a hop is found; deck nodes join only along the axis; budgets and the window unchanged.
- **Worldgen:** the no-pit proof (forward ⊆ backward) in every state layer and night stage on 64 seeds per run and 1,000 in the soak; every region border 3 to 4 gates, at least two by road; every district on a loop; every ledge a shortcut of at least 40 cells and a walk back under 90 s; a ledge within 60 cells of every hub and kept fire; every plateau POI seen from a road before its join (tenet 2); face runs within the shape rules; every site's level and route-time bands; density per region and per district; threat rules as `PLAN.md` §2.6; a deliberately pitted county the solver must reject.
- **PSP budgets:** `heap_budget.rs` held (sim resident under 6 MB, build peak under 9 MB, console form); the tour at 55 fps at Full with the new scenes; free RAM in play at least 3 MB.
- **Play:** the Reader, the Rusher and the Explorer on seeds 1 to 5 reach their endings; the first walk at threat 1 and under 4 minutes (Reader); time to the mine mouth within its band; deaths per hour within 20 % of today's; at least one ledge used in the first hour by every bot; `EXPERIENCE.md`'s checks green.

### 9.2 Risks

| Risk | Answer |
| --- | --- |
| **Boxed in.** Cliffs everywhere read as walls and the county as a maze | Tenet 1 with numbers: open districts, 3 to 4 gates a border, 2 joins a district; the R0 paper review before carving; the owner's playtest at R8 |
| **The rebuild of `stories.rs` and the 16 chunks** (the county's biggest code) fails to place stories | `terraced` beside `plain`; the stories keep their rows and only their spot-finding learns levels; 1,000-seed soak with today's 1-in-100 placement failure rate as the bar |
| **Softlocks from one-way ground** | The no-pit proof is structural (§4.7), on every state layer and night stage, and a grok audit |
| **Cheese from high ground** | Hold-then-evade heals what was shot from a cliff; perch rows shoot back |
| **The art bar** on long faces | Region pieces, jog rules, the critique loop, A/B frames for the face style before R3 commits |
| **PSP frame time** (GE-bound) from cliff shadows and decks | Balanced paints the face's shadow; decks borrow chunk slots; the tour gate |
| **PSP build time** | The smaller county more than pays for the new stages; `jane bench gen --packed` by stage |
| **The night lands twice** | `NIGHT.md` R4 waits for the switch (§7.1) |

**Rough size:** R0 M, R1 L, R2 M, R3 L, R4a L, R4b XL, R5 M, R6 L, R7 3 x M, R8 M. With three or four agents after R0, about eight working rounds to the switch and three after it.

---

## 10. Owner questions (recommended defaults in bold)

1. **County size.** **1,536 square** (route times as now, denser, cheaper on the PSP) / keep 2,000 / 1,280 (tighter still).
2. **Levels.** **Four (0 to 3)** / three (no crown; the School on the plateau).
3. **Face height.** **Two cells a level** / three (more drama, more of the screen spent on rock).
4. **Enemies and ledges.** **Chasers hop after her inside their leash; bosses, big and perch rows never** / nothing hops but her.
5. **Shooting up.** **Only at a target standing nearer the edge than she is to the foot** / never up.
6. **Overlooks.** **Yes**, a few authored edges where her camera eases out over the drop (presentation only) / no.
7. **Dungeon height.** **The mine, the Burial and the School, after the county** / none / all eight.
8. **Old saves.** **Refused at the switch** with a plain message (pre-release) / kept by keeping the old county for them (costs a second builder for ever).
9. **STORY CHANGE (M1): Company Row goes down steps** to a sunken lane under the High Street ("below the street, where the shift is kept"). **Accept** / keep the level gap of `NIGHT.md`.
10. **STORY CHANGE (M2): the Cutting is the Company's.** The ramp road through the escarpment was blasted by the Goldskin Company to bring the Works' road down to Castle; before it the Works were "up the steps" (the Church steps). A plate at the Cutting's top (GOLDSKIN MINING Co. THE CUTTING. 18-- ) and the N3 files walking down it make the night's road the Company's road. **Accept** / no.
11. **STORY CHANGE (M3): the footbridge fell the night of the bell.** The Repair verb gate on the river (§4.3) is the footbridge; a new council notice at its Castle end says it is closed until further notice, and the dog, asked, says it went down "the night she stopped saying anything" (two new rows, written to `VOICE.md`). **Accept** / leave its cause unsaid.
12. **Night barriers on gates.** **Two side gates, never the Cutting, never the first walk, each border keeping two open** / keep them on side tracks as `NIGHT.md` had them.
13. **Order.** **R0 paper review first, `NIGHT.md` R4 after the switch** / build the night's places on the old county now and again later.
