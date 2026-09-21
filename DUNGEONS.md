# Jane: Dungeons

What a dungeon is in this game, how one is generated, and what the seven of them are. Written September 2026. Pair with `PLAN.md` section 3 (authored mission, generated space), `DESIGN-2020.md` section 4 (what 2020 drew), `STORY.md`, `VOICE.md`, `PLATFORM.md` section 2 (co-op) and `MISSING-SYSTEMS.md`.

This is a design, not code. Nothing here is IN until a test names it (`SYSTEMS.md` Rule 0). Anything about feel, tone or difficulty that a test cannot settle is marked **Question for John**.

Contents:

1. Knowledge: what makes a dungeon good, by source, each with a rule for this game, and a checklist.
2. The generator: data shapes, template format, layout, proofs, the Gold Mine worked through.
3. The dungeons: cellar, Gold Mine, Museum, Butterfly Forest, the pipes and the Factory, the Burial Chamber, the School, and two small optional ones.
4. ASKS: tiles, props, units, spells, items, dialogue, engine capabilities, build order.

Facts about the live engine that this design leans on, checked against the code:

- A cell is 8 px. The view is about 48 x 27 cells. Live rooms are 12 x 10 to 40 x 24 cells, corridors are 3 cells wide, and `gate_h` / `gate_v` seal a 3-cell corridor exactly.
- A plate is pressed by **any living unit** or any `push` / `carry` prop (`sim/triggers.ts`). A rat can hold a door open. So can a friend.
- `schoolTouch` switches a prop on when a bolt of the school it `answers` ends within 14 px of its centre. It skips props that are already `on`. `worldVerb` skips props that are `used`.
- The solver (`world/validate.ts`) floods from the entrance, spends keys greedily, fires every reachable `use` list once, assumes fights are won, and fires `while` triggers when their rect is reached **without checking their `when` conditions**. It does not know which verbs the player has. It ignores `release`. These gaps are closed in section 2.6.
- Flags are world-wide, not per zone. Generated flag names must carry the zone id.
- Nobody can be revived. Whoever dies wakes at the party's last fire. A lock-in holds while anyone is alive inside it.

---

## 1. Knowledge

Each entry: the principle, where it comes from, what is taken and what is left, and the rule it becomes here.

### 1.1 Zelda

**K1. A dungeon is a dependency graph, and the graph is what is being designed.**
Mark Brown's Boss Keys series draws every Zelda dungeon as a graph of keys and the locks they open, and asks three questions of it: does the path branch, does the player choose between branches, and must the player go back. From those he sorts dungeons into a simple gauntlet, a standard lock-and-key design, and a puzzle box that changes its own layout to reveal the way. His favourites are the last kind: "like solving a Rubik's cube from the inside." Sources: [How my Boss Key dungeon graphs work](https://www.patreon.com/posts/how-my-boss-key-13801754), [Zelda Dungeon interview with Mark Brown](https://www.zeldadungeon.net/we-interviewed-mark-brown-of-game-makers-toolkit/), [Hyrule University's review of Boss Keys](https://hyruleuniversity.wordpress.com/2017/09/29/review-of-boss-keys/), [BorisTheBrave, Lock and Key Dungeons](https://www.boristhebrave.com/2021/02/27/lock-and-key-dungeons/). The 2020 folder already holds this exact icon kit (`DESIGN-2020.md` section 4.0) and `validate.ts` is its machine form.
*Take:* the graph as the authored artefact. *Leave:* long linear gauntlets, which are what most later 3D Zelda dungeons became.
**So in this game:** every dungeon is authored as a mission graph in data. At least once per dungeon the player stands in a hub with two open leads and chooses. A graph that is a straight line is rejected at review.

**K2. The dungeon item changes what earlier rooms mean.**
Brown credits Link's Awakening with fixing the formula: "the boss key and backtracking with items were all introduced here" ([interview](https://www.zeldadungeon.net/we-interviewed-mark-brown-of-game-makers-toolkit/)). The item is not a key with a different picture. It is a new way of reading rooms the player has already walked through: the crack in the wall was always there.
*Take:* the verb is found inside, in the middle, and the first half of the dungeon is seeded with things it changes. *Leave:* items that are used in one dungeon and never again. `PLAN.md` section 2.6 already forbids that: a verb re-opens the old map.
**So in this game:** the first thing a verb opens is seen before the verb is found. At least two rooms before the verb node hold something the verb changes. The verb then opens things in every earlier region.

**K3. The mini-boss guards the item. The boss tests it.**
A fight in the middle pays for the item. The fight at the end asks whether the player understood it.
*Take:* both. *Leave:* bosses that cannot be hurt at all without the item. In a game with consumable materials and no pause, a boss gated on a consumable is a soft-lock waiting for a seed.
**So in this game:** the verb is the clever way to beat the boss, never the only way. Every boss can be beaten slowly with what Jane had at the door. The verb makes it three times faster and much safer.

**K4. Small keys pace. The boss key is a statement of the goal.**
Small keys are fungible and consumed, which creates the series' one classic failure: spend a key on the wrong door and the dungeon cannot be finished. Tom Coxon's [metazelda](https://github.com/tcoxon/metazelda) avoids it by construction (to reach key n the player must already hold keys 1 to n-1). The big key is different: it is seen as a locked door early, so the whole dungeon is a walk toward something already shown. That is this game's spine in small (`PLAN.md` section 2.2: the School is visible from the platform).
**So in this game:** `key_generic` stays ("Opens any plain lock once, then it is the lock's"). Every zone holds at least as many plain keys as plain locks, and the solver proves that no order of spending strands the player (check C3). The boss door is visible, by line of sight, before its key is found (check C10).

**K5. The map and the compass are pacing devices, not conveniences.**
Finding the map turns wandering into planning. Finding the compass turns planning into intent. They arrive a third and two thirds of the way in.
*Take:* the beat. *Leave:* the items. This game has no markers beyond what Jane has seen or been told (`PLAN.md` section 2.5), and interiors already keep a fog bitmap that is the map.
**So in this game:** each dungeon has one wall notice that is its map, written in the `VOICE.md` register of whoever ran the place: a fire assembly plan, a floor plan by the cloakroom, a timetable. Reading it reveals the fog for the rooms it names (ask: a `reveal` action). It is true about the rooms and may be out of date about the doors.

**K6. One central mechanism per dungeon, small enough to hold in the head.**
The Water Temple's three water levels, Eagle's Tower's pillars, Stone Tower's inversion and the Sandship's timeshift all make the whole building one puzzle. Eiji Aonuma's apology for the Water Temple is exact about what went wrong: not the puzzle but the friction. "I am most sorry that it was not easy for you to put on and take off the heavy boots, that all the time you had to visit the inventory." ([Game Informer](https://gameinformer.com/b/news/archive/2009/11/29/aonuma-apologizes-for-water-temple.aspx), [Zelda Universe](https://zeldauniverse.net/2020/03/03/zeldas-study-the-regret-that-haunted-eiji-aonuma-for-13-years/)).
*Take:* a whole-building state. *Leave:* more than three states, any state change that needs a menu, and any state change whose switch is far from where its effect is seen.
**So in this game:** each dungeon has one central idea with at most three binary state variables. A state is changed by one USE on one prop, the prop sits on or beside the hub, and the change is heard everywhere before it is seen. The solver floods the product of states, so a two-state building is as provable as a plain one (section 2.6).

**K7. Shortcuts open and drops are one-way.**
A one-way drop commits the player. A shortcut that opens from the far side turns a long first trip into a short second one. Ocarina's hub and spoke plan ("all the key rooms fork off from one central plaza", [interview](https://www.zeldadungeon.net/we-interviewed-mark-brown-of-game-makers-toolkit/)) does the same job with a room.
**So in this game:** death sends the player to a fire and nothing else brings her back, so the shortcut is not a nicety. Every dungeon opens at least one route from its deep end to within twenty seconds' walk of its rest point, and every one-way drop has a way for friends to follow and a way out that does not need the boss.

**K8. A room is one screen and one idea.**
A Link to the Past and Link's Awakening build dungeons from rooms the size of the screen. The player reads the whole problem at once.
**So in this game:** a standard room template is at most 28 x 20 cells of floor, under one view. Only hubs and arenas are larger. A template teaches or tests one thing. If a room needs a second idea it is two rooms.

**K9. A small place, easy to understand, with everybody slightly suspect.**
Takashi Tezuka on Link's Awakening and Twin Peaks: "I wanted to make something that, while it would be small enough in scope to easily understand, it would have deep and distinctive characteristics", and Iwata recalls he "suggested we make all the characters suspicious types" ([Iwata Asks](https://iwataasks.nintendo.com/interviews/ds/zelda/1/2/)).
**So in this game:** every dungeon is an ordinary institution first: a mine with a pay office, a museum with opening hours, a school with a timetable. It has one wrong thing, stated flatly, and the wrong thing is the mechanism (K6). This is `VOICE.md` rule 3 applied to architecture.

### 1.2 Procedural dungeons that keep an authored feel

**K10. Generate the mission first and the space second. Build with cycles, not trees.**
Joris Dormans separates the two: "The mission dictates a logical order for the completion of the tasks, which is independent of the geometric lay-out" ([Adventures in Level Design, 2010](https://pcgworkshop.com/archive/dormans2010adventures.pdf)). Unexplored then composes levels from cycles: two arcs between an entrance and a goal, with a pattern applied to the pair. When one arc is short and one long, the lock goes at the goal and the key at the end of the long arc, so the player sees the lock first and is close to it when the key is found ([BorisTheBrave on Unexplored](https://www.boristhebrave.com/2021/04/10/dungeon-generation-in-unexplored/), [Game Developer](https://www.gamedeveloper.com/design/unexplored-s-secret-cyclic-dungeon-generation-), [ctrl500](https://ctrl500.com/tech/handcrafted-feel-dungeon-generation-unexplored-explores-cyclic-dungeon-generation/)). Unexplored works on a fixed 5 x 5 grid of nodes that are "never deleted or moved, just annotated", which makes the step to tiles easy.
*Take:* mission then space, the coarse lattice, the cycle as the unit of design, the named cycle patterns (lock and key, hidden shortcut, dangerous route). *Leave:* grammar rewriting of the mission itself. `PLAN.md` fixes the order of challenges, so the mission is authored whole.
**So in this game:** the mission graph is data written by hand. Every graph contains at least one authored cycle. The generator only embeds it.

**K11. Lay the solution path first. Vary inside marked chunks only.**
Spelunky picks a guaranteed path across a 4 x 4 grid, gives each cell a hand-made template with the exits that path needs, then randomises only marked chunks inside the templates ([Darius Kazemi's generator lessons](http://tinysubversions.com/spelunkyGen/), Derek Yu, *Spelunky*, [Boss Fight Books](https://bossfightbooks.com/products/spelunky-by-derek-yu)).
*Take:* path first, templates with declared exits, variation only in sockets. *Leave:* per-tile dice on the critical path.
**So in this game:** critical nodes are placed before anything optional. A template's grid is fixed. Only its sockets vary.

**K12. Pools by difficulty. Dead ends hold something. Reject and re-roll.**
The Binding of Isaac grows a floor plan, collects its dead ends, and gives each one a purpose; the boss goes in the farthest. Interiors come from easy, medium and hard pools by depth. A plan that fails its checks is thrown away and rolled again ([BorisTheBrave on Isaac](https://www.boristhebrave.com/2020/09/12/dungeon-generation-in-binding-of-isaac/)).
*Take:* all three. *Leave:* loopless floors and rooms that are only fights.
**So in this game:** no dead end is empty: it holds a jar, a page, materials or a note. The boss room is the farthest critical room from the entrance by walking distance with shortcuts closed. Re-rolling is the normal path, as it already is (`world/index.ts`).

**K13. Author the flow by hand. Make the rooms by hand. Never repeat a room. Backtrack a little, then give up.**
Enter the Gungeon picks one of a few hand-drawn flow graphs per floor, each "designed around a specific feature"; fills it from a large pool of hand-made rooms, avoiding repeats; lays loops by choosing exits that "bring the two unclosed ends of the loop closer together"; and on a placement failure will "backtrack and regen the choice of room, up to 3 times" ([BorisTheBrave on Gungeon](https://www.boristhebrave.com/2019/07/28/dungeon-generation-in-enter-the-gungeon/)).
**So in this game:** this is the layout algorithm, nearly as is (section 2.5). The pool is smaller: three variants per pool before a dungeon ships, five as a target.

**K14. Set pieces are pasted in whole.**
Diablo generates its labyrinths but pastes quest rooms in verbatim, one per level, and scatters small pre-made patches by pattern match ([BorisTheBrave on Diablo](https://www.boristhebrave.com/2019/07/14/dungeon-generation-in-diablo-1/)).
**So in this game:** arenas, the Headmaster's office and the snake's room are set pieces: one template, no variants, mirrored at most. Dressing (barrel piles, torch runs) is the scatter, and `Kit` already has it.

**K15. What breaks, and the defence against each.**

| Failure | Who met it | Defence here |
| --- | --- | --- |
| Unwinnable seed | Everyone | The solver, plus ablation: remove a key and prove the lock really holds (C1), plus the adversarial key order (C3) |
| Sameness | Isaac early on, any small pool | Variants differ in contents and shape, never in contract. No template twice in one dungeon. Three variants per pool or the pool does not ship |
| Mazes without meaning | Diablo's corridors, most roguelikes | No corridor exists unless a mission edge needs it. No dead end without a holding (K12) |
| Keys behind their own locks | Naive lock and key generators | Falls out of the solver. metazelda's key levels are the principle; here the order is authored |
| The generated thing feels generated | Everyone | Dormans: cycles. Gungeon: hand-made rooms. Here: both, plus one authored central idea per dungeon (K6) |

### 1.3 Co-op that is fun and never required

**K16. Puzzles built for a head count feel like patches when the count is wrong.**
Four Swords Adventures was built for four, and the single-player formation system exists so one person can do "the same puzzles that would normally require four players" ([Aonuma, GDC 2004](https://www.nintendoworldreport.com/interview/2180/gdc-2004-eiji-aonuma-zelda-roundtable)). Tri Force Heroes went further: three or nothing, with doll stand-ins alone. Reviews called the solo game tedious and noted it cannot be played by two at all ([Nintendo World Report](http://www.nintendoworldreport.com/review/41340/the-legend-of-zelda-tri-force-heroes-3ds-single-player-review), [Wikipedia summary of reception](https://en.wikipedia.org/wiki/The_Legend_of_Zelda:_Tri_Force_Heroes)).
*Take:* the pleasure of two people doing two halves of one thing. *Leave:* any head count in the design.
**So in this game:** every room is designed for one. More hands make it faster or safer. **Anything a friend can do, a barrel, a rat or patience can do.**

**K17. Portal 2's test, turned round.**
Valve sent a co-op chamber back the moment a tester solved it with one pair of portals: nothing in co-op may be solvable alone. It also gave players a ping, because pointing is most of co-operation ([Portal Wiki, Ping Tool](https://theportalwiki.com/wiki/Ping_Tool), [Wikipedia, Portal 2](https://en.wikipedia.org/wiki/Portal_2)).
**So in this game:** the gate is the inverse. A room that a solo bot cannot finish does not ship. A room where a second bot saves no time is a missed chance and is noted, not rejected. A ping (one key, a mark on the floor in the coat colour) is worth having later and is not asked for here.

**K18. Roles, not classes.**
It Takes Two gives each level "new, paired, asymmetrical capabilities" and is designed from the first day around two people ([Wikipedia](https://en.wikipedia.org/wiki/It_Takes_Two_(video_game)), [The Ringer interview with Josef Fares](https://www.theringer.com/2021/4/22/22396763/josef-fares-it-takes-two-interview-co-op-gaming)).
*Take:* asymmetry is the fun. *Leave:* asymmetry by character. Here everyone is the same heroine in a different coat.
**So in this game:** roles come from position. One lures and one pulls the lever. One holds the plate and one loots. One pushes the great torch and the others guard her back. The room offers the roles; nobody is assigned one.

**K19. Drop in, share the space, take what you open.**
Secret of Mana let a second and third player sit down mid-game in a shared world ([Wikipedia](https://en.wikipedia.org/wiki/Secret_of_Mana)). Diablo made loot first come and scaled the monsters to the party. This game has the first two built (`PLATFORM.md` section 2) and deliberately does the opposite of the third: the world is never rebalanced, the people are weakened (four players: each deals 38% and takes 145%).
**So in this game:** a split party is very weak. So a split must be **short, close and not a fight**: one room apart, twenty seconds, a plate or a lever or a carried rock at the far end, never a lock-in. The reward for splitting is time. The punishment for lingering is arithmetic: things respawn, patrols come round, and she is at 38%.

**K20. With no revive, the run back is the revive.**
A death in a dungeon sends that player to the last fire. The party is then split by definition, and the dead one is walking alone at 38%.
**So in this game:** the fire inside the dungeon and the shortcut to it matter twice as much in company. Every lock-in has a way in from outside that opens when it seals, so the one who died can rejoin the ones still fighting (check C6). This is how `PLAN.md`'s rule ("no lock-in room may trap one player away from the others without a way for them to follow") is met.

### 1.4 Atmosphere

**K21. The mansion is one big lock and key dungeon, and it is less backtracking than it feels.**
Chris Pruett's analysis of Resident Evil names the pattern recursive unlocking: the house "opens itself up like a spiral shell." Measured on a speedrun, most rooms are visited only twice and 38% once. "When a puzzle in a room becomes solvable, it's likely that the next item required after it... is in an adjacent room." The real game is routing: "the player unlocks shortcuts and the player runs out of ammo." And going back is a comfort: early rooms "are familiar and the location of nearby saves and health has been long since memorized" ([Recursive Unlocking](https://horror.dreamdawn.com/?p=81213)).
**So in this game:** the hub is crossed at least three times and is different each time: a door now open, a light now on, an exhibit not where it was. The next thing needed is usually one room from where the last thing was found. Materials for Repair are the ammunition: just enough, proven enough (C4).

**K22. The safe room is a real room.**
Resident Evil built the save point into the house: a door, a typewriter, a box, and music that is calm and not quite at ease ([Film Stories](https://filmstories.co.uk/features/resident-evil-save-rooms-kept-us-safe-from-the-stresses-of-survival-horror/), [AV Club](https://www.avclub.com/resident-evil-save-room-music-is-its-own-melancholy-mus-1798257360)).
*Take:* the room. *Leave:* ink ribbons. Saving is scarce enough already.
**So in this game:** each dungeon has one rest room, always from the same template family so it is recognised on sight: small, one door, warm light, no spawn sockets, nothing paths into it (the hub rule from `PLAN.md` section 2.6: threat 0, nothing follows you in). It is what the institution would have had: the mine's First Aid Room, the museum's cloakroom, the factory's time office, the school's sick bay. It sits just off the hub, before the mini-boss.

**K23. An ordinary place with a second place under it, and a signal between them.**
Silent Hill's team chose "nostalgic streets that trigger a feeling of deja vu, (seemingly) normal people, and strange ugly monsters"; the fog was a draw-distance fix that fitted Toyama's idea of a game as "someone else's dreams"; Yamaoka wrote sounds, not music ([1999 developer interview, shmuplations](https://shmuplations.com/silenthill/)). The siren tells the player which town she is in.
*Take:* two faces and a signal. *Leave:* radio static, body horror, nonsense (`VOICE.md`).
**So in this game:** the bell at nine is the siren. Each dungeon has two faces, by the clock or by its own mechanism: lit and dark, powered and dead, lesson and break. The change is announced by a sound one beat before anything moves.

**K24. See less than you want. Show movement without a cause. Go home before dark.**
Darkwood limits sight to a cone and lets the player see "objects move without knowing what caused the motion." Its makers set out to build horror without jump scares. Its day is for exploring and its night is for surviving in a hideout, and getting caught out at sundown is the threat that shapes the day ([Game Developer](https://www.gamedeveloper.com/design/how-i-darkwood-i-s-visibility-mechanics-create-a-new-kind-of-horror), [Wikipedia](https://en.wikipedia.org/wiki/Darkwood)).
*Take:* light as the edge of knowledge, the hideout, the clock as pressure. *Leave:* base defence.
**So in this game:** nothing ever spawns inside a player's light. Idle patrols are routed along the edge of torch radius, so shapes cross the dark end of a room. A lock-in says so in a toast before anything stands up. A dungeon entered at four in the afternoon is a different decision from one entered at eight.

### 1.5 The checklist

A dungeon ships when every line is true. Lines marked (S) are proven by the solver on every seed. Lines marked (T) are proven by a test. Lines marked (R) are review.

1. (R) It is an ordinary institution with one wrong thing, and the wrong thing is its mechanism. K6, K9
2. (R) Its mission graph has a hub with a real choice, and at least one authored cycle. K1, K10
3. (S) Every critical node is reachable, in the authored order, and each lock really holds without its key. C1
4. (S) No order of spending plain keys or materials strands the player. C3, C4
5. (S) The verb is learned before anything demands it, and its first use is in a room with no enemies and the materials to hand. C5
6. (S) The first thing the verb opens, and the boss door, are seen before they can be opened. C10
7. (R) The mini-boss guards the verb. The boss is three times easier with the verb and still possible without it. K3
8. (S) A rest room exists at 40 to 60% depth, off the hub, with no spawns. C7
9. (S) A shortcut opens from the deep end to within twenty seconds of the rest room. C9
10. (S) Every lock-in shows a way in when it seals and hides it when it clears or resets. C6
11. (T) A solo bot finishes every template in every rotation, and the whole dungeon on three seeds.
12. (T) A two-bot test shows each co-op affordance saves time and is never needed.
13. (R) It gives a big jar, at least one page, the next place's key, and its verb opens at least three things on the surface that the player has already walked past.
14. (R) It has a night face or a second face of its own, announced by a sound.
15. (R) Its wall notice, its sign outside and its item descriptions pass `VOICE.md`. No row has the heroine's name baked in.

---

## 2. The generator

**Authored mission, generated space, proven result.** A dungeon is three kinds of data and one algorithm:

| Thing | Where | Who writes it |
| --- | --- | --- |
| Mission graph: the fixed order of challenges, their locks and keys, the contract names | `data/dungeons/<id>.json` | Authored once per dungeon |
| Room templates: text grids with named sockets, in pools | `src/world/templates/<dungeon>/*.room` | Authored, three or more per pool |
| Story rows that lean on contract names: triggers, dialogue, quests | `data/*.json` as today | Authored, unchanged by the generator |
| Layout, corridors, which template fills which node, dressing, enemy mix | `src/world/dungeon.ts` | Generated per seed, then proven |

The generator returns an ordinary `Blueprint` through `Kit`, so `buildZone`, the solver, the save format and the renderer do not change. `BUILDERS.mine` becomes `(seed, attempt) => buildDungeon(MINE_DEF, seed, attempt)`.

### 2.1 Units of space

- **Bay.** The lattice cell: 36 x 28 cells, of which the outer 4 on every side are margin. Corridors run in the margins and never through a room.
- **Room footprint.** 1 x 1, 2 x 1, 1 x 2 or 2 x 2 bays. A 1 x 1 room has at most 28 x 20 cells of floor, which is under one view (K8). A 2 x 1 hub has 64 x 20. A 2 x 2 arena has 64 x 48.
- **Lattice.** 4 x 4 bays for the mine (144 x 112 cells plus an 8-cell border is today's 160 x 130 exactly), up to 6 x 5 for the School.
- **Mouth.** A door opening: 3 cells on a room's rim, centred on a bay side, so two facing mouths in adjacent bays always line up and a straight 3-wide corridor joins them. Gates are placed by the generator in the corridor just outside a mouth, never by the template.
- **Sill.** The floor cell row just inside every mouth carries the `Sill` tile: walkable, but a pushed prop cannot cross it. Barrels never leave their room, so they can never jam a corridor or be lost to their plate.

### 2.2 Data shapes

```ts
// data/dungeons/<id>.json : one per dungeon. Authored. The generator never edits it.
type DungeonDef = {
  id: ZoneId;                         // "mine"
  name: string;                       // "Goldskin Mine"
  phase: 1 | 2 | 3 | 4 | 5 | 6;       // DESIGN-2020 3.1. Becomes UnitSpawn.phase on every unit
  tiles: { floor: Tile; wall: Tile; alt: Tile[] };
  indoor: boolean;
  ambient: number;
  lattice: { cols: number; rows: number };          // bays are 36 x 28, margin 4
  givenVerbs: Verb[];                 // known on arrival; proven by earlier dungeons
  givenKeys: string[];                // `opens` tags handed over outside (today's GIVEN_KEYS)
  states: StateVar[];                 // reversible building-wide mechanisms. At most 3
  nodes: MissionNode[];
  edges: MissionEdge[];
  budget: {
    sideRooms: [number, number];      // optional rooms, min and max
    critPathCells: [number, number];  // walking length of the first completion, in cells
    restToBossCells: number;          // max, with shortcuts open
    baseHeat: number;                 // enemy cost points for a node with heat 1
    enemies: Record<string, number>;  // units.json row -> cost. The dungeon's whole bestiary
  };
  fallback: Placement[];              // one hand-placed embedding, used if every attempt fails
};

type Verb = "icebolt" | "repair" | "explosion" | "grow" | "spark" | "fireball";

type StateVar = {
  id: string;                         // "lights"
  values: [string, string];           // ["lit", "dark"]
  initial: string;
  flag: string;                       // world flag that holds it. AS BUILT: unset or 0 = `initial`, 1 = the other
};                                    // value, so nothing has to set it when the zone is made. Name it for the
                                      // other value ("museum_dark" when the Museum starts lit).

type NodeKind =
  | "entrance" | "teach" | "fight" | "puzzle" | "key" | "hub" | "rest"
  | "miniboss" | "verb" | "bosskey" | "boss" | "reward" | "exit" | "side";

type MissionNode = {
  id: string;                         // "office". Also the key prefix for everything anonymous in it
  kind: NodeKind;
  order: number;                      // place in the fixed challenge order. Side nodes take their parent's
  critical: boolean;                  // false = optional
  pool: string;                       // template pool: "mine.office"
  holds: Holding[];                   // what goes into the template's sockets
  binds: Bind[];                      // socket, mark or rect -> contract name
  demands: Verb[];                    // verbs this node cannot be finished without
  grants: Grant[];                    // what finishing it gives the mission
  heat: number;                       // 0 .. 1.3, share of budget.baseHeat. 0 = no enemies ever
};

type Holding =
  | { socket: string; loot: Stack[] }
  | { socket: string; unit: string }                                        // units.json row
  | { socket: string; prop: string; talk?: string; needs?: Stack[];
      use?: ActionList; release?: ActionList; locked?: boolean; hidden?: boolean };

type Bind = { from: string; as: string; what: "prop" | "unit" | "mark" | "rect" };

type Grant =
  | { key: string }                   // an `opens` tag: "mine_vault"
  | { verb: Verb }
  | { item: string; qty: number }     // materials: wood, iron
  | { flag: string }
  | { state: string };                // gives control of a StateVar (the breaker, the valve)

type EdgeKind =
  | { t: "open" }
  | { t: "key"; tag: string; gateAs?: string }                              // gate_h / gate_v in the corridor
  | { t: "verb"; verb: Verb; prop: string; needs?: Stack[]; propAs?: string } // broken_steps, rubble, web_wall
  | { t: "state"; var: string; is: string }                                 // passable only in that state
  | { t: "lockin"; gateAs?: string }                                        // seals on entry, opens on clear
  | { t: "oneway"; how: "drop" | "opens_on"; flag?: string }                // from -> to only, or until flag
  | { t: "sight" };                                                         // see, do not pass: the tease

type MissionEdge = { from: string; to: string; kind: EdgeKind; shortcut?: boolean; also?: EdgeKind[] };

type Placement = { node: string; template: string; bay: [number, number]; turn: 0 | 90 | 180 | 270; mirror: boolean };
```

```ts
// Parsed from a .room file. Authored.
type RoomTemplate = {
  id: string;                          // "mine.plate.a"
  pool: string;                        // "mine.plate"
  bays: [number, number];              // [1, 1]
  turns: (0 | 90 | 180 | 270)[];
  mirror: boolean;
  doors: { id: string; side: "n" | "e" | "s" | "w"; bay: number; required: boolean }[];
  sockets: { id: string; kind: SocketKind; cx: number; cy: number; w: number; h: number }[];
  marks: { id: string; cx: number; cy: number }[];          // spawn-on-trigger points, patrol points
  rects: { id: string; cx: number; cy: number; w: number; h: number }[];  // "room", "inner", "drop:1"
  needs: { verbs: Verb[]; items: Stack[] };                 // what the player must bring
  grants: string[];                                         // socket ids she can then reach
  blocks: { what: string; until: string }[];                // negative guarantees, proven by ablation
  coop: CoopTag[];
  heatMax: number;                                          // most enemy cost the room can hold fairly
  grid: string[];
};

type SocketKind =
  | "chest" | "plate" | "push" | "carry" | "lever" | "verbprop" | "rest"
  | "jar" | "page" | "notice" | "spawn" | "boss" | "dress";

type CoopTag = "plate_or_friend" | "twin_hold" | "lure_and_lever" | "carry_relay" | "guard_the_pusher";
```

### 2.3 The template format

A text grid, like the art (`src/art/*.ts`): one character per cell, a legend under it. The grid is the whole footprint including its one-cell wall rim, so a 1 x 1 template is at most 30 x 22 characters.

```
id      mine.plate.a
pool    mine.plate
bays    1x1
turn    0 90 180 270 mirror
doors   e1 required, n1 optional
needs   -
grants  chest:reward
blocks  chest:reward until plate:main
coop    plate_or_friend
heat    3
grid
##############nnn#############
#t............___...........t#
#............................#
#....PP......................#
#....PP......................#
#............................#
#...........BB...............#
#...........BB...............#
#............................#
#.....................r......#
#..........................._e
#..........................._e
#..........................._e
#............................#
#....r.......................#
#............................#
#.......................CC...#
#.......................CC...#
#..??...............??.......#
#..??...............??.......#
#t..........................t#
##############################
legend
  #  wall            .  floor           _  sill
  n  door n1         e  door e1
  P  plate:main      2x2
  C  chest:reward    2x2
  B  push:main       2x2   barrel | crate
  r  spawn           0..2 from the dungeon's enemy table, within heat
  t  dress:torch     ?  dress:pile
```

Rules a template must obey, enforced by a lint test that runs on every `.room` file:

1. The rim is wall except at declared doors. An optional door the layout does not use is filled with wall.
2. Every socket sits on floor with its whole footprint, and no solid socket touches a sill or stands in the straight line between two doors.
3. `spawn` sockets are at least 4 cells from any door. Nobody is hit in a doorway.
4. There is a clear path at least 2 cells wide between every pair of doors with every `push` prop at its starting cell.
5. A template with a `plate` socket has a `push` socket in the same room, and the 2 x 2 push path from one to the other exists (flood of the footprint, with a free cell behind it at each step).
6. Letters are a closed set. Tiles beyond wall, floor and sill come from a per-dungeon legend (`~` water, `:` track, `"` garden, `=` glass) and nothing else.

### 2.4 How a template declares what it guarantees

A template is an interface. It says three things and a test proves each.

- **`needs`**: what the player must bring. Usually nothing. A room that wants Repair says `verbs: repair, items: wood 1` unless the wood is in the room.
- **`grants`**: the sockets she can reach, and that she can leave by every declared door, given `needs`. Proven by stamping the template alone into a blueprint with a stub corridor on each door and running the solver from each door in turn.
- **`blocks`**: what she cannot get without the stated condition. `chest:reward until plate:main` means: with the plate's `use` list suppressed, the chest must stay locked. Proven by ablation in the same test. This is what stops a variant from quietly opening a side way round its own lock.

Then a **bot test** plays it: for each of the eight transforms, a bot enters, does the thing (pushes the barrel onto the plate, opens the chest, walks out of every door), and the test asserts the grant. This is `PLAN.md`'s rule made concrete: a variant that cannot prove itself cannot ship.

**What may vary between variants of one pool:** the grid, where the sockets are, how many `spawn` and `dress` sockets there are, whether the plate is in the open or behind a push maze, whether there are two barrels or one. **What may never vary:** the door set's required members, `needs`, `grants`, `blocks`. The mission only ever talks to the pool.

### 2.5 Layout: from graph to cells

Every random choice comes from the `Kit` stream for `(seed, zone, attempt)`. Lists are sorted by id before any pick. Nothing iterates a `Map`.

```
buildDungeon(def, seed, attempt):
  if attempt == MAX_ATTEMPTS - 1: return stamp(def.fallback)      # never throw at the player

  1. choose   for each node in order: pick a template from its pool, seeded,
              skipping any template already used in this dungeon (K13)
  2. embed    place the entrance at a seeded bay on a seeded edge of the lattice
              for each critical node in `order`:
                 candidates = free bays (or bay groups) adjacent to a placed neighbour in the graph,
                              times the template's allowed turns and mirror,
                              kept only if a required door faces the neighbour's free door
                 score      = + closer to the other end of any cycle this node closes (Gungeon)
                              + farther from the entrance if kind is boss (Isaac)
                              + nearer the hub if kind is rest
                              - bays that would wall in a node not yet placed
                 pick the best of the top three, seeded
                 if none: re-pick this node's template, up to 3 times, then fail the attempt
              then side nodes, onto free doors of their parents, until budget.sideRooms is met
  3. route    adjacent facing doors: a straight 3-wide stub
              everything else (cycle closers, shortcuts, sight lines): A* over the margin grid,
              3 wide, turns only at bay corners. A corridor may carry one alcove (a dead end with a holding)
  4. lock     for each edge: key -> gate_h / gate_v by corridor direction, keyed, named by `gateAs`
                             verb -> a lock segment template in the corridor (the steps fill a 2-tall neck)
                             state -> a gate driven by the state's flag
                             lockin -> the lock-in macro (below)
                             sight -> a 1-wide run of a see-through solid tile (Rail, Fence, Glass, Water)
                             oneway -> a way_in prop, or a gate unlocked by `flag`
  5. fill     holdings into sockets; units with phase = def.phase; enemy mix per node (2.7);
              dressing from the dungeon's table (Kit.pile, Kit.torchRun)
  6. name     bound things get their contract name; everything else `${zone}_${node}_${socket}_${n}`
  7. emit     Blueprint + generated triggers (lock-ins, state gates)
```

**The lock-in macro.** One function writes every lock-in, so none can forget its reset. For a node `N` entered by gate `G`, with spawn marks `m1..mk` and an inner rect `N_inner` that starts 8 cells inside the door:

```
trigger N_lock   enter, rect N_inner, when not flag N_clear
                 actions: lock G, show N_wayin, spawn N_u1..k at m1..k, toast
                 reset:   unlock G, hide N_wayin, despawn N_u1..k
trigger N_clear  while, rect N_inner, when dead N_u1..k
                 actions: unlock G, hide N_wayin, unlock N_reward, flag N_clear
prop    N_wayin  hidden, outside G, `to` = this zone, mark N_in. "Climb in". One way.
```

The way in is hidden until the room seals, so it can never bypass a keyed gate. It is how a friend who was late, or who died and walked back, follows (K20). Boss arenas use the same macro with the boss as the only unit.

**Contract names survive because the mission binds them.** `binds` maps a template's socket, mark or rect to the name the story uses. The mine's arena node carries `{ from: "gate:in", as: "gate_boss" }`, `{ from: "boss", as: "iron_knuckles" }`, `{ from: "inner", as: "boss_arena" }`. The burial's lock-in node binds its four spawn marks to `lockin_a` .. `lockin_d`, its inner rect to `lockin_room` and its entry gate to `gate_snake`. The snake's template carries eight ordered marks `loop:1..8` that become its patrol. `triggers.json`, `dialogue.json` and `quests.json` are untouched. `CONTRACTS[zone]` is then **derived** from the def's binds, and a test asserts the derived list contains every name in today's hand-written list, so nothing can be dropped by accident.

### 2.6 Proof: the solver, extended

The existing flood stays. It gains inputs and the generator adds checks around it. A blueprint that fails any check is re-rolled; the failing check's name is kept for the dungeon viewer.

**Solver changes (in `validate.ts`):**

| Change | Why |
| --- | --- |
| `verbs: Set<Verb>` input, grown by any `learn` action in a reached prop's `use` list or `talk` tree. A prop that `answers` a verb or school fires only if a verb of that kind is known | Today a cold torch or a rubble pile opens for a player who cannot cast. "Taught before demanded" cannot be proven without it |
| `while` triggers fire only when their `when` holds: a `flag` must have been set by a fired list, a `dead` unit must have been reached or spawned by a fired trigger | Today `burial_torches` fires whether or not the torches can be reached |
| A `to` that points into the same zone is a one-way edge | Drops, vents, way-ins |
| Stateful flood: nodes are (cell, state) for the def's `states`. A move keeps the state. A reached lever that controls a state adds an edge to the other state **at the lever's own cell**. Keys and opened gates are monotone as now | Two-state buildings. Starting the other state's flood at the lever, not at the entrance, is what makes it sound: you are where you stood when you pulled it |
| `give` and chest loot of materials counted as now; `needs` of **any** verb prop, not only Repair | Grow and Spark sinks |

**Generator checks:**

| | Check | How |
| --- | --- | --- |
| C1 | Every critical node is reached, in order, and every lock holds | Record the pass in which each node's `room` rect is first entered; passes must not decrease along `order`. Then for each key, verb and state edge: solve again with that one grant withheld; the far node must be unreachable |
| C2 | No key behind its own lock | Falls out of C1's forward solve |
| C3 | No order of spending plain keys strands the player | On the room graph (not cells): search every order of opening reachable plain locks. At most 6 plain locks per dungeon, so at most 720 graph floods. Any order that ends with a required gate shut and no key reachable fails |
| C4 | Materials cannot be starved | Per item: everything all sinks in the zone can consume is no more than everything the zone's chests and guaranteed drops supply. Then no spending order can starve a critical sink |
| C5 | The verb is taught before it is demanded, in safety | Forward solve with verb gating. Also: the first prop that answers the new verb lies in a node with `heat` 0 whose own sockets hold its `needs` |
| C6 | Every lock-in can be followed into | Every trigger that locks a gate also shows a way-in whose mark lies inside the trigger's rect and whose prop was reached from outside without that gate |
| C7 | A rest room at the right depth | A `rest` node exists, `heat` 0, no `spawn` sockets, adjacent to the hub, first reached between 40% and 60% of `critPathCells` |
| C8 | Length is in band | Walking distance of the first completion, by A* between consecutive node centres in solve order, inside `budget.critPathCells` |
| C9 | A loop and a shortcut | With every lock open the room graph has cycle rank of at least 1, and a `shortcut` edge ends within `restToBossCells` of the rest room |
| C10 | The tease | For the verb's first lock and for the boss gate: some cell reached in an earlier pass has wall line of sight to the prop |
| C11 | Plates can be held | Each plate whose `release` re-locks something required shares a room with a pushable that has a push path to it (the template proved it; this re-checks after dressing claimed cells) |
| C12 | Nothing solid can appear on a person | No `show`, `lock` or `fill` target overlaps a rect a player must stand in to cause it. Gates stand in corridors outside trigger rects, as `boss_arena` does today |

**Tests to add** (names are the bar, per Rule 0): `templates.test.ts` (lint, solver and ablation for every `.room`), `template-bots.test.ts` (eight transforms each), `dungeon-gen.test.ts` (64 seeds per dungeon in the suite, 1,000 as a soak behind `DUNGEON_SEEDS=1000`, zero fallbacks allowed in the 64), `dungeon-contract.test.ts` (derived contract contains today's), and the existing `dungeons.test.ts` unchanged and still green on the generated mine.

**The dungeon viewer.** The seed viewer gains a page: 24 seeds of one dungeon side by side, rooms labelled with node ids, locks drawn on corridors, the first-completion path drawn as a line, attempts used and failed check names printed under each. It is how layouts get reviewed without playing 24 mines (`PLAN.md` section 2.3).

### 2.7 Difficulty follows the phase, tension follows the order

- **Phase.** `def.phase` is written to `UnitSpawn.phase` on every unit, so `PHASE_SCALE` does the rest. One `skeleton` row serves the mine at 1 and the School at 6.
- **Heat.** Each node's enemy cost is `round(node.heat * budget.baseHeat)`, capped by the template's `heatMax`. The generator fills it from `budget.enemies` with a seeded mix, within 15%.
- **The curve is authored in the mission**, not computed: low at the door, rising to the mini-boss, zero at the rest room and the teach room, a dip after the verb (the player is handed power and allowed to enjoy it), then highest just before the boss. This is the Resident Evil and Silent Hill shape: dread, release, dread.
- **Fairness caps, checked at generation:** no more than two ranged units in one room; never a stunner and a lock-in together below phase 3; a lock-in's cost is at most 8, which a solo bot at the dungeon's phase must beat in the template bot test. That is what "must survive a player at 38%" means in practice: she cannot win that room alone in a party of four, and she does not have to, because the way in is open to her friends.

Suggested cost table: rat 1, bat 1, spider 1, flower 1, skeleton 2, wall_spider 2, cactus 2, statue 2, pumpkin 2, bandit 3, soldier 3, skeleton_guard 3.

### 2.8 How co-op affordances are expressed

A template lists `coop` tags. Each tag is a promise with a test.

| Tag | The room offers | The solo way | Two-bot test asserts |
| --- | --- | --- | --- |
| `plate_or_friend` | A plate that a friend can stand on | A barrel | Two bots finish faster than one |
| `twin_hold` | Two plates that must both be down | Two barrels, or a barrel and a lured rat | Same |
| `lure_and_lever` | A lever that hurts or moves something standing on a marked rect | The thing is slow; pull it yourself as it arrives | Same |
| `carry_relay` | A carried prop and a long way to take it | Walk it | Same |
| `guard_the_pusher` | A pushed prop that matters (the great torch) through enemies | Push, stop, fight, push | Same |

Rules: an affordance never gates a critical grant behind two bodies, and never gates a bonus behind two bodies either, because solo players must be able to get every jar. It only buys time or safety. A split it invites is one room wide, twenty seconds long and has no fight at the far end (K19). No quest may ask the party to split (`PLAN.md` section 9).

### 2.9 Worked example: the Gold Mine

The mission, which is what is authored. Contract names from today's `CONTRACTS.mine` are in `code`. New names are marked *new*.

```
order  node       kind       holds / binds                                        heat
0      entry      entrance   `exit_door`, mark `entry`, rect `mine_entry`, notice  0.2
1      plate      teach      `plate_a`, `plate_chest` (key_generic), `plate_barrel` 0.4
1      store      key        `store_chest` (key_generic, iron x4)                  0.4
2      guard      fight      `clerk` (drops HM key), guard_chest (wood x2)         0.8
3      core       hub        track dressing, sight of the steps and the big door   0.6
4      firstaid   rest       fire *new*                                            0
5      office     miniboss   `headmaster` (drops vault key); drawer *new* -> learn repair;
                             broken cabinet *new* (wood x1, in the room) -> small jar   1.0
6      gallery    bosskey    `boss_key_chest` (key_mine_boss); adit door *new*     0.5
6      vault      reward     `vault_chest` (gold x5, key_museum), leaf page *new*  0.6
7      arena      boss       `iron_knuckles`, rect `boss_arena`, four hoists *new*,
                             arena_chest *new* (iron x4)                           boss
8      nook       reward     big jar *new*                                         0
-      cage       side       two plates, leaf page                                 0.3
-      powder     side       behind rubble: needs Explosion, a later visit. Small jar  0

edges
entry   -open->                         plate
entry   -open->                         store
entry   -key generic `gate_generic_a`-> guard
store   -key generic `gate_generic_b`-> core
core    -open->                         firstaid
core    -key mine_headmaster `gate_hm`-> office
core    -verb repair `broken_steps` (wood x2)-> gallery
core    -sight->                        arena          (the big door, seen from the hub)
office  -key mine_vault `gate_vault`->  vault
core    -key mine_boss `gate_boss`, lockin-> arena
arena   -oneway opens_on mine_cleared-> nook
nook    -oneway opens_on mine_cleared-> gallery        shortcut
gallery -verb repair `broken_track` (iron x4)-> plate  shortcut: closes the big loop
gallery -open->                         cage
guard   -verb explosion rubble->        powder
```

What the solver and the checks say about this graph before any layout exists: plain keys 2, plain locks 2, both keys reachable before either lock (C3 holds for every order). Wood: sources 2 + 1, sinks 2 + 1. Iron: sources 4 + 4, sinks 4 (track) + 4 (hoists, 1 each). The track and the hoists are both optional, so no spending order touches the critical path (C4). Repair is learned in `office` (order 5) and first demanded by the cabinet in the same room, heat 0 once the Headmaster is dead, wood in the room (C5). The steps are seen from `core` at order 3 and opened at order 6 (C10). The cycle: entry, store, core, gallery, track, plate, entry (C9).

One layout, as the generator would place it on a 4 x 4 lattice. Another seed mirrors it, puts the office west and the gallery east, and hangs the plate room north of the entry.

```
           c0              c1              c2              c3
     +-----------+   +---------------------------+   +-----------+
 r0  |   NOOK    |   |           ARENA           |   |   VAULT   |
     |  big jar  K===|  Iron Knuckles            |   | museum key|
     |           |   |    h1   h2   h3   h4      |   | leaf page |
     +-----K-----+   +-------------B-------------+   +-----V-----+
           |                       |                       |
     +-----+-----+   +---------------------------+   +-----+-----+
 r1 A|  GALLERY  |=SS|           CORE            |=H=|  OFFICE   |
     |  boss key |   |  hub: sees SS and B       |   | Headmaster|
     |           |   |  cart on dead track       |   | the drawer|
     +-----T-----+   +------g-------------+------+   +-----------+
           |                |             |
     +-----+-----+   +------+----+   +----+------+   +-----------+
 r2  |   CAGE*   |   |   STORE   |   | FIRST AID |   |  POWDER*  |
     | two plates|   | key, iron |   |   fire    |   | small jar |
     +-----+-----+   +------+----+   +-----------+   +-----X-----+
           |                |                              |
     +-----+-----+   +------+----+   +-----------+         |
 r3  |   PLATE   |===|   ENTRY   |=g=|   GUARD   |=========+
     | barrel,key|   |  notice   |   | clerk,wood|
     +-----------+   +-----D-----+   +-----------+

  D exit_door      g gate_generic_a (entry-guard), gate_generic_b (store-core)
  H gate_hm        V gate_vault       B gate_boss (lock-in; way in beside it)
  SS broken_steps  T broken_track     K gates that open on mine_cleared
  X rubble (Explosion, a later visit) A adit door to the county
  h1..h4 hoists    * optional side room
```

First completion, as C8 walks it: entry, plate, entry, store, entry, guard, entry, store, core, first aid, core, office, core, gallery, (track, plate, entry: the loop is now open), core, arena, nook, gallery, adit. The hub is crossed five times and is different each time (K21). About 1,100 cells of walking, inside a band of 900 to 1,400.

---

## 3. The dungeons

In the order the story reaches them. Each sits at one balance phase (`DESIGN-2020.md` section 3.1), teaches one verb, and has one idea of its own.

| | Dungeon | Phase | Verb | The one idea | Form |
| --- | --- | ---: | --- | --- | --- |
| 0 | Julie's cellar | 1 | (Icebolt, upstairs) | A loop with two doors on one kind of key | Tutorial |
| 1 | The Gold Mine | 1 | Repair | You are shown the broken thing first | Two nested cycles |
| 2 | The Museum | 2 | Explosion | Lights on, it is a museum. Lights off, the plinths are empty | Hub and four wings, two states |
| 3 | Butterfly Forest | 3 | Grow | No doors. Things grow where the light comes down | Outdoors, a ring of glades |
| 4 | The pipes and the Factory | 3, 4 | Electric | Light is how the machines see you | A drop, a reversal, a way back |
| 5 | The Burial Chamber | 5 | Fire | Cold light shows what is there. Warm light keeps it off | Hub and four corners, any order |
| 6 | The School | 6 | none: all six | The building follows the timetable | Hub, six lessons, a tower |
| A | The Closed Line (optional) | 3 | none | It is not the same tunnel twice | Five rooms, re-rolled per visit |
| B | The Ice House (optional) | 3 | none: Icebolt's world use | You freeze your own way across | Six rooms |

**Growth.** `PLAN.md` wants 150 HP to 1,000 over the game, about half from bosses. Strength 30 to 200 is +170. Proposed: a boss's **big jar** gives +14 Strength (70 HP), a **small jar** +2 (10 HP). Six big jars are +84; forty-odd small jars across dungeons and county are the rest. **Gold-leaf pages** do the same for Spirit: a mini-boss or vault page +6, a small page +2. Numbers are John's to tune; the counts per dungeon below make the curve hit the 2020 table at each dungeon's door. A naming note: the growth action in the working tree is `{ do: "grow" }`, and the spell is also `grow` (`world: "grow"`). They live in different unions and do not collide, but one of them will be misread in a row some day. Renaming the action `found` or the spell `bloom` before either ships would be cheap.

**Conventions.** "Existing row" means the row is in `units.json` today. "By analogy" names the existing row a new one should copy before its numbers are tuned. Every unit is spawned at the dungeon's phase.

---

### 3.0 Julie's cellar (phase 1): keep

**Fiction.** Julie kept the cellar the way she kept everything: labelled, locked where it mattered, and one key short. Two hatches go down from the kitchen wall and come up again at opposite ends, because a cellar with one way out was, in her margin, "a mistake you make once". There are rats, a bench for potions, a storeroom behind a plain lock, and a study with a desk and nothing on it.

The roses grow without light. The dust on the study desk has a clean ring in it, the size of the stand in the kitchen. The dog does not come down. It says the stairs are steep, which they are not.

**The idea.** A loop closed by either of two doors that take the same key. It is the smallest possible lesson in K4 and K7: a key is spent on a choice, and the choice here cannot be wrong. It is already built and tested. It stays authored (`PLAN.md`: "it is a house"), not generated.

**Mission graph.**

```
stair_a -open-> room A (cellar_chest: key_basement x2, barrels)
room A  -key basement `iron_door_a`-> corridor
corridor -open-> rat room (rat_chest: key_generic, vials) | potion room (potion_bench) | study | rose alcove
corridor -key generic `storage_gate`-> storage (storage_chest: wood x4, iron x4)
corridor -key basement `iron_door_b`-> room B -> stair_b
```

**What each room teaches.** Room A: chests, barrels can be pushed. The iron door: keys are consumed. Rat room: rats, and rat meat, which the burial will want. Potion room: the bench. Storage: a plain key is a decision (C3 holds: one key, one lock). Study: nothing, on purpose. Rose alcove: gathering.

**Changes proposed, all small.**

1. One **small jar** in the storage room, beside the wood. The player learns what a jar is at home, before any dungeon asks her to go looking for them.
2. The study's desk gets an examine line about the ring in the dust. It pays off in the Factory (section 3.4). No mechanism.
3. Three things that later verbs open: a cracked wall behind the rose alcove (Explosion: a leaf page), a dry planter in the alcove (Grow: the roses come back for good, so Stone Skin becomes renewable), a cold hearth in room B (Fire: a second rest point under the house).

**Enemies.** `rat` x4. No boss. **Rest:** Julie's bed upstairs. **Night:** nothing changes, unless the rose omen is true that seed (`STORY.md` section 6: pick a white rose after dark and "something in the cellar notices": one `soldier` spawned in the study by a trigger on `night`). **Co-op:** two stairs mean two people can come down at both ends and meet in the middle; either iron door can be opened from either side. **Risk:** none new.

---

### 3.1 The Gold Mine (phase 1): Repair

**Fiction.** GOLDSKIN MINING Co. took the hill apart for forty years and posted a notice when it stopped. The workings are timbered galleries round a shaft hall, with a store, a first aid room, a pay office and a vault, all where a company would put them. The carts are still on the rails. The pay office has been made into something else: benches in rows, a register, a hand-bell on the desk.

The Headmaster brought the school down here when the nights got longer, because it was the safest place he could think of. He still keeps order. His clerk still carries his keys. The register is marked every morning. Behind the door everyone stopped opening, Goldskin's pit boss is still on shift.

**The idea.** *You are shown the broken thing first.* From the hub the player sees the gallery across a flight of broken steps, and the big door beyond it, long before she can do anything about either. It is Dormans' lock and key cycle (K10): the lock at the near end of a long arc, the key at the far end, and the walk back short. Everything proven today is kept: plate and barrel, two plain keys, the clerk, the Headmaster, the vault key, Repair with two wood, the ornate chest, the vault with the Museum key, the arena lock and its reset.

**What changes, and why.**

| Change | Why |
| --- | --- |
| Repair is **found in the mine**, in the Headmaster's confiscated drawer, not taught by the dog at the door | K2, K3: the verb is inside, guarded. The dog's line becomes a pointer. As built (for John's redline): "There is a page missing from this house. The Headmaster confiscates things, and he has never once given anything back at the end of term." / "Broken stairs want wood. There is some in the cellar storage, behind a plain lock." (The draft here said "She left the way of it down there", which has the dog saying "she" about something it could only know by having been her: `STORY.md` section 3.) |
| A **First Aid Room** with a fire, off the hub | K22. Today the nearest rest is outside |
| The **broken track** (`broken_track` and the store's four iron already exist and do nothing) becomes the shortcut from the gallery back to the plate room | K7, C9 |
| An **adit** off the gallery | The mine sign's omen needs a second way out to be fair (`PLAN.md` section 5). With the adit behind Repair, a true omen costs a night or a fight, never the run |
| Four **hoists** in the arena | K3: the boss tests the verb |
| Two side rooms, a jar, two pages | Growth by finding. K12 |

**Mission graph.** Section 2.9.

**Rooms.**

| Room | Teaches or tests |
| --- | --- |
| Entry | The notice (the map, K5): "GOLDSKIN MINING Co. IN CASE OF FIRE assemble in the SHAFT HALL. Do not use the cage. The cage is not in use." |
| Plate | Push. A plate needs weight, and the weight can stay when she cannot |
| Store | A free key, and iron she has no use for yet: a question |
| Guard | First real fight. The clerk drops the HM key; the chest holds the wood |
| Core (hub) | The choice: HM door, the steps she cannot cross, the big door she can see, the vault she cannot. Crossed five times |
| First Aid | Rest. One door. A stove, a cot frame with no mattress, a cabinet |
| Office | The Headmaster. Then the drawer (the verb), then the cabinet (first use, safe, wood in the room, pays a small jar) |
| Gallery | Boss key. The track and the adit |
| Vault | The reward that matters to the story: the Museum key. A leaf page |
| Arena | The boss, and the verb under pressure |
| Cage (side) | `twin_hold`: two plates, two barrels in the room. A leaf page |
| Powder store (side) | Sealed by rubble. Seen now, opened after the Museum |

**Enemies.** All existing rows: `rat`, `bat`, `skeleton`, `skeleton_clerk`, `headmaster`, `iron_knuckles`. Cost budget `baseHeat` 6.

**Mini-boss: the Headmaster** (existing row: `melee_stun`, `melee_fast`). He tests the verb learned before this dungeon: Icebolt's chill (speed x0.6) is how a phase 1 player keeps him at arm's length, and his stun punishes standing still to cast. On death he unlocks the drawer (`onDeath: unlock hm_drawer`). The drawer is a 2 x 2 `once` prop that opens a dialogue tree, like the fire scroll:

> CONFISCATED. To be returned at the end of term.
> A catapult. A tin of humbugs. One page, in handwriting you know from a letter: "Mending. It costs what the thing is made of. Wood for wood, iron for iron. It will not mend people. I have tried."

**Boss: Iron Knuckles** (existing row, 800 HP, slow, hits in flurries). Kept beatable exactly as today, so the proven test stands. Added: four **broken hoists** round the arena, each over a marked rect. Repair a hoist (1 iron; four iron are in a chest inside the arena) and its lever appears. Pull the lever while he stands under it: `strike` the rect for 200 physical and `staggered` (4 s, cannot move, takes double). Each hoist works once. Four clean drops kill him without a swing; one or two make a plain fight short. He walks at 0.45 px a tick against Jane's 1, so alone she can lead him under, run to the lever and pull as he arrives. With a friend, one leads and one pulls (`lure_and_lever`).

**Repair in engine terms.** Unchanged: spell kind `world`, `worldVerb` finds the nearest prop within 2 m that `answers: "repair"` and is not `used`, takes its `needs` from the caster's bag, runs its `use` list. Props here: `broken_steps` (hide self), `broken_track` (hide self), `broken_cabinet` (hide self, show `cabinet_jar`), `broken_hoist` (hide self, show `hoist_lever_n`).

**Keys and rewards.** Plain keys x2, HM key, vault key, Large Mine Key, `key_museum` (all existing). Big jar (nook). Small jars x2 (cabinet, powder). Leaf pages x2 (vault, cage). Gold bars x5 + x3. **Rest:** First Aid Room; also the fire at the mine mouth chunk. **Shortcut:** the track (gallery to plate room), then the nook gate after the boss. **Exit:** the adit, to a county mark `mine_adit` a short way round the hill from `mine_mouth`.

**What Repair re-opens on the surface.** The collapsed footbridge that is the short way home (`PLAN.md` section 2.6). The farm's pump (a side quest). Broken stiles in the Long Hedge. Manhole ladders in town, which are the way into the pipes.

**At night.** The zone is indoors and ignores the clock, but two omens live here. *No exit, some nights:* if true, `exit_door` carries a `nightLock` line from the inside between nine and five, and the adit is the way out. *The Headmaster still rings the bell at nine:* if true, at nine every unit in the mine is sent to its spawn cell (`send`), which resets any fight the player was winning and clears any room she had emptied. **Question for John:** `blueprint.ts` says a `nightLock` goes on the outside door only, so nobody is ever shut in. The omen as written in `PLAN.md` shuts people in. One of the two has to give; this design assumes the omen wins here and only here.

**With 2 to 4 players.** Plate: a friend stands on it. Cage: two friends instead of two barrels. Arena: lure and lever. Guard room: nothing to split for, by design; it is the first real fight and the party should feel why it stays together. Nothing else changes.

**What could go wrong.** A hoist rect overlapping a lever cell, so the puller hits herself: template lint forbids it. `strike` must not hit friendly units unless told to. The adit needs a county chunk change (`mine_adit` mark and a door), which touches `CONTRACTS.county`.

---

### 3.2 The Museum (phase 2): Explosion

**Fiction.** The Castle Museum is limestone, with four wings off a round hall: SCIENCE, ARTS, HISTORY, and one whose name has been painted over in council cream. The floor plan by the cloakroom still shows it. Opening hours are ten to four. A notice from the Attendant asks visitors not to touch the exhibits, and adds that the exhibits have been asked the same.

With the lights on it is a museum. The armour stands on its plinths, the fox is in its case, the waxwork miner in the Goldskin gallery holds up his lamp. The lights are on one breaker in the maintenance room. When they go off the plinths are empty, and you can hear where the exhibits have got to. When they come back everything is in its place. The dents stay.

**Decision: the Museum gives Explosion.** 2020 drew Explosion in the mine's boss room and needed it for one butterfly, so it must come before the Forest. 2020's Museum had no enemies and no boss: "a key-shuffle building". It keeps the key shuffle (K21: the mansion) and gains one mechanism that supplies the enemies.

**The idea.** *One breaker, two buildings.* A state variable `lights` (`lit` / `dark`), thrown at a lever in Maintenance, beside the hub.

| | Lit | Dark |
| --- | --- | --- |
| Exhibits | Solid props on their plinths. Some stand in doorways | Units that walk. Their plinths and doorways are empty |
| Plinths | A plinth is a plate. The exhibit's weight holds it down: its case stays open | The plate rises when the exhibit leaves: its case locks, or a grille opens |
| Power shutters | Up | Down |
| Seeing | Ambient 0.6 | Ambient 0.08. Jane's own light and nothing else |

So some doors are open only in the light and some only in the dark, and one (the stores) in neither: lit, an armour stands in it; dark, the armour walks off and the shutter drops. **Explosion is the answer to that door.** A lit exhibit is a prop that `answers: "blast"`. Blown up in the light, it is gone for good, in both states. Before the verb, the exhibits always come back. After it, the player can remove them one at a time, and rooms she crept through become rooms she owns (K2).

In engine terms the breaker's `use` list is an `if` on the flag `museum_lights`: going dark hides each exhibit prop, spawns its unit at the plinth mark, locks the shutters and switches the gallery lamps off; going lit despawns the units, shows the props (unless that exhibit's `gone` flag is set), unlocks the shutters, switches the lamps on. A unit killed in the dark is back on its plinth in the light and steps down fresh next time, so flipping the breaker is never free: this is the Resident Evil economy (K21) with potions and mana as the ammunition.

**Mission graph.**

```
order node         kind      holds                                                    heat
0     atrium       entrance, hub   floor plan (notice). Sees: the painted-over arch, all four wing doors   0
1     history      puzzle    the boat exhibit: key_toilets inside it                   0.3 dark
2     toilets      key       WOMEN: key_maintenance. MEN: small jar. A damaged door between them wants Repair (wood x2)  0
3     cloakroom    rest      stove (fire). wood x2 in a coat locker                    0
4     maintenance  puzzle    the breaker: grants state `lights`. First throw is safe: the room has no exhibits  0
5     arts         fight     dark only (an armour stands in its door when lit). key_floor  0.9 dark
6     science      miniboss  the Shot-Firer. page_explosion in his hand                1.0
6     science_case teach     a cracked case beside him: blast it, leaf page. No enemies once he is down  0
7     stores       bosskey   lit AND Explosion: blow up the armour in the door. key_attendant  0.4
8     magic        boss      behind the painted-over arch (Explosion). The rotunda. The Attendant  boss
9     magic_case   reward    big jar, key_forest, a portrait with the face scratched out
-     natural      side      natural history: a plinth puzzle (lure the bear onto a second plate). small jar  0.5 dark
-     sewer_stair  side      "To Sewer" (2020): a grate that opens from below, from the pipes. The night door

edges
atrium -open-> history | cloakroom
atrium -key toilets-> toilets          atrium -key maintenance-> maintenance
atrium -state lights=dark-> arts       atrium -key floor-> science
atrium -state lights=lit, also verb explosion-> stores
atrium -verb explosion (bricked arch)-> magic, key attendant, lockin
magic  -oneway opens_on museum_cleared-> atrium   shortcut: the fire door
```

**Rooms.**

| Room | Teaches or tests |
| --- | --- |
| Atrium | The plan, and four leads. The arch is the tease: plaster, and a draught through it |
| History | Exhibits hold things. The boat is a chest |
| Toilets | Repair again: an old verb stays useful. The key shuffle from 2020, kept whole |
| Maintenance | The state. She throws it, hears the building change, and can throw it back at once |
| Arts | The dark. Armours walk slowly and hit hard; stuffed foxes are fast. The key is on an empty plinth |
| Science | The mini-boss, then the first blast in safety |
| Stores | The "aha": neither state opens it. The verb and the state together do |
| Rotunda | The boss |

**Enemies.** New rows, all as exhibits (a prop def and a unit def each):
`armour` by analogy `soldier` (slow, heavy; the `soldier` sprite with a steel palette will do for a first pass). `stuffed_fox` by analogy `rat` with `run` 1.3 (new small sprite). `waxwork` by analogy `skeleton` (palette swap). Existing `bat` in the roof spaces. `baseHeat` 7.

**Mini-boss: the Shot-Firer.** A waxwork miner in the Goldskin Mining gallery ("THE MODERN METHOD. Presented by the Company."). Lit, he is a prop holding the page in a closed wax hand. Dark, he is a unit. By analogy `cactus` (stationary caster) with `walk` 0.3: he lobs charges, a `ground` spell that lies for two seconds then bursts (radius 1.5 m). The floor tells her where not to stand. He is fought in the dark by necessity, which is the lesson of the wing. He drops `page_explosion`.

**Boss: the Attendant.** By analogy `headmaster`, 1,500 HP at phase 2, with a ring of keys. The rotunda has four armours on plinths and a breaker of its own on the far wall. He fights in the light. At 75%, 50% and 25% he throws the breaker (`phases[n].onEnter`): dark, the four armours step down, and he is hard to find. Throw it back and they freeze on their plinths and he is `dazzled` for five seconds (speed 0). In each lit window she chooses: hit him, or **blow up a frozen armour so it never steps down again**. Explosion is on a six second cooldown, so it is one or the other. A player who spends the first two windows on armours fights the last phase against none. Without Explosion the fight is four armours three times over: possible, long, and what K3 means by "three times easier".

**Explosion in engine terms.** Spell `explosion`: kind `bolt`, school `blast` (new), a slow lobbed charge (speed 2.5, range 8 m, cooldown 6 s, mana 30), full damage in a 28 px splash. A prop that `answers: "blast"` within the spell's `touch` radius of the impact runs its `use` list. Rows: `rubble` 3 x 2 (hide self), `cracked_wall_h` / `cracked_wall_v` 3 x 1 and 1 x 3, sealing a corridor like a gate (hide self), `exhibit_*` (hide self, set `gone` flag), `cracked_case` (unlock self: it is a chest). For a cave-in too big for a prop, `fill` a named rect from `Rubble` to floor; `tileDeltas` saves it.

**Keys and rewards.** Toilets, maintenance, floor and Attendant's keys (new items). `key_forest`. Big jar. Small jars x2. Leaf pages x2 (science case, and one behind a lit-only plinth in History). **Rest:** the cloakroom. **Shortcut:** the Magic wing's fire door to the atrium.

**What Explosion re-opens.** The rock over the cave mouth in the Lowfields (`PLAN.md`). The mine's powder store. The cellar wall behind the roses. Quarry Steps. The grate between the pipes and the Factory.

**At night.** The front door does not open after nine (`nightLock`: "Open ten to four. The door is not locked. It is just not open."). The way in at night is up the sewer stair from the pipes, once those are open. After the bell the breaker will not hold: it trips back to dark within a few seconds (`if night`). The night Museum is the dark Museum throughout, with no relief. The lit-only plinth in History cannot be reached at night; one dark-only case in Arts holds better loot. Day and night are both worth a visit, and neither is required.

**With 2 to 4 players.** One stands at the breaker and one crosses a wing: lights off to clear a doorway, lights on to freeze what is chasing her (`lure_and_lever`). It is the best co-op toy in the game and it is a split, so it is priced: every throw to dark also wakes one stuffed fox in the corridor by Maintenance. The breaker-holder must not linger. In the rotunda, one fights and one runs for the breaker. Natural history's two plinths are `twin_hold`.

**What could go wrong.** `show` putting a solid exhibit on top of a player standing on a plinth: the engine must nudge her to the nearest free cell (ask E12), and C12 keeps mission-critical plinths out of any rect she must stand in. A friend flipping the lights while another is in a lit-only room: she is shut in until it is flipped back, which is a prank, not a soft-lock, because the breaker is reversible and the solver proves every state can reach it. The exhibit list must be one generated `if` list per breaker, never hand-written, or an exhibit will be forgotten in one direction.

---

### 3.3 Butterfly Forest (phase 3): Grow

**Fiction.** Past the library the lane runs into hedges that were once somebody's garden and are now a wood. There are no doors in Butterfly Forest and nothing is locked. There is only where the light comes down, which is where things grow, and where it does not. A council sign at the gate says the forest is open from sunrise to sunset and that visitors are counted in.

The butterflies are the only colour in it. They go to flowers in the sun and they are not shy of people: they will sit on a sleeve, not for long. A man has a hut in the middle with a net by the door and glass cases on every wall. Each case has a label. The labels are trades, not species.

**Where Grow is learned.** At the ruined library, as 2020 and `PLAN.md` have it: "only page left in the book". The library is this dungeon's teach node in a zone of its own (four rooms, no enemies, heat 0): the page, a dry planter to try it on, a small jar behind the vine it grows. The Forest's def lists `grow` in `givenVerbs` and the solver holds it to that.

**The idea.** *No doors. Light is the lock and a living thing is the key.* 2020: "No keys, No doors, Forest dungeon", eight butterflies, a boss summoned with five. The zone is outdoors (`indoor: false`): hedges (`Hedge`: solid, blocks sight) are the walls, glades are the rooms, the stream is the long way round. Grow works only in light ("Nothing here will grow" in shade, which `worldVerb` already says). By day the sunbeams fall in fixed glades. It does three things:

| Grow on | Does | Engine |
| --- | --- | --- |
| A **bud** in light | Blooms. A butterfly that can see it comes and settles | `answers: "grow"`, `use`: `send` butterfly to the bud's mark |
| A **vine root** on a bank | A bridge across the stream | `fill` a named rect with `GrownPath` (`tileDeltas` saves it) |
| A **hedge seed** in a gap | The gap closes | `fill` with `Hedge`. It opens nothing; it takes away somewhere to run |

Grow that closes as well as opens is the fresh part. A closed gap shortens the fast butterfly's loop, pens the boar-sized things in the guarded glade, and turns a crossroads into a corridor. C1 is run with every hedge seed grown, and every critical node must still be reachable, so the player can never wall herself out.

**Mission graph.**

```
order node        kind      holds                                                     heat
0     gate        entrance  the council sign (notice: a plan of "the gardens" as they were). Fire just outside   0
1     first_glade teach     butterfly 1: a bud in a beam. It settles. She has no net. It leaves       0
2     hut         miniboss  the Collector. Drops the net. The cases. A cold hearth: rest after he is down   1.0
3     ring        hub       a ring of glades round the hut, the stream through it. Six leads at once  0.5
4a    settler     puzzle    butterfly 1 again, now with the net                        0.3
4b    rock        puzzle    butterfly 2 in a hollow behind a rock: Explosion           0.5
4c    runner      puzzle    butterfly 3, fast, on a loop with three gaps: hedge seeds, or friends in the gaps  0.4
4d    island      puzzle    butterfly 4 across the stream: a vine root, or a broken footbridge (Repair, wood x2)  0.5
4e    guarded     fight     butterfly 5 in a glade of undead flowers and a cactus      1.0
4f    hollow      puzzle    butterfly 6 in a short cave behind rubble; it glows        0.6
5     stone       boss      the summoning stone. Five butterflies released: the Emperor comes down  boss
6     reward      reward    big jar. The way on: Grow itself gates the Works road
-     seed_tree   side      butterfly 7: "burn a tree so it drops seeds". Needs Fire. A later visit
-     lamp_glade  side      butterfly 8, a moth, night only, comes to a lit lamp. Needs Electric or Fire. A later visit
```

Any five of the six open the boss: a hub with a real choice (K1). All eight give the Butterfly Amulet, which needs two verbs the player does not have yet, so the Forest is finished on a return visit (K2 applied to a whole dungeon).

**Butterflies** are units, new row `butterfly`: faction `friendly`, `run` 1.6 (faster than a walk, slower than a sprint), a patrol of perch marks with a `dwell` at each (ask E11), a small glow. USE on one opens a one-line dialogue whose option needs `hasItem net` and runs `despawn`, `give butterfly`, `flag butterflies add 1`. Without the net the line is: "It sits on your sleeve. You have nothing to keep it in, and it seems to know." Row `moth` is a palette swap, night only.

**Enemies.** Existing rows: `flower`, `cactus`, `pumpkin` (patrolling the rides), `spider` and `wall_spider` (night, and the hollow), `bat`. No new common enemy. `baseHeat` 8.

**Mini-boss: the Collector.** By analogy `bandit` (melee and a ranged spell). His ranged spell is a thrown net: a `ground` effect, `webbed` for two seconds. He guards the tool, not the verb, because the verb was taught in the library; he is here so that butterflies are seen and lost before they can be kept (K2). New art.

**Boss: the Emperor.** A great pale moth, 2,000 HP at phase 3, by analogy `bat` at boss scale: fast (run 1.8), swoops (`melee_fast`), and sheds scale dust (a `ground` effect that slows, new effect `dusted`). It is very hard to hit on the wing. The glade has eight buds, five of them in beams by day. Grow a bud: the bud's `use` list `send`s the Emperor to it, and on arrival it feeds, `stunned` for six seconds. That is the window. Each bud blooms once. Bolts still land between windows for a patient player, so the fight cannot be lost to running out of buds. At night only three buds are lit (moonbeams fall elsewhere) and it is a harder fight: **Question for John** whether the stone should refuse at night instead.

**Grow in engine terms.** Spell `grow`: kind `world`, `world: "grow"` (the code path exists in `worldVerb`), cooldown 10 s, mana 20. Light gating is one rule in `worldVerb`: a prop that answers `grow` is refused unless it is `litAt` (ask E8). Templates place buds in beams. Sunbeams are light rows with a rect and a day-only flag. A bud in shade is a reason to come back with a light of her own.

**Keys and rewards.** No keys. The net (item, bound). Butterflies x8 (story items). Big jar. Small jars x3 in dead-end glades. Leaf pages x2. The Amulet at eight: mechanically a +6 Spirit page and a flag the ending can read. **Question for John:** what the Amulet is for is `STORY.md`'s to say. **Rest:** a charcoal burner's fire outside the gate; the Collector's hearth in the middle once he is down. **Shortcut:** a vine root beside the hut bridges the stream straight back to the gate.

**What Grow re-opens.** Dry beds that become paths (`PLAN.md`). Gaps in the Long Hedge (closing one pens the Top Field's scarecrow side off the road). The cellar roses. The collapsed culvert on the road into the Works, which is how the Forest gates the next region with a verb and not a key.

**At night.** The butterflies roost and cannot be caught. Moonbeams fall in different glades, so different buds are live and the buds she bloomed by day have closed. Spiders come down. The moth flies. The sign said sunrise to sunset.

**With 2 to 4 players.** The runner: friends stand in the gaps instead of growing them shut (`plate_or_friend` in spirit). The guarded glade: one draws the flowers' attention, one nets. The Emperor: one grows buds and one hits. The ring invites a split into six glades; the patrolling pumpkins on the rides are why it should be brief.

**What could go wrong.** `fill` with a solid tile under a unit: the action must skip occupied cells and try again next tick (E12). `send` to a bud the Emperor cannot path to: the template keeps a 3-cell clear ring round every bud. Butterflies wandering out of the load ring: their perches are all inside one glade. Outdoors, the solver floods a zone with no walls at its edge: the forest zone is ringed with `Hedge`.

---

### 3.4 The pipes and the Factory (phases 3 and 4): Electric

**Fiction.** The pipes under Castle were laid by the company to carry water down from the hill and something else back up. The manhole covers are stamped G.M.Co. and open from below. The town's side is dry most days. The Works' side is not, and the valves that decide it are down there with whatever else is.

The Factory is semi-abandoned, which is how the council put it. The shift never clocked off. It is dark inside because the foreman had the lamps taken out, run by run, and wrote down why: the machines only mind what they can see. His diary stops on a Tuesday. The generator is still warm. There is a glass orb wired into it, on a stand that belongs in somebody's kitchen.

**The idea.** *Light is how the machines see you.* Everywhere else in the county a lamp is safety. In the Factory a lamp is a sentry's eye: sentries (`sight: "lit"`) fire only at what stands in light. The building is crossed twice. First in the dark, safely, to the generator. Then the verb is found, the generator is started, **every lamp in the building comes on**, and the way back is the same halls with every sentry awake. Electric is how she takes the building back: a `spark` into a fuse box kills that hall's lamps for good. But the shutters on that hall's circuit die with them. Dark is safe and shut. Lit is open and watched. Each hall is a choice about which she needs.

It is the Museum's breaker turned inside out (there, dark was the danger) and the county's rule turned inside out, which is why it belongs to the Works. 2020's notes are all here: "Lights come on when power on", "avoid robot los puzzle", "Drop in to genorator room through vent", robots "too powerful" to fight that Electric can "control... and smash the door".

**The pipes** are a zone of their own at phase 3, and the approach to the Factory, not a separate dungeon. Eight to ten rooms of culvert, generated from the same lattice with 2 x 1 tunnel templates. One state variable, `east_valve` (`dry` / `flooded`): a lever that `fill`s one named run with `DryBed` and another with `Water`. No boss. `rat`, `spider`, `wall_spider`, `bat`. A small jar, a leaf page. Their lasting value is the point: every manhole ladder Repaired from below (iron x1) is a door to the surface that stays open, so the pipes become a covered road between the town, the Museum's sewer stair and the Factory yard in a county with no fast travel. They are not safe. They are out of the night.

**Mission graph (Factory).**

```
order node        kind      holds                                                     heat
0     sump        entrance  up from the sewer through a grate (Explosion). Dark        0.2
1     loading     teach     one lamp left burning over the far door, one sentry. Step into the light and learn   0.3
2     lockers     key       key_generic x2. The foreman's diary, page one (notice: the plan of the works)        0.3
3     line_a      fight     dark. Rats, bats. Haulers on patrol, which cannot be fought, only avoided           0.7
4     time_office rest      the clocking-in room. A stove. Cards in the rack, all stamped for today            0
5     vent        puzzle    a one-way drop (2020). A way-in prop: friends can follow. No way back up            0
6     generator   miniboss  the Charge Hand. The orb on its stand: learn spark                                  1.0
6     gen_shutter teach     a dead socket beside the hall's shutter: spark it, the shutter opens. No enemies     0
7     power_on    verb      Repair the generator (iron x4, in the room) and spark it: state `power` on. Everything lights  0
8     line_a again fight    lit. Sentries awake. A fuse box at the near end, in plain sight: the second lesson   1.1
9     press_hall  puzzle    haulers, call boxes, a weak wall. Spark a call box: the hauler walks through the wall  0.8
10    office      bosskey   the foreman's office: key_foreman, the rest of the diary, a leaf page               0
11    assembly    boss      the Foreman                                                                        boss
12    roller_door reward    big jar, key_burial. A socket opens the roller door to the yard for good: the shortcut
-     stores      side      behind a shutter that needs power and a hall that needs dark: fuse one hall, not the other. Small jar
-     roof_vent   side      a second way in from the yard, by a vine root (Grow). Joins at lockers
```

`power` is a one-way state (off, then on for good), so it is a flag, not a `StateVar`. The per-hall lit and dark states are monotone too (a blown fuse stays blown). The solver needs nothing stateful here, only verb gating. The cleverness is all in where the fuse boxes and shutters are, which is template work.

**Rooms teach, in order:** light is sight (loading); haulers are weather, not enemies (line A); commitment (the vent); the verb in safety (the shutter); the reversal (power on: a sound first, K23, then the lamps come on in a wave from the generator outward); fuses (line A again, the same room made new, K2); control (press hall).

**Enemies.** New: `sentry`, by analogy `statue` (stationary, ranged), with `sight: "lit"` and a bolt of school `shock`. `hauler`, by analogy `lurker` (so strong that fighting is not the answer), melee only, on a patrol, with `sight: "lit"` as well. Both need new art. Existing: `rat`, `bat`. `baseHeat` 9.

**Mini-boss: the Charge Hand.** The shift supervisor, by analogy `headmaster` with a `shock` melee. He fights in the generator hall, which is dark. He carries a hand lamp (`glow`), so he is the only lit thing in the room, and he is what the hall's one dormant sentry can see: lead him past it and it fires at him. The lesson of the dungeon, turned on its teacher.

**Boss: the Foreman.** 3,000 HP at phase 4. Plated: `resist` 0.7 to physical, frost and fire, and -0.5 to `shock`, so the verb hurts him more than anything she owns. The assembly hall is lit, with four sentries on gantries that serve him, four fuse boxes, and four floor grids each with a socket. Pattern: he walks her down; the sentries fire at anything lit. Blow a fuse and that quarter goes dark: its sentry sleeps, and he cannot see her in it either (`sight: "lit"`), so he stops, turns, and walks to the nearest breaker to reset it. That walk is the window. Spark the socket of the grid he is standing on: `strike` for shock damage and `jolted`. At 66% and 33% he resets every fuse at once (`onEnter`). With a friend: one draws him over a grid, one sparks the socket. Without Electric he is a long fight under fire from four sentries: possible with Stone Skin and patience, and miserable, as intended.

**Electric in engine terms.** Spell `spark`: kind `bolt`, school `shock` (new), fast (speed 7), range 12 m, cooldown 1.5 s, mana 12, effect `jolted` (0.5 s stun on anything with `resist.shock` below 0). Props that `answers: "shock"`:

| Prop | `use` list |
| --- | --- |
| `socket_dead` | `unlock` the shutter it feeds; `lightWhenOn` |
| `fuse_box` | `hide` each lamp prop on its run, `lock` each shutter on its circuit, `flag` |
| `call_box` | `send` its hauler to the mark behind the weak wall, `then`: `hide` the wall, `shake`, and the hauler stays where it stops |
| `generator_cold` (shown when `broken_generator` is Repaired) | `flag factory_power`, `show` every lamp, `unlock` every powered shutter, `shake`, toast |
| `grid_socket` | `strike` the grid's rect |
| `relay_box` (county) | `switch` on every `lamp_dead` of its run; `flag lamps_<run>` |

A lit lamp cannot answer a bolt (`schoolTouch` skips props that are `on`), which is why the fuse box is a separate prop. It is also better fiction.

**What Electric re-opens.** **Dead lamp runs, for good** (`PLAN.md` section 2.6): each failing run on the east and north roads has a relay box at its head; one spark and the run stays lit every night after, the road keeps its -1 threat, and the walk home is changed for the rest of the game. This is the largest single reward in the design and it is the verb, not loot. Also: the signal at Castle Halt (the Closed Line). The Museum's breaker, which now holds through the night. The moth's lamp in the Forest.

**Keys and rewards.** Plain keys x2, `key_foreman`, `key_burial`. Big jar. Small jars x2. Leaf pages x2. The diary (three notices, each one fact short). **Rest:** the time office; the works canteen outside (`sites.json`). **Shortcut:** the roller door. **The cellar study** pays off here: the stand is the same stand.

**At night.** Indoors, so the halls do not change. The yard does: the Cooling Yard is threat 5 and +2 at night in the Works, so arriving by the pipes is the sensible way after nine, and the roller door opens onto something she may not want to walk out into. **Omen:** "The day shift clocks off at six. If true: at six the haulers walk to the time office and stand there until seven." An hour a day when the press hall is empty and the rest room is not.

**With 2 to 4 players.** A carried lamp would be the obvious toy and is not asked for. Instead: the Charge Hand is kited past the sentry by one while the other stays dark. In line A lit, one draws fire at the mouth of the hall while one runs for the fuse box: a split of one room and ten seconds, which is exactly K19. The Foreman: lure and socket.

**What could go wrong.** `litAt` must be cheap and deterministic: prop lights only (radius, `on`, not hidden), from the 16-cell buckets, no renderer state. The player's own bolt glow must not count as lighting her. A hauler `send`-ed somewhere it cannot path: the press hall template guarantees a 5-wide lane from every hauler patrol to every weak wall, and `send` gives up after a budget and says so. The vent is a same-zone `to`; the solver must treat it as one-way, and C6's follow rule covers it.

---

### 3.5 The Burial Chamber (phase 5): Fire

**Fiction.** The burial ground is older than the town and was full before Goldskin bought a place in it. The stair goes down to a square hall with a passage off each side. It is cold, and it is the kind of cold that comes off something. The torches burn blue if they burn at all, and they show things the ordinary kind do not.

He was buried with what he liked to look at, and with company. The four he trusted went into the four corners. One kept snakes. One loved her garden. The stones say so. They do not say why the four dates are the same, or who has been bringing the soldier fresh flowers.

**The idea.** *Cold light shows what is there. Warm light keeps it off.* Two kinds of flame, two schools. **Cold torches** (`torch_blue`, built: they answer frost) reveal: lighting one `show`s what was hidden near it: a door, a chest, the far half of a floor. **Braziers** (new: they answer fire) protect: the dead of this place (`shade`, `shunsLight`) will not step into warm light. And 2020's note, "Large Torch can push": the **great torch** is a pushable prop with a warm light. Pushing takes thirty ticks and twenty energy a cell. It is a rest room she moves one cell at a time through a hall full of things standing at the edge of its light. That is the image of this dungeon, and it is one prop row and one AI flag.

**Form.** 2020 drew START in the centre and a boss in each corner, the wizard opened by all four. It is the one dungeon where the order is the player's (K1, in full). Two corners are open from the start. Fire is found in one of them, and opens the other two.

```
order node           kind      holds                                                        heat
0     start          entrance, hub   `exit_door`, rect `burial_entry`. A fire (rest). Four passages. `gate_wizard`, seen and shut   0.3
      -- the Snake's corner (west and north-west): built. Needs Icebolt and bait --
1a    alcove         puzzle    `torch_a`, `torch_b` -> `torch_chest`                          0
1b    rat_room       key       `snake_key_chest`. Rats, for meat                              0.4
1c    lockin         fight     `gate_snake`, rect `lockin_room`, marks `lockin_a`..`d`, `giant_key_chest`   0.8 lock-in
1d    snake_room     boss      `burial_snake`, rect `snake_arena`, `snake_gate_east`, `snake_gate_south` (the way back, built)  boss
      -- the Gardener's corner (south): half built --
2a    east_hall      puzzle    `lurker_a`, `lurker_b`: fed, not fought. Statues. `lurker_chest`  0.6
2b    garden         miniboss  rect `garden`, `garden_flower` holds `root_a`..`c` shut. `fire_scroll`: learn fireball   0.8
2c    orchard        teach     dead dry trees (2020) across the path: burn one. No enemies. Small jar   0
2d    glasshouse     boss      the Flower                                                    boss
      -- the Spider's corner (north-east): needs Fire --
3a    web_hall       puzzle    web walls across every passage: fire burns them. Wall spiders   0.9
3b    nursery        boss      the Spider                                                    boss
      -- the Soldier's corner (south-west): needs Fire --
4a    dark_hall      puzzle    the great torch, shades at the edge of it, braziers to light on the way   1.1
4b    parade         boss      the Soldier                                                   boss
5     gate_wizard    bosskey   a `while` trigger on four flags unlocks it. Four flames light in the start hall, one per corner   0
6     vault          boss      Goldskin                                                      boss
7     stair_up       exit      the Ball. A stair that goes up, not out: into the School's boiler room
```

Every name in today's `CONTRACTS.burial` is kept. What is built (cold torches, the lock-in and its chest, the snake's clock, bait, the root wall and the scroll) is re-tuned to phase 5 by setting the def's phase, and nothing else.

**Enemies.** Existing: `soldier`, `skeleton_guard`, `lurker`, `statue`, `spider`, `wall_spider`, `flower`, `cactus`, `pumpkin`, `rat`, `bat`, `burial_snake`. New: `shade`, by analogy `soldier`, with `shunsLight`: it paths round any cell that is `litAt` by a warm light and waits at the edge. A `soldier` sprite drawn in two dark tones will do. `baseHeat` 10.

**The corner bosses.** Each has a way back to the hall that opens on its death, as the snake's south gate does today.

| Boss | Pattern | By analogy | The verb |
| --- | --- | --- | --- |
| **The Snake** (built) | 900 ticks of pursuit, then home to spit rings. Resets if a wall breaks the line | itself | Icebolt's chill slows the pursuit. Not a Fire fight, on purpose: it is the corner open before Fire |
| **The Flower** | Rooted. Throws seeds that crawl a short way and bloom into `flower` units (2020: "seeds that crawl and bloom"). Thorn ring at close range | `cactus` at boss scale, plus `spawn` on a cooldown | Burning (`burning`) kills seedlings before they bloom and is the only damage over time that outpaces its regeneration |
| **The Spider** | Lays eggs that hatch every 20 s; web-wraps her target at 6 s (2020's numbers, from `MISSING-SYSTEMS.md`) | `wall_spider` at boss scale | Fire burns eggs and webs. A wrapped friend is freed by a fireball at her feet |
| **The Soldier** | A parade ground, dark, four braziers. He is slow and cannot be hurt much from the front (`resist` 0.8). Shades pour in from the dark | `iron_knuckles` | Light the braziers and the shades stop at the edge. The great torch, pushed behind him, makes him turn |

**Boss: Goldskin.** 4,000 HP at phase 5. Never seen before this room (`STORY.md` section 5). Gilded: `resist` 0.8 to everything until fire touches him, then `softened` for six seconds (no resist). Fire literally opens him. His hall has four braziers and four cold torches. At 75%, 50% and 25% he puts every flame out (`onEnter`: `switch` off; a brazier that is off can answer a fireball again). In the dark, shades rise at four marks, and he himself can only be seen inside cold light. So each phase asks one question: warm light first, to be safe, or cold light first, to find him. He casts what the dungeon has already taught her to read: slow poison globes, a ring like the snake's, an icebolt. On death: the Ball, a big jar, and the stair up.

**Fire in engine terms.** `fireball` exists (bolt, school `fire`, splash, `burning`). New props that `answers: "fire"`: `brazier` (`lightWhenOn`, warm), `web_wall_h` / `web_wall_v` (seal a corridor like a gate; hide self), `dry_tree` 2 x 2 (hide self; one of them drops seeds for the Forest's seventh butterfly), `cold_hearth` (show a `campfire` in its place: a new rest point). New prop `great_torch`: 2 x 2, `push`, a warm light of radius 84.

**What Fire re-opens.** Bramble over lane mouths. **Cold hearths**: unlit fires all over the county, each of which becomes a rest point for good once lit. In a game where the distance between two fires is the length of a run (`PLAN.md` section 2.5), this is Fire's equal to Electric's lamps. The Forest's seed tree. The moth's lamp.

**Keys and rewards.** `key_snake`, `key_snake_boss` (built). Four corner flags. Big jar (Goldskin) and four half-jars (+7) from the corners. Leaf pages x3. The Ball (story item, bound). **Rest:** the fire in the start hall, which is the right depth for all four corners because it is the hub. **Shortcuts:** the four return gates.

**At night.** Indoors. But the stair is off the road and not visible from it (`sites.json`), at threat 5 with +2 at night in the Works. Getting there is the night's part of this dungeon. **Omen:** "Somebody tends the soldier's grave. If true: fresh flowers each morning, and between three and four at night a `friendly` unit stands in the parade ground who will not speak and leaves if approached."

**With 2 to 4 players.** Four corners invite a four-way split, and at 38% each that is how a party dies, so the hall says it plainly: one fire, in the middle. The great torch is `guard_the_pusher`: one pushes, the rest stand in its light and fight outward. Goldskin: one keeps the braziers lit, one keeps the cold torches lit, the rest hit what that shows them.

**What could go wrong.** `shunsLight` and A*: a shade whose goal is inside light must path to the nearest dark cell and wait, not thrash. The great torch pushed into a dead end is recoverable (pull exists), but the template keeps every great-torch hall free of 2-wide necks. A cold torch that `show`s a solid prop needs E12. The snake's line-of-sight reset must keep ignoring pillars after its room is rotated by the generator: its template is a set piece (K14), mirrored only.

---

### 3.6 The School (phase 6): the end

**Fiction.** Castle School stands on the crown of the hill with one window lit. It can be seen from the station platform and from most other places, and nobody in town looks at it. The bell rings at nine. The timetable is posted inside the front door and is kept up to date.

The building follows the timetable. At the bell, rooms that were open are closed, and corridors go somewhere else. Lessons are still set: Woodwork, Chemistry, Botany, Physics, Domestic Science, and one period marked only with a snowflake. The register in the top room has every name in the county in it, in one hand. The last name is {name}, and the ink is not dry.

**The idea.** *The building follows the timetable.* The bell that has told the whole county which town it is in (K23) is, here, a rope in the hall that she can pull. No new verb: the School asks for all six.

Two state variables, and the clock:

| State | Changed by | Does |
| --- | --- | --- |
| `period` (`lessons` / `break`) | The bell rope in the assembly hall, on the hub | In lessons the classrooms are open and the corridors are patrolled by staff. At break the classrooms are shut, the corridors are empty, and the yard, the kitchens and the boiler room are open |
| Night (the real bell at nine) | The clock. Or the sick bay: one bed sleeps until six as beds do; the other is labelled "You will be woken at the bell" and sleeps until nine in the evening (`rest` with `until: 21`) | After nine the timetable on the wall is a different timetable: every room is open, every lamp is out, and the top corridor exists |

The bed is a lever. That is the only new trick, and it needs no engine work: `rest` already takes `until`, and a bed already only turns the clock when the whole party is resting, which makes it a regroup beat in co-op for free.

**Mission graph.**

```
order node         kind      holds                                                       heat
0     boiler       entrance  up the stair from the Burial. (The front doors open from inside later: the shortcut to the Bellfield)   0.4
1     hall         hub       the bell rope: grants state `period`. The timetable (notice). Six doors and a tower stair, seen and shut   0
2     sick_bay     rest      two beds. A fire in the grate. Nothing comes in                0
3a    woodwork     puzzle    Repair: a run of broken benches to cross a flooded floor; wood in the racks. Lessons only     0.8
3b    chemistry    puzzle    Explosion: sealed fume cupboards, one of them the way on. Lessons only                        0.8
3c    botany       puzzle    Grow: a glasshouse in daylight: buds, a vine, a gap to close. Lessons only, and by day only   0.8
3d    physics      puzzle    Electric: a board of dead sockets and one live feed; light the room and the sentry-like demonstration model wakes. Break only  0.9
3e    domestic     puzzle    Fire: cold ranges that are braziers; shades in the pantry. Break only                       0.9
3f    ice_house    puzzle    Icebolt: cold torches in the yard's ice house show the floor that is there. Break only         0.7
4     caretaker    miniboss  the Caretaker, in the corridors at night only. Carries the tower key                        1.2
5     tower        boss      `while` six lesson flags and the tower key. The top room. The Ringer                        boss
6     top_room     reward    the register. The lit window. The choice (`STORY.md` section 4) is made here or carried out of here
-     staff_room   side      leaf page. Night only
-     lost_property side     small jars x2. Break only
```

Each lesson ends at a classroom clock that she stops (a `once` prop: `flag lesson_<n>`). Six stopped clocks and the Caretaker's key open the tower. The lessons may be done in any order, inside the constraint of which period and which hour each needs, and the timetable on the wall tells her that constraint truthfully. Planning a day round a timetable, in a building that punishes being in the wrong corridor at the wrong time, is the last exam of a game about being home before dark.

**The solver** treats this as a stateful flood over `period` x `night` (four layers). Night is reachable if a bed labelled 21 is reachable; day if a bed labelled 6 is. Both are in the sick bay.

**Enemies.** Everything the county has, at phase 6: `soldier`, `skeleton_guard`, `shade`, `sentry`, `wall_spider`, `bat`. New: `master`, staff in a gown, by analogy `bandit` (a caster that closes to melee), patrolling corridors during lessons. **The children are never shown and never enemies** (`STORY.md` section 7.5): there are coats on pegs and chairs pushed back, and that is all. `baseHeat` 12.

**Mini-boss: the Caretaker.** By analogy `skeleton_clerk` at boss scale: he carries every key, walks a long loop of the night corridors with a lamp, and is heard (keys) before he is seen (K24). He does not fight to the death: at half health he puts out his lamp and walks away into the dark, and must be found again. He tests Electric and Fire at once: lit corridors show him, and he cannot put out a brazier.

**Boss: the Ringer.** 5,000 HP. **Question for John:** who or what rings the bell is `STORY.md`'s open question, and this document does not answer it. The name is a placeholder and the fight is written so that the answer can change without the fight changing. The belfry is round. The bell tolls on a fixed count, every twenty seconds, and each toll does two things: a ring of bolts outward from the centre (the snake's `snake_ring`, at scale), and the room changes face, day to night to day (`onEnter` cannot do this; it is a timer, so the Ringer takes the snake's route: a small custom clock in its controller, which would be a second exception to the one-custom-mover rule and needs agreeing). By day the Ringer can be hurt and the floor is plain. By night it cannot be seen outside cold light, shades stand at the edge of warm light, dead sockets want sparking to keep two lamps burning, and buds in the moonbeam from the one window bloom into cover that stops the next ring. Every verb has one job. None is required; each makes one toll survivable.

**Rewards.** Big jar, and the ending. **Rest:** the sick bay. **Shortcut:** the front doors, opened from inside, onto the Bellfield (threat 6) and the road down.

**With 2 to 4 players.** The timetable splits parties by temptation: three lessons are open at once and the hall is between them. The corridors during lessons are why that is a bad idea and why break is the time to move. The belfry gives every player a verb to mind.

**What could go wrong.** Four state layers times a 6 x 5 lattice is still under a million flood cells, but C1's ablations multiply it: run ablations on the room graph, not on cells. A party asleep in the sick bay while one member is elsewhere cannot turn the clock, by the existing rule; the label on the bed should say so in voice. The Ringer's clock is custom code and needs the snake's level of test.

---

### 3.7 Optional: the Closed Line (phase 3)

**Fiction.** Beyond the platform at Castle Halt a second track goes into a second tunnel, bricked to head height, with a notice: LINE CLOSED. DO NOT ALIGHT. The signal outside it still changes. Nobody at the station will say what it is signalling to.

Inside, it is not the same tunnel twice. The diagram on the signal box wall is accurate, each time, to a different tunnel. Further in there are coloured lights that are not signals. If she stands still they come closer, and she can nearly hear the words.

**Why it earns its place.** It is on screen in the first minute of the game and cannot be entered for ten hours (K4: the goal shown early). It needs three later verbs to open (Explosion for the brickwork, Repair for the points, Electric for the signal), so it is the clearest case of the old map re-opening. And it is the only place where the premise, that Castle is undecided (`STORY.md` section 1), is a mechanism: **the layout is re-rolled on every visit.** The generator that gives every run a different mine gives this one place a different self each time she walks in, and the diagram on the wall (the notice, K5) is always right.

**Mechanism.** Five nodes: signal box (notice, a lever frame), three signals to be set to the aspect on the diagram (one by lever, one by `spark`, one by Repair and then lever), and the far end. The mission never changes; the embedding does. Engine: the zone's build seed takes a salt from a flag that counts her entries (ask E15). Zone state for this one zone is discarded on exit. **Enemies:** `bat`, `rat`, and `wisp`, a friendly drifting light (new, tiny art) that does nothing but follow at a distance. **Reward:** two leaf pages, a small jar, and a line of the song. No boss, no rest room, five minutes. **Co-op:** the lever frame and the signals are three places at once, which friends can man; alone she walks between them. **Risk:** a save made inside it must store the salt it was built with, or a reload builds a different tunnel round her.

### 3.8 Optional: the Ice House (phase 3)

**Fiction.** The council's sign at the landing says NO BATHING and, under it, NOBODY HAS DROWNED IN THIS LAKE. The statue stands a hundred yards out with its back to the shore. There was a causeway once. The water over it is black and quite still, and it takes the cold very well.

Under the statue is an ice house, built by the old families to keep what needed keeping cold. Its door faces away from the town. Whatever was stored there was not food: the shelves are the wrong size.

**Why it earns its place.** Icebolt is the first verb and the only one with nothing to open on the surface but torches. This gives it a world use from the first hour: **still water** (a flat prop over `Water`, `answers: "frost"`) freezes into a walkable `Ice` tile by `fill`, for good. The statue in the lake is a required site with nothing under it, and the lake's omen needs somewhere to be checked.

**Mechanism.** *She freezes her own way across.* Six rooms of black water with still-water props in chains. Mana is the key: each freeze costs an Icebolt, there are more pools than one bar of mana covers, and the vials on the shelves are counted (C4 treats mana vials here as a material). Wrong turns cost crossings. **Enemies:** `statue`, `bat`, `spider` on the walls. No boss. **Reward:** a big leaf page (+6), a small jar. **Surface:** still pools in Sallow Bottom and the Eel Beds hide two jars and a short cut. **Co-op:** two mana bars freeze twice as far, which is a plain and honest advantage. **Risk:** `fill` to `Ice` under a swimming thing: nothing swims. If the lake omen is true that seed, shapes are drawn under the ice (renderer only) and nothing else changes.

---

## 4. ASKS

Everything this design needs that does not exist today. "Art" means a new text-grid sprite. Where a first pass can reuse a sprite, it says so.

### 4.1 Tiles

| Tile | Flags | For |
| --- | --- | --- |
| `Sill` | walkable; pushed props cannot enter (new flag `F_NOPUSH`) | Generator: barrels never leave their room |
| `Glass` | solid, not sight-blocking | Museum cases and sight edges |
| `Hedge` | solid, sight-blocking, outdoor | Butterfly Forest walls; Grow's closed gaps |
| `Ice` | walkable | Still water frozen by Icebolt |
| `MuseumFloor`, `MuseumWall` | indoor; wall solid and sight-blocking | Museum |
| `PipeFloor`, `PipeWall` | same | Pipes |
| `WorksFloor`, `WorksWall` | same | Factory |
| `SchoolFloor`, `SchoolWall` | same | School |

Existing tiles reused: `Rubble`, `GrownPath`, `DryBed`, `Track`, `Rail`, `Fence`, `Water`, `Garden`.

### 4.2 Props

| Prop | Size | Flags | Used by |
| --- | --- | --- | --- |
| `jar`, `jar_big` | 1x1, 2x2 | `once`, prompt "Open"; `use`: `grow` strength, with the prop's own key as `id` | All |
| `leaf_page` | 1x1 | `once`, flat, prompt "Read"; `use`: `grow` spirit, same | All |
| `page_repair`, `page_explosion`, `page_grow` | 2x2 | `once`, `talk` tree with `learn`. Copy `scroll_fire` | Mine, Museum, library |
| `orb_spark` | 2x2 | Copy `orb_ice` | Factory |
| `notice` | 2x1 | Copy `sign`; `use`: `reveal` | All |
| `way_in` | 2x1 | not solid, flat, prompt "Climb in", `to` same zone | Every lock-in; the vent |
| `broken_hoist`, `broken_cabinet`, `broken_generator`, `ladder_broken` | 2x2, 3x1, 3x3, 2x1 | `answers: "repair"` | Mine, Factory, pipes |
| `rubble` | 3x2 | solid, blocks sight, `answers: "blast"` | Museum onward, and the old map |
| `cracked_wall_h`, `cracked_wall_v` | 3x1, 1x3 | same; seals a corridor | Same |
| `cracked_case` | 2x2 | a locked chest that `answers: "blast"` | Museum |
| `exhibit_armour`, `exhibit_fox`, `exhibit_waxwork` | 3x2, 2x2, 2x2 | solid, `answers: "blast"` | Museum |
| `plinth` | 2x2 | Copy `plate`, new sprite | Museum |
| `breaker`, `bell_rope`, `valve`, `hoist_lever` | 1x1 | Copy `lever`, new sprites | Museum, School, pipes, Mine |
| `gallery_lamp`, `works_lamp` | 1x1 | `lightWhenOn` | Museum, Factory |
| `bud` | 1x1 | flat, `answers: "grow"`, `lightWhenOn` (small) | Forest, School |
| `vine_root`, `hedge_seed` | 2x1, 1x1 | flat, `answers: "grow"`; `use`: `fill` | Forest and the old map |
| `summon_stone` | 2x2 | `use` behind `if` on the butterfly count | Forest |
| `socket_dead`, `fuse_box`, `call_box`, `grid_socket`, `generator_cold` | 1x1 (generator 3x3) | `answers: "shock"` | Factory, School |
| `lamp_dead`, `relay_box` | 1x1 | `answers: "shock"`, `lightWhenOn`, `nightOnly` | County lamp runs |
| `shutter_h`, `shutter_v` | 3x1, 1x3 | Copy `gate_h` / `gate_v`, new sprites | Museum, Factory |
| `weak_wall` | 3x2 | solid; hidden by a hauler's `send` | Factory |
| `manhole` | 2x2 | Copy `hatch`; `to` county | Pipes |
| `brazier` | 1x1 | solid, `answers: "fire"`, `lightWhenOn`, warm | Burial, School, old map |
| `cold_hearth` | 2x2 | `answers: "fire"`; shows a `campfire` | County, cellar |
| `great_torch` | 2x2 | solid, `push`, warm light radius 84 | Burial |
| `web_wall_h`, `web_wall_v`, `dry_tree` | 3x1, 1x3, 2x2 | `answers: "fire"` | Burial, Forest |
| `still_water` | 3x2 | flat, `answers: "frost"`; `use`: `fill` to `Ice` | Ice House, county |
| `clock_stopped` | 1x1 | `once`; `use`: `flag` | School |
| `sickbay_bed_bell` | 2x3 | Copy `bed`; `rest` with `until: 21` | School |

### 4.3 Units

| Unit | By analogy | Differs by | Art |
| --- | --- | --- | --- |
| `armour` | `soldier` | none | Palette swap of `soldier` |
| `stuffed_fox` | `rat` | `run` 1.3 | New, small |
| `waxwork` | `skeleton` | none | Palette swap |
| `shot_firer` (mini-boss) | `cactus` | `walk` 0.3; spell `charge_lob` | New |
| `attendant` (boss) | `headmaster` | three `onEnter` phases | New |
| `butterfly`, `moth` | `dog` (friendly, `npc`) | patrol with `dwell`, `run` 1.6, glow, `talk` | New, tiny; moth is a swap |
| `collector` (mini-boss) | `bandit` | spell `net_throw` | New |
| `emperor` (boss) | `bat` | boss scale, `run` 1.8, spell `scale_dust` | New, large |
| `sentry` | `statue` | `sight: "lit"`, spell `spark_ai` | New |
| `hauler` | `lurker` | melee only, patrol, `sight: "lit"` | New, large |
| `charge_hand` (mini-boss) | `headmaster` | shock melee, `glow` | New |
| `foreman` (boss) | `iron_knuckles` | `resist`, two `onEnter` phases | New |
| `shade` | `soldier` | `shunsLight` | Two-tone swap of `soldier` |
| `great_flower`, `spider_queen`, `the_soldier` (corner bosses) | `cactus`, `wall_spider`, `iron_knuckles` | section 3.5 | New x3 |
| `goldskin` (boss) | `headmaster` | `resist` 0.8, three `onEnter` phases, caster book | New |
| `master` | `bandit` | none | New |
| `caretaker` (mini-boss) | `skeleton_clerk` | boss scale, long patrol, `glow` | New |
| `ringer` (boss) | `burial_snake` (for its clock, not its body) | custom toll clock | New. **Question for John** |
| `wisp` | `dog` | glow, follows at a distance, no talk | New, tiny |

### 4.4 Spells and effects

| Spell | Kind, school | Note |
| --- | --- | --- |
| `explosion` | bolt, `blast` | speed 2.5, range 8, cooldown 6, mana 30, splash 28 px at full damage, `touch` 28 |
| `grow` | world, `world: "grow"` | cooldown 10, mana 20. The code path exists |
| `spark` | bolt, `shock` | speed 7, range 12, cooldown 1.5, mana 12, effect `jolted` |
| `spark_ai`, `charge_lob`, `net_throw`, `scale_dust` | bolt; ground; ground; ground | enemy rows |

| Effect | Does |
| --- | --- |
| `staggered` | 4 s, speed 0, takes double |
| `dazzled` | 5 s, speed 0 |
| `jolted` | 0.5 s, speed 0; only lands on units with negative `shock` resist |
| `dusted` | 4 s, speed 0.5 |
| `softened` | 6 s, removes all `resist` |

### 4.5 Items

`key_toilets`, `key_maintenance`, `key_floor`, `key_attendant`, `key_forest`, `key_foreman`, `key_burial`, `key_tower` (all `opens` tags of the same name). `net` (bound). `butterfly` (story item, stack 8). `butterfly_amulet` (bound). `the_ball` (bound). `foremans_diary` (usable: `talk`). No item for jars or pages: they are props, and growth is the world's.

### 4.6 Dialogue

Trees: `page_repair` (the confiscated drawer), `page_explosion`, `page_grow`, `orb_spark`, `butterfly` (catch, with and without the net), `foremans_diary` (three entries), `register` (the top room), one `notice_<dungeon>` each (seven), `sickbay_bed_bell`. Dog: replace the `learn repair` action in `offer_mine` with the pointer line; add one line after each dungeon that names the next place and explains nothing. Every row written against `VOICE.md`; no row names the heroine.

### 4.7 Engine capabilities

Short, and each one is used by more than one dungeon unless it says otherwise.

| | Capability | Why it is worth it |
| --- | --- | --- |
| E1 **built** | Action `{ do: "if"; when: Condition[]; then: ActionList; else?: ActionList }` | Two-state buildings (breaker, valve, bell rope) need a list that depends on a flag. Without it they are per-tick `while` triggers, which is a hack. The solver reads both branches inside the stateful flood |
| E2 | `Blueprint.triggers`: trigger rows carried by the blueprint, merged with `triggers.json` for the zone | Generated lock-ins and state gates. The solver already reads triggers by zone |
| E3 | A prop's `to` may name the current zone: move the unit to the mark, no reload | Way-ins (the co-op follow rule), the vent |
| E4 | Tile flag `F_NOPUSH`, checked in `footprintFree` on push | One line. Removes a class of soft-lock |
| E5 | Growth by finding. **Already in the working tree** as `{ do: "grow"; stat; amount; id }` with `state.growth.found` (uncommitted, seen 21 September). This design uses it as built and asks one thing of the generator: a jar's `id` is its prop key, and a generated prop key comes from node and socket (`mine_office_jar_0`), never from coordinates or attempt, so a re-rolled layout cannot hand the same jar out twice or lose one | Jars and pages |
| E6 | Action `{ do: "strike"; rect: string; amount: number; school: School; effect?: string; hitsFriends?: boolean }` | Hoists, floor grids, any trap. The first hazard verb |
| E7 **built** | Schools `blast` and `shock`. SpellDef `touch?: number` (px) replacing the fixed 14 in `schoolTouch` | Two verbs. A blast must reach the middle of a 3x2 prop |
| E8 **built** (day-only lights are props with `dayOnly`, not rects; `light.cold` added so `shunsLight` minds warm light only) | `litAt(x, y)` in the sim, from prop lights only. Light rows may be a rect and may be day-only. UnitDef `sight?: "lit"` and `shunsLight?: boolean`. `worldVerb("grow")` requires `litAt` | The Factory, the Burial, the Forest, and the game's spine: lamps that mean something to the sim |
| E9 **built** | Action `{ do: "send"; unit: string; to: string; then?: ActionList }`: walk a unit to a mark, ignoring aggro, then run a list on it. Gives up after a path budget | Haulers, the Emperor, the mine's nine o'clock omen, the six o'clock shift |
| E10 **built** | UnitDef `phases[n].onEnter?: ActionList` | Every boss that changes the room |
| E11 **built** (a third number on a `UnitSpawn.patrol` point; `patrol: [{ mark, dwell }]` on a unit holding) | Patrol points with a `dwell` in ticks | Butterflies, the Caretaker |
| E12 **built** (a solid `fill` waits for its WHOLE rect to be clear, not cell by cell: a hedge that grew round the one cell she stood on would box her in) | `show`, `lock` and solid `fill` never land on a unit: nudge to the nearest free cell, or skip the cell and retry | Exhibits, shutters, hedges. Bug safety |
| E13 **built** (a held `notice` with no `use` reveals every placed room) | Action `{ do: "reveal"; rects: string[] }`: set fog bits | The wall notice as the dungeon map. 2020 had reveal rects |
| E14 | `ZoneId` widened: `museum`, `library`, `forest`, `pipes`, `factory`, `school`, `line`, `icehouse` | Bookkeeping |
| E15 | A zone's build seed may take a salt from a flag; that zone's state is dropped on exit; the salt is saved | The Closed Line only. Skip if that dungeon is cut |
| E16 | The generator and the solver extensions of section 2 | Not sim code: `src/world/` only |

`SYSTEMS.md` permits one custom mover, the snake. The Ringer's toll clock would be a second exception and needs John's leave. Everything else above is a row or a verb.

### 4.8 Build order

1. **Verbs the mine needs:** E4, E3, E6, each with a test (E5 is already under way). Rows: `jar`, `leaf_page`, `way_in`, `page_repair`, the hoists. (Small, and playable at once in today's hand-built mine.)
   **Built, September 2026** (`test/dungeon-verbs.test.ts`): E2, E3, E4, E6 and the rows, proven in the generated mine. `notice` is a row without `reveal` (E13 is step 5).
2. **Templates:** the `.room` parser, the lint, the solver and ablation harness, the bot harness. Twelve mine pools at one variant each, cut from today's `mine.ts`.
   **Built** (`test/templates.test.ts`), **except the bot harness**: a bot plays the whole mine on three seeds instead; per-room bots are still to write. Twelve pools, two variants each, one arena (a set piece, K14). One legend entry per line, not several; rects are header lines (`rect inner 1 1 65 21`). How it works: `ENGINE.md` 8.3.
3. **Generator:** choose, embed, route, lock, fill, name, emit; the lock-in macro (E2); the fallback. Solver changes: verb gating, `when` on `while`, same-zone `to`. Checks C1 to C12. The dungeon viewer. Gate: `dungeons.test.ts` green on the generated mine, 64 seeds with zero fallbacks, John runs it on three seeds (`PLAN.md` M4).
   **Built** (`test/dungeon-gen.test.ts`), **except the viewer page**. 1,000 seeds, zero fallbacks, three attempts at worst. John's three seeds are still John's. The stateful flood, the `state` edge and the generated control list came afterwards, ahead of the Museum (`test/verbs2.test.ts`, on a test building; `ENGINE.md` 8.3).
4. **Mine content:** the drawer, First Aid, the track shortcut, the adit (county chunk change), the side rooms, variants up to three per pool.
   **Built, except the powder store** (the spell is in; it wants the Museum's `rubble` row); the cage is in. Two variants per pool, not three. The adit is in: a door in the gallery to the county mark `mine_adit`, and the county's side is barred until the gallery has been stood in, then a way back in. The dog no longer teaches Repair: its line points at the drawer. The office has heat 0 in the data: the Headmaster is its heat, and the drawer is `guardedBy` him, which is what C5 checks.
5. **Cellar touches.** E13 and the notices.
   **E13 built**, and the mine's notice reveals the mine. The cellar's touches are not.
   **Built ahead of steps 6 to 9, so that five dungeons can be written side by side** (`test/verbs2.test.ts`): E1, E7 (both schools, `touch`, the rows `explosion`, `grow`, `spark`, the effects `dazzled`, `jolted`, `dusted`, `softened`, three icons), E8, E9, E10, E11, E12, E13 and the stateful flood. What is left in those steps is content.
6. **Museum:** E1, E7 (`blast`), E10, E12, the stateful flood. This is the proof that a two-state building can be generated and proven. Then Explosion's places on the surface.
7. **Library and Butterfly Forest:** E8 (`litAt`, day-only rect lights, the Grow rule), E9, E11.
8. **Pipes and Factory:** E7 (`shock`), `sight: "lit"`, the county's relay boxes and dead lamp runs.
9. **Burial:** re-host the built rooms as set-piece templates under the generator with every contract name bound; `shunsLight`, the great torch, three corner bosses, Goldskin.
10. **School.** Then the two optional dungeons, the Ice House first (it is small and gives Icebolt its world use early in the player's game, so it could move up to step 5 if wanted).

One rule from `PLAN.md` section 7 governs all of it: nothing new starts until the last thing has been played.

