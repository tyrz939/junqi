# Play plan (October 2026)

*Status: proposal for the owner. Written 2026-10-02 from the four design audits of 1 October (story, dungeons, world and quests, gameplay) and three research briefs of 2 October (progression, combat, fires), plus the owner's notes. Full reports with sources: `progress/audit/` (local, gitignored). Nothing here is built yet. Where this plan and `PLAN.md` disagree, this plan is the proposal and `PLAN.md` stands until the owner says yes.*

## 0. The short version

Jane's writing and dungeon structure already stand near the best of their genre. Play is what holds it back:
- The systems exist but have no teeth: a fire is a free instant full heal, and death is a short walk.
- The difficulty sits in the wrong places: the main path is safe for two hours, while the field beside it can one-shot her.
- The spells are one of everything.
- The puzzles never surprise.
- The side content piles up in the first region.

The plan fixes that in eight phases. Phase 0 is small fixes. Phases 1 to 3 rebuild the core loop on four ideas:

1. **Fights have targets and time in the right places.**
   - She targets a foe or a prop, or aims freely.
   - Melee auto-attacks.
   - Spells have real casts (1.0 to 1.5 s) she can walk through.
   - Enemies wind up, and she can hop.
   - The post-cast root goes.
2. **Fires are made, not found.** People's fires stay lit. Wild pits need deadwood and a match (or Fire). Rest heals over time, and only a bed heals at once.
3. **She grows two ways.** Finds stay the power channel (jars, pages, Words). XP and levels pay every quest and buy talent points in eight witch **Crafts**, at most three per witch, reset at the dog.
4. **Every number that changes is measured.** A telemetry harness in the repo, and bots that play the new rules before any tuning is trusted.

Phases 4 to 7 then spend that foundation on co-op, the world and quests, the dungeons and the story.

Every mechanic below is integer, in ticks, inside the sim, so lockstep, saves and the hash all hold.

---

## 1. Principles kept

- **Words answer the world; nothing else does.** Puzzle verbs (Icebolt, Repair, Explosion, Grow, Electric, Fire) come from the story. No talent, item or level can open, light, mend or burn a prop. That is how no build softlocks or skips a dungeon.
- **Finds are power; levels are choice.** Levels give talent points and no stats. Grinding buys options, never numbers, so the solver's placement promise ("phase N−1's upgrades are reachable before threat N") still holds.
- **Threat is felt only.** Enemies show no level or numbers. Only Jane has a level.
- **No softlocks, proven.** Every new rule ships with an L0 to L6 check (`VERIFICATION.md`).
- **Words true to the world.** Every new line is checked like the old ones.
- **Each phase ships green.** Fast and slow tiers pass, the release is refreshed and `main` is current. Risky changes land behind a `tuning.rs` flag until the bots can play them.

---

## 2. The new core loop

### 2.1 Combat: targets, auto-attack, real casts

**Owner's direction (2026-10-02):**
- Make casting enjoyable.
- 0.5 s feels short coming from WoW, so go with what is best for this game.
- Melee should auto-attack.
- Aiming at moving things without targeting is very hard, so mix targeting with free aim and keep the puzzles working.

The research found that Jane has **no cast times today**. Her "WoW feel" is a 1.5 s GCD plus a 0.5 s root *after* every cast: a wait that tells nobody anything. The new model moves the wait *before* the effect, where it can be read, built up and interrupted. It also makes hitting a moving thing a matter of choosing it, not of leading it.

**Targeting: a target when she has one, free aim when she wants it.**
- **Choosing a target:**
  - clicking a foe;
  - Tab, or RB/LB on a pad, which cycles foes in front of her, nearest first.
- **A target's display:** a ring under it, its name and a slim health bar. Hostile targets get a red ring and props a gold one.
- **Targeted bolts curve onto their target** with a capped turn rate, so moving foes are hittable. Walls still matter:
  - a cast needs sight to start;
  - a bolt can still strike a wall if the target ducks behind one;
  - range is checked at start and at release.
- **Props are targets too.** Clicking a blue torch, a cracked wall, a socket or a bud and casting the Word sends it there. Puzzles get easier to aim and lose nothing, and assist's "prefer a prop on the line" becomes explicit.
- **Free aim stays:**
  - with no target;
  - while she holds the free-aim key (or pushes the right stick on a pad);
  - for every ground spell (Hoarfrost, Kindling, Powder's charges, the Hand-Bell-style pools), which always lands at the cursor or stick point.
- **Soft target:** attacking with no target picks the nearest hostile in front of her (WoW's auto-target on attack).
- **Lockstep:** the target is a unit or prop id in the input frame, validated in the sim like everything else.

**Click to move (owner, 2026-10-02), alongside WASD, not instead of it.** On mouse:

| Input | What she does |
| --- | --- |
| Left-click a foe or prop | **Targets it without moving**, so a caster can target and cast from range |
| Right-click ground | Walks there, pathing around fences, props and water |
| Right-click a foe | Walks into her *attack range* and starts auto-attacking. A melee build closes to reach; at range, a caster's right-click only targets |
| Right-click a person, door, fire or chest | Walks over and uses it |
| A spell key with the target out of range | Walks to the edge of range, then casts (League of Legends' rule) |
| Any WASD key | Cancels the walk and gives direct control back. The pad keeps the stick |

The pathing has four rules:
- **It never cheats the fog.** Paths route only through cells she has seen; into unseen ground she walks straight and stops at the first obstacle.
- **It never pushes puzzle props.** Click-walking routes around pushables; pushing stays a deliberate WASD move.
- **It prefers roads and lit ground**, and a hit stops the walk.
- **It is lockstep-cheap.** A click sends one destination, and every peer computes the same path with the sim's existing deterministic pathfinding (capped length).

**Melee auto-attacks.** Right-clicking a foe, or attacking with a hostile target in reach, starts her stick swinging on its swing timer, as in WoW.
- She turns to face her target, and moving does not stop the swings.
- Swings stop when the target dies or leaves reach, or while she casts; they resume after.
- Every swing refunds energy, so auto-attack is **the generator** for the hop and for Blackthorn's actives, which fire as instants or on her next swing.
- Melee no longer competes with spells for button presses, so a melee witch plays like a WoW feral or warrior and a caster can stand back.

**Casts with weight.**

| Spell | Cast | Why |
| --- | --- | --- |
| Icebolt | **1.0 s** | The filler: chill, kite, light blue torches |
| Fireball | **1.5 s** | The heavy hit plus burning. Hearth's procs make it instant (in the style of WoW's Hot Streak) |
| Explosion | **1.2 s charge**, at a target or a ground point | Cracks walls, softens plate, knocks back |
| Electric (Spark) | **Instant**, short cooldown | The interrupt: it cancels a machine's wind-up |
| Mend | **1.5 s cast** heal on her target (a friend) or herself | The support seat's staple |
| Grow, Repair | **Instant** at props | A verb at a prop pays no tax |
| Craft actives | A mix: instants (Still, Rap, Darn), casts (Overload 1.5 s), channels | Each craft gets a rhythm of its own |

**Rules:**
- **The GCD is 1.0 s** (was 1.5 s). Auto-attack is off it.
- **No post-cast root.** She **walks at half speed while casting**, and the hop cancels her cast.
- **Mana is spent at completion.** An interrupted cast costs nothing.
- **Interrupts:** stuns and heavy knockback interrupt her; plain damage does not (no pushback).
- **A 200 ms queue window** lets the next press wait for the current cast, so chains feel tight at 50 ms of LAN delay.

**What makes casting enjoyable:**
- a glow that builds at her hands, with a rising tone;
- a slim cast bar under her;
- a release with punch (flash, sound, hitlag on big hits);
- procs that hand her an instant;
- Craft talents that let her cast while moving at full speed (Rime's "cold hands" for Icebolt);
- interrupting a foe's wind-up with Spark.

The cast times are data fields, so tuning after telemetry is one number per spell.

**The rest of the fight model:**

| Keep | Change | Add |
| --- | --- | --- |
| Per-spell cooldowns, mana, energy, sprint | Boss HP down so fights take **20 to 40 actions**, not 60 to 125 | **A hop**: 30 energy, about 1.5 m, i-frames on ticks 1 to 7. It cancels her cast. Space (keys) or LT (pad) |
| The Hand-Bell and Shot-Firer tells | | **Enemy wind-ups on every blow**: rats 0.35 s, skeletons 0.5 s, soldiers and bosses 0.8 s, the cactus 1.0 s. **Plus the table's input delay D** in co-op, so a guest gets a solo reaction window. A wind-up's melee hits only if she is still in reach and in its arc; its bolts and pools land where they were aimed when it began |
| | | **Interrupts both ways**: Spark cancels a machine's wind-up, chill lengthens one by 50%, and knockback cancels a small foe's. Boss tells are drawn interruptible or not |
| | | **Hitlag in the sim** (attacker and victim only, 4 to 6 ticks; a boss 10 on a phase change). **Knockback**: 6 px on a swing, 10 px on Explosion, none on bosses; plates ignore pushes |

**Diablo, taken and left.** Take the feel (hitlag, knockback, sound), the hop, generator and spender, and "marked" foes: a common foe given one extra rule from the roster Jane already has (lit-sight, shuns light, plated, webbing), named in an omen and readable on sight. Leave random loot and affixes; they fight the solver and the voice.

### 2.2 Rest, fires and death

| Fire | Lit? | What it gives |
| --- | --- | --- |
| **Kept fires**: the Halt brazier, Castle square, Julie's hearth, the farm, the Reedcutters', the canteen, the mine mouth, every dungeon rest-room stove, and fires a quest lights (then kept by the parish) | Always. Rain dims the light; the rest stays | Save and waking point at once. **Heal over time** while seated: full in about 30 real seconds. It breaks if she acts, moves or is hit, or if a hostile targets her or stands in its aggro reach with sight |
| **Made fires**: `campfire_cold` pits in clearings, work sites and cleared camps; two or three old grates only Fire lights | Cold until she lays **2 deadwood** and lights it with **Fire**, a **match** or a **fire stone**. A 1 s hold, never a dice roll. A match refuses in rain and says why, and nothing is spent | As a kept fire, while lit. **Burns 4 game hours** (8 real minutes); each extra deadwood adds 2 h, up to 12. Lit for the whole party. Rain puts it out unless it stands by a tent or shed. **The waking point is the place, not the flame**: ash keeps it |
| **Beds** | n/a | **Full heal at once**, clears statuses, passes the clock (party) |

- **Deadwood** is a new item (stack 16) from stumps, woodpiles and fallen trees, regrowing on the food clock. Planks are refused: "Too good to burn. Repair wants those."
- **Matches** carry hours 0 to 10: 5 at the Halt trunk ("The fire keeps your place. Keep a box dry."), 10 regrowing in Julie's drawer, and about 20 along the spine. **Fire**, from the Burial, makes every pit trivial: a new verb that opens the old map.
- **Unbanked jars.** Growth from a jar or page found since the party's last rest lands at once. If she dies before anyone rests, it comes off and lies as a glint where she fell; die again first and it returns to its shelf. Nothing is ever lost for good, and a fire becomes where you *bank*.
- **Proofs.** With empty bags she can always reach a save and a heal:
  - a kept fire inside every region's base threat;
  - deadwood within 12 cells of every cold pit;
  - a match source before the first cold pit;
  - the Reader finishes with matches zeroed at each chapter;
  - a burnt-out fire still wakes her;
  - a slept night burns like a sat-up one (L6).
- **The tedium budget** (bots): at most 3 fire chores per real hour, at most 5% of time gathering, at most 1 lighting in 10 refused by rain. Rests per run should fall from 58 to 112 to about 20 to 40.

### 2.3 Growth: finds, levels and eight Crafts

**Finds stay the power channel.** Jars are strength (health, melee) and pages are spirit (mana, spell power). They stay world-shared, once per id, and are re-spaced to fix the plateau after hour 4. The six **Words** stay story-given.

**Levels pay choice.** XP is shared by the party, like growth. The cap is 25, and XP to the next level is 100 × L (level 10 at 4,500, level 25 at 30,000). Each level gives **one talent point and no stats**: a toast, a glow and a sound. A thin XP bar and her level go on the HUD.

| XP source | Each | Share |
| --- | --- | ---: |
| Spine quests | 600 (phase 1) to 1,800 (phase 6) | 48% |
| Side quests and tales | 40 + 30 × phase | 29% |
| Words learned | 250 (the Ice Orb makes her level 2 at minute 3) | 5% |
| Discovery: a named place first seen, a kind first killed | 20 | 5% |
| Kills: 2 to 12 by phase, bosses 150 to 300. Half at one phase under her growth, **none at two** (WoW's grey) | | 13% |

That is about level 10 at the Museum, 15 at the Forest, 20 at the Burial, and 25 at the end with the side quests done. Skipping every side quest still reaches about 21. A night in a bed gives rested XP that doubles kill XP up to 300; it is framed as a bonus and never a penalty. XP is never lost on death.

**Eight witch Crafts replace "one of everything".** Each Craft is a fantasy that deepens one Word (or her stick, or the lamps), with about 20 points of nodes:
- open row A;
- row B at 3 points in the craft;
- row C at 6, with one choice of two;
- row D at 9;
- a **capstone at 12**;
- a deep-only **far shelf at 16**.

A witch uses **at most three Crafts**, so 20+4, 12+8+4 and 8+8+8 are all real builds. Grim Dawn and Titan Quest's "pick masteries and find the synergy" is the model, and Diablo 2's synergies are the warning: depth must pay in *kinds*, breadth in *combos*, and never in bare percentages.

| Craft | Fantasy and role | Deepens | Capstone (12) |
| --- | --- | --- | --- |
| **Rime** | The cold hand: control, kiting | Icebolt | Hard Frost: a frozen foe struck by any blow shatters, from anyone's hand |
| **Hearth** | The kitchen fire: damage over time | Fire | Wildfire: a foe that dies burning passes it on |
| **Current** | The Works' live wire: burst, chains | Electric | Ring Main: every lit lamp in reach arcs to foes beside it |
| **Powder** | The shot-firer: charges, armour | Explosion | Chain Shot: a burst sets off every charge and strips shields |
| **Hedge** | The green witch: roots, thorns | Grow | The Long Hedge: a thorn wall foes must path around |
| **Mending** | Make do and mend: support | Repair, Mend | Hold: a dome in which friends take half for 4 s |
| **Blackthorn** | The stick: melee, energy, tank | her blows | Thrawn: once per rest, the felling blow leaves her standing 2 s |
| **Lantern** | The lamplighter: light against the dark | lamps, the light stone | Curfew: every foe in her light backs out of it for 6 s |

- **Words become tools; Crafts make them weapons.** Without its Craft a Word keeps its effect (chill, jolt, crack, burn), but has a longer cooldown and about half today's damage. A Craft's first node restores today's numbers and adds a twist, so four elemental bolts stop competing for every bar slot.
- **Craft spells are `world: false`.** Hearth's Ember never burns a web; only the Fire Word does.
- **Cross-craft combos reward breadth.** Rime + Powder (freeze, then shatter); Hedge + Current (bound foes stand in the arc); Hearth + Hedge (thorns burn); Lantern + Current (her witch-light counts as a lamp).
- **Mend** becomes a found charm in the library that every witch has at a slow basic rank, so a solo witch can always heal. Mending deepens it.
- **Spells come two ways, as the owner asked.** Words come from the world (the dungeons). Craft actives come from talent points.
- **Talents are per seat; growth and level are shared.** A guest joining at world level 14 has 13 points to spend at once, and roles emerge (a Mending seat beside a Blackthorn seat).
- **Respec happens at the dog, by day.** Points stay in pencil until she rests ("sleep on it"). The first full reset is free; after that each costs the dog 1, 2, 3, then 4 meals from her own bag, easing back a step every three game days. One craft alone costs one meal. The scene is sitting with the dog over Julie's eight shelves, and no line needs to explain it.
- **Guard rails, held by tests:**
  - a `jane-data` check that no node is a bare percentage, that the three-craft cap holds, and that craft rows are `world: false`;
  - the bots finish the story with each of the 8 deep builds and a sample of shallow triples, inside today's death band.

---

## 3. Phases

Each phase ends green, pushed and released. Format bumps are grouped so saves break once per phase, not once per feature.

### Phase 0: quick wins and correctness (days; mostly data)

| # | Change | Where |
| --- | --- | --- |
| 0.1 | **Telemetry in the repo**: the audit's harness becomes `jane play --telemetry`, writing deaths, blows, heals, rests, chapter times and per-hour stats to CSV | `jane-cli`, `jane-bot::run` |
| 0.2 | **Fix the two bot stalls**: seed 6 at the Factory (Reader, Cautious) and seed 8 after the Mine (Rusher) | story bot, solver |
| 0.3 | **No one-shots near the start**: nothing within 400 m of the Halt deals 35% or more of starting health in one cast; the cactus gets a 1 s bristle tell; a bot audit extends "first walk is safe" to the fields beside it | `data/spells.json` `cactus_spray`, `jane-bot` audit |
| 0.4 | **Castle shows at most three "!"** at once, gated by day, act or news; the parish board is the breadcrumb | `data/dialogue/town.json` |
| 0.5 | **Quest marks on prop givers** (the lost-property book, the parish board, the glovebox, the farmhouse door), and **the marks swapped to WoW's**: "!" offers, "?" takes back | `View::quest_mark`, `ui::marks` |
| 0.6 | **The story's set-ups paid off**: the Small Present in all three endings, never opened; her name taken off the scarf and the stump, so the time book lands first; Julie's own line in the time book; "She is not dead." at the Burial, saving "{name}. I would know." for the last page | `data/items.json`, `data/dialogue/*.json` |
| 0.7 | **A repetition pass plus a lint test**: "all the same" at most 3 (it is 15), "No bell" openers at most 2 (it is 10 of 14), the third Halfway House line, the five rule-breaking lines, and knock lines rationed to one in three odd | `data/dialogue/*.json`, `jane-data` tests |
| 0.8 | **The great torch on the Burial's critical path** | `data/dungeons/burial.json`, `tactics/burial.rs` |
| 0.9 | **A warning before the Factory's line fuse** shuts the stores for good; **optional areas made sure** (the Drowned Lane, the Still Pool, the Cooling Yard) | `factory.json`, `data/areas.json` |

The audit's "teeth in the first hour" (Iron Knuckles at 4 actions) waits for the tuning pass after Phase 3, because Phases 1 to 3 move every number.

### Phase 1: combat feel (one save and replay bump)

- **Targeting:** hard targets on foes and props; Tab, RB and LB cycle; click to target; a soft target on attack; bolts that curve onto their target; free aim kept.
- **Melee:** auto-attack with an energy refund.
- **Click to move:** right-click to walk, attack or use; walk to range on an out-of-range cast; WASD cancels (§2.1).
- **Casts:** cast times per §2.1, a GCD of 1.0 s, no post-cast root, half-speed walking while casting, and the 200 ms queue. The cast feel: a glow, a tone, a bar and the release.
- **Enemies:** a `windup` on every enemy blow, plus D at the LAN table.
- **The hop.**
- **Feel:** hitlag and knockback, plus interrupts both ways.
- **Bosses:** HP down to 20 to 40 actions.
- **The bots learn to target, hop and read wind-ups before any telemetry is trusted.** Otherwise their deaths rise and mislead tuning.

Touches: `combat.rs`, `state.rs`, `sim.rs` (the move gate), `ai.rs`, `flush.rs`, `status.rs`, `assist.rs`, `tuning.rs`, `input.rs`, `view.rs`, `save.rs`, `codec.rs`, `jane-net`, `jane-present` (raise frames, cast glow, decals), `jane-audio` (wind-up cues), `jane-bot`.

### Phase 2: fires, rest and death (behind `FIRES_MADE`)

- **Fires:** kept and made fires, deadwood and matches.
- **Rest:** heal-over-time rest that threat breaks; a full heal at beds.
- **Death:** unbanked jars.
- **Proofs:** the L0, L1, L3 and L6 proofs from §2.2.
- **Bots:** they gather and light.
- **Release:** flip the flag when the Reader finishes on seeds 1 to 8.

Touches: `data/props*.json`, `data/items.json`, the `station` and `julie_house` chunks, `verbs.rs` (`rest()` split; `make_fire`, `add_wood`), `interact.rs`, `living.rs` (burn-out on the ten-minute step), `life.rs`, `ai.rs`, `view.rs`, `furnish.rs`, `small.rs`, `skeleton/build.rs`, `jane-art` (laid, lit and ash states, the match flare), the map dots. `SAVE_VERSION` 12.

### Phase 3: growth (XP, levels, Crafts)

- **XP and levels:** XP in `Growth`, levels, and per-seat `Talents`. Spend, Undo and Respec are actions.
- **Crafts:** the eight Crafts as data (about 30 new spell rows plus `data/talents.json`), the Words retuned, and Mend as a library charm.
- **The dog:** a respec node on the dog.
- **UI:** an XP bar and level-up effect, a Crafts tab, and a house-voice toast.
- **Bots:** a build policy per model, and `tests/builds.rs`.
- **Then the tuning pass, measured by 0.1:**
  - the first hour gets teeth, aiming for a first death in the Mine on about half of seeds (Iron Knuckles about 15 actions with a tell);
  - the jar curve is re-spaced;
  - the boss lengths are checked.

### Phase 4: co-op and options

- **The party penalty counts seats nearby** (within about 60 m or in the same dungeon) and skips idle seats. It is reworded as the county reacting to a crowd ("The county has noticed there are two of you") and scales the enemies, not her.
- **The three dormant co-op tags get used**:
  - the Hoist, by two (`lure_and_lever`);
  - the Timekeeper's rope (`twin_hold`);
  - `plate_or_friend`.
- **The Lamp-bearer:** one carries the light while the other fights the lit-sighted sentries.
- **A revive in the style of Don't Starve Together:** a fallen friend lies 20 s as a ghost, and an apple stands her up at 30%.
- **"How the county takes her"**: a per-save dial set at New Game and changed only at a bed (Celeste's rule):
  - the county: gentle, as written or harsh;
  - the way back: nearest fire, last rest, or last bed only;
  - her hand: wind-ups +50%, 80% speed solo.

  A hidden "Julie's blanket" adds +3% resistance per repeat death to the same foe, capped at 15%.

### Phase 5: world and quests

1. **The map as a memory:**
   - fires rested at, as warm dots (made fires with a burn arc, cold pits as grey rings);
   - signs read, as named dots she can re-read;
   - place names, and three to five pins of her own.
2. **Sub-area banners** the first time each day she enters, and a **perimeter for each sub-area** (a hedge ring, a field wall, a reed edge, a slag bank).
3. **Cues past the screen edge:** crows over camps, the Hoar Stone's silhouette above the fog, positional sound, and a far landmark per region (a Works chimney with smoke, the church spire, the lake statue).
4. **Build the Waters and the Works side content.** W1 to W15 and K1 to K14 are already designed; start with the two silent hubs (the Reedcutters' order slate, the canteen's rota).
5. **A consequence row for every quest**, and growth, keys or materials instead of food after the first link of a chain.
6. **Turn 15 errands** with a second step that changes the first.
7. **New quest verbs:**
   - walk with (a lamp escort);
   - hold the light till six;
   - carry;
   - listen;
   - test a claim.
8. **One cross-region arc per region**, in the style of Duskwood's Stalvan chain; **delayed consequences** that return an act later; **side quests that reach the ending**; a **"Who's where" journal page**.
9. **World moments:**
   - **The lamps are off tonight:** announced three days ahead.
   - **Sunday at the Halt:** schedules meet on the platform.
   - **Julie's map:** pages drawn from the seed, with a feature or two as they were.

### Phase 6: dungeons and puzzles

1. **The School's lessons as two-verb rooms.**
2. **An Icebolt world verb**: still water freezes, and pushed things slide on ice. Taught in the Mine, twisted with the Pipes' valve, concluded in the ice house.
3. **The sight edge**: glass, rail and water see-through runs. Layouts are scored for adjacency, and C10 requires the first lock and the boss door to be seen across a gap.
4. **The Factory's unfinished twists**: the Foreman walks to the breaker, the Charge Hand is visible to his own sentry, and a light/dark trade sits on the main path.
5. **Reset-on-exit puzzle rooms**, so real Sokoban dead ends can exist while C1 to C13 still hold.
6. **Boss Keys metrics as tests**, per dungeon over 16 seeds:
   - the verb has four or more roles;
   - there is a twist room;
   - a choice of two or more appears at some step;
   - at least one puzzle takes three or more moves.
7. **A verb-pair chemistry table** that the School tests:
   - frost on fire makes steam that blocks sight;
   - Spark arcs through flood water;
   - Explosion shatters ice.
8. **A visual lock language**, so fewer boards are needed.
9. **One signature moment and one new puzzle per dungeon** (`progress/audit/dungeons.md` §6).
10. **Puzzle-varying room variants**, or retire the three-variant rule for set-piece dungeons.

### Phase 7: story

1. **A personal midpoint at the Factory**: drafts of her own birthday letter in Julie's hand, years apart.
2. **The dog's hand-ins say less when the journal already holds the act's fact.**
3. **A co-op fiction in about six lines.**
4. **The forty-one names seeded** across the memorial, coat tapes, register and country stories.
5. **Three free-text choices for Jane** in the style of Kentucky Route Zero: what she feels, with no consequence.
6. **The dog's muzzle greys by act.**
7. **The song in the tunnel**: one line hidden per act, and the train ending sings only the lines she found.
8. **"Julie."** as a third option at the choice once four handwriting clues are known.
9. **The night the house is as it was.**

---

## 4. Decisions (owner, 2026-10-02: "go ahead with all your points")

Everything below is **decided as recommended**, with three of the owner's own changes:
- **D1:** cast times are set by gameplay, as in §2.1.
- **D10:** the quest marks are swapped to WoW's way round.
- **D15 and D16** are added.

| # | Decision | Decided |
| --- | --- | --- |
| D1 | Cast weight | **§2.1**: Icebolt 1.0 s, Fireball 1.5 s, Explosion 1.2 s, Spark instant, Mend 1.5 s, verbs at props instant, GCD 1.0 s |
| D15 | Targeting | **Hybrid**: click, Tab or RB/LB targets a foe or a prop; targeted bolts curve onto it; free aim with no target, on a held key or the pushed right stick, and for ground spells |
| D16 | Melee | **Auto-attack** on a hostile target in reach; swings refund energy |
| D17 | Click to move | **Yes, beside WASD**: right-click to walk, attack or use; left-click targets without moving; a spell out of range walks her to range; paths never cheat the fog or push puzzle props |
| D2 | Hop binding: Space (bar slot 1 moves to 1 and left click) or sprint plus a tapped direction | **Space** on keys, LT on pad |
| D3 | Levels give points only, not stats | **Yes**; finds stay power |
| D4 | Eight Crafts, at most three per witch, capstone 12, far shelf 16, cap 25 | **Yes** |
| D5 | Respec at the dog by day for meals (free first, then 1, 2, 3, 4) | **Yes**; no clock cost, since the clock is shared in co-op |
| D6 | Made fires: rain puts them out unless sheltered. This rewords `WORLD.md` §2.2's "weather never takes away a rest point" to "a *kept* rest point" | **Yes** |
| D7 | Rest at a fire heals over time and threat breaks it; only a bed heals at once | **Yes** |
| D8 | Unbanked jars drop on death until a rest | **Yes**, after the one-shots are capped |
| D9 | The co-op penalty counts only nearby seats, reworded | **Yes** (this changes your server-wide decision) |
| D10 | The quest marks | **Swapped to WoW's**: a gold "!" offers a quest, a "?" takes it back (Phase 0) |
| D11 | Reset-on-exit rooms, so puzzles can go wrong | **Yes**, a few per dungeon |
| D12 | Giving Jane a stake: the Factory drafts, Julie's line in the time book, the Small Present paid off | **Yes** |
| D13 | The difficulty dial | **Yes**, in Phase 4 |
| D14 | Build order as in §3 | **Yes**; Phase 0 starts at once |

---

## 5. Risks

- **Compounding difficulty.** Heal-over-time, cold pits, unbanked jars and enemy wind-ups all move balance at once. Mitigations:
  - cap the one-shots first;
  - land fires behind a flag;
  - tune only on telemetry from bots that hop and make fires.
- **Format churn.** Phases 1, 2 and 3 each bump the save, hash and replay formats once. Old saves stop loading at each.
- **Bots.** Every phase needs the bots taught first, or the slow tier goes red and the numbers lie.
- **Scope.** Phases 5 to 7 are content-heavy. Phases 0 to 3 are the foundation and are worth finishing before any of it.
- **The pad is untested by hand.** The hop and soft-lock need a real controller session.

---

## 6. Bugs to fix after the reset (owner playtests, 2026-10-03)

- **U-shaped block above some props.** Coming from above, she can walk right up to the prop, but in the space above it she cannot move left or right: she can only enter or leave from straight above. The likely cause is the side posts (`PropDef::solid_parts`) and the "enter and leave the space behind only from the north" rule for puzzle, gate and `way` props, which still make a U around the space above. **A known case: movable (pushable) crates.** The owner saw it there. Pushables were deliberately kept full-width so pushing from behind still works, which is probably why. **Fix:** list which props do this; for each, make the side edges match the drawn ground box, or keep the U only where a puzzle truly needs it, and draw it so it reads. **Test:** extend `units.rs`'s approach test to step sideways out of the space above every boxed prop.
  **Fixed (2026-10-05):** every prop that kept its width had the posts (pushed and carried things, gates, `way` rows, whatever answers a verb). There are no posts now, and no prop needs a U. A pushed or carried thing meets her at its drawn box; the solver never counts on one shutting a way (`blocks_feet`). A gate, a `way` row or a prop that answers a verb has its box carried up to within `NOTCH_MAX` (11 sixteenths) of its back. That is less than her body is tall, so she can't slip past it. The `units.rs` and `feet.rs` tests step out sideways.

## 7. After the reset, once the phases above are done: a RAM-efficiency pass

Owner, 2026-10-03: low-RAM systems are targeted later (Dreamcast 16 MB, PSP 32 MB, original Xbox 64 MB), "so features can carry then". This is not a port; it makes the PC build lean so a port is a renderer job, not a redesign.

1. **Measure first.** Add a `jane bench --mem` breakdown by system: atlases and baked art, the terrain chunk cache, the county and blueprints, sim state, audio buffers, and each renderer's own allocations. Record the baseline (about 205 MB on soft and 520 MB on wgpu as of 2026-09-30).
2. **Art.**
   - Draw-time motion instead of baked frames: sway as a bend (ART-PLAN §9), then birds, smoke and water.
   - Palette-indexed (CLUT) storage for atlases where the tier allows.
   - Evict art that isn't on screen.
3. **Terrain.**
   - A chunk cache sized by budget, not by count.
   - Paint chunks ahead of her movement, so a smaller cache doesn't stutter.
4. **World and sim.**
   - Store the county in compressed chunks; tiles and props can use run lengths or a palette.
   - Check what the sim keeps per zone, and drop what can be rebuilt from the seed. Determinism must hold: same hash.
5. **Audio.** Stream or compress the synthesized sound beds instead of keeping full PCM.
6. **Budgets as tests.** Per-system memory budgets in the slow tier, so regressions show. Targets: 64 MB on soft first, then 32 MB.

## 8. Where things stand (2026-10-03, end of the week)

**On `main` and in the playtest download (ee7f731 and later):**
- Phase 0 and the playtest fixes;
- the WoW-style quest marks;
- made fires, behind `FIRES_MADE = false`;
- the map, banners and far landmarks;
- art batches 1–3;
- the Waters and Works quests;
- the key ring.

**Not merged:**
- **Phase 1 combat.** It is on branch `worktree-agent-a61d63c1db2901d51` (head 958401b), with both halves integrated and 137 of 138 slow tests passing. The one red test is the Reader on seed 2, stalled in the School (likely a lesson she can't pay for). See `PHASE1-STATUS.md` on that branch. Next: diagnose seed 2, run telemetry for the Reader and Cautious on seeds 1–8, retake the screenshots, then merge.

**Queued after the reset, in order:**
1. Finish Phase 1, then teach the bots fires and flip `FIRES_MADE`.
2. The bugs in §6: the U-block above pushable crates.
3. The Grok findings (`grok-feedback.md`): refuse unproven blueprints, make stale fixtures fail, and CI.
4. Phase 3: growth (XP, levels and the eight Crafts), then the tuning pass.
5. Art batch 4 (dungeon framing, motifs and boss rooms), then batch 5 (hero objects, vignettes, portraits and the letter-paper UI). Also ART-PLAN §9: a sway rebuild as a draw-time bend, and leaves that move.
6. The RAM-efficiency pass (§7).
7. A generated `STORY-AND-QUESTS.md`, and the test binaries binding to loopback.

Run `tools/hiqos.ps1` in the background during heavy sessions, and prune merged worktrees regularly.
