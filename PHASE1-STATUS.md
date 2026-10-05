# Phase 1 status (combat feel: PLAY-PLAN §2.1), 2026-10-03

Branch `worktree-agent-a61d63c1db2901d51`. It holds part A (the player's side) and part B (`worktree-agent-ad2cfca3bbc70a178`, the foes' side and the feel), merged with density-integration. `SAVE_VERSION` is 12, bumped once for both parts.

## Built

**Targeting** (`jane-sim/src/target.rs`)
- A hard target (a unit or prop id) rides in every `InputFrame` (now 12 bytes) and the sim validates it each tick.
- Tab, RB and LB cycle the foes in front. With no target, an attack takes the foe under the cursor or the nearest foe in front. The free-aim key, the right stick, no target, and every ground spell all aim freely.
- A bolt cast at a unit curves onto it. A bolt cast at a prop ends on it, and every Word works that way.

**Casts** (`cast.rs`)
- Each spell row has a `cast` field: Icebolt 1.0 s, Fireball 1.5 s, Explosion 1.2 s. Everything else is instant.
- The GCD is 1 s. A spell's cooldown starts when the cast begins. There is no post-cast root, and she walks at half speed while casting.
- Mana is paid when the cast lands. A press within 12 ticks of her being free is queued.
- A stun, a speed-0 hold as it lands, a hop, a heavy push or Esc (`Command::Halt`) stops the cast. Nothing is spent, and the GCD and cooldown are handed back.

**Auto-attack**
- It holds a foe that her own knock pushed a step away, and she steps back in to it.
- Each landed swing refunds energy.

**Click to move** (`walk.rs`, `Command::Goto`)
- Paths go only through seen cells and never push anything.
- They prefer roads and light.
- WASD or a hit cancels the walk.

**Part B** (`feel.rs`): wind-ups with +D at the LAN table, the hop on Space and LT, hitlag, knockback and boss `hpScale`. `feel::interrupt_her_cast` now calls `cast::interrupt_unit`.

**Client:**
- Bindings: left click selects, right click goes, Tab, Ctrl for free aim, Space for the hop.
- Drawing: the target ring, name and health bar; the cast bar, hand glow and release flash; a rising cast tone; the click marker.
- Screenshots in `sheets/p1-player/` and `sheets/p1-combined/` (`sheets/` is gitignored).

**Bots**
- Casts at foes target them, and bolts aimed at answering props target the prop.
- A free-aimed cast holds its aim while it builds.
- They won't start a cast a dangerous foe can close on, Spark what is too close, and drop a cast under a tell.
- In the Factory they use only Spark from the dark at things that see by light, leave to recover mana when empty, and keep one stack of wood and iron.

**Tests:** `jane-sim/tests/fight.rs` (20 tests) and the combat tape with targets, `Goto` and `Halt`. The fast tier, clippy and fmt are green. Both hash fixtures were regenerated on 2026-10-03.

## Slow tier (last full run, commit before this file)

137 test binaries/groups pass. Passing: every dungeon crawl (all 10 on seeds 1–3), the Lost's clarity sweep, the School and Burial crawls, the Cautious on seed 6 and the Rusher on seed 8, the growth curve, and the set-aside story.

One test fails: `the_reader_reaches_an_ending_on_seeds_1_to_8`.

```
seed 2: ended 0 not 2
seed 2: the_school not done
seed 2: the_choice not done
seed 2: the town never noticed ["bell_stopped"]
seed 2 hill  the_end 0 | 2400:00 | spine 10/12 | deaths 0 | stuck [("the_school step 2: the school took too long", ...),
  ("the_school step 2: the school: nothing left to try in the school (its boss: ringer; down: []);
   locked school_door_physics (reachable, wants no tag); locked school_door_lost ...")]
```

Seeds 1 and 3–8 reach their endings.

## Best guess at the cause

- **School, seed 2:** the same shape as the seed 5 stall fixed today. A lesson can't be done, so the lesson doors (opened at break) stay shut and the crawl runs out of tries.
  - On seed 5 the lesson needed 2 wood for Repair at `school_woodwork_steps`, and her full bag had thrown the wood away. The new rule keeps one stack of wood and iron.
  - Seed 2 may be short of a different lesson input, or one lesson's Word lands at the wrong time.
  - To check: run `jane play --model reader --seed 2 --ending hill --explain` up to the School stall, then look at the bag and at the props listed as "answers ... (needs ...)".
- **Background (not a test failure):** with a 1 s GCD the bots spend mana faster than with part B's 1.5 s. The Factory stalls fixed today came from that combined with Icebolt revealing her to the Foreman.

## Next 3 steps

1. Diagnose Reader seed 2 at the School as above, fix the cause in the bot or the content, and rerun the story test: `cargo test --release -p jane-bot --test story -- --ignored`.
2. Rerun the full slow tier once green. Then run telemetry for the Reader and the Cautious on seeds 1–8 (`jane telemetry --models reader,cautious --seeds 1..8 --out ...`) and compare against before the phase (the Reader averaged about 5 deaths a run, first death around minute 106).
3. Retake the `sheets/p1-combined/` shots with the foe's wind-up glint clearly in frame. The current ones show the cast bar and glow, but the skeleton sits too close to read. Then merge to density-integration.
