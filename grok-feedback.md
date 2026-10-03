# Grok review: the useful findings

*Grok Build CLI 1.0.44 ran a read-only deep review (plan mode) on 2026-10-03, at `ee7f731`. Below are only the findings I judged useful, with my verdict on each. "Verified" means I checked the cited code myself. Nothing here is fixed yet; these are candidates for after the reset.*

Grok found **no critical determinism break**: no float, no `HashMap` walk and no wall-clock read on the sim's state path. That matches our own gates.

## Act on these first

| # | Finding | Verdict |
| --- | --- | --- |
| 1 | **A zone the solver rejected is still played.** `county::build_proven_with` returns `Ok(bp)` when its 12 attempts run out (`jane-world/src/county/mod.rs` ~268–274). `build_zone_with` turns that into `Some` (`jane-world/src/lib.rs` 47–49), and dungeons and interiors do the same. `jane-sim/src/blueprints.rs` 31–50 also keeps an unproven `build_county` fallback under a stale comment. New Game seeds from the clock (`jane-app/src/main.rs`), so a seed no test ever built can start broken. | **Verified, real. Highest value.** Refuse an unproven blueprint and re-roll the seed for New Game (or show an error), and delete the fallback. Add a sweep over many random seeds to the slow tier |
| 2 | **Stale fixtures pass silently.** `jane-world/tests/determinism.rs` 97–106 and `jane-bot/tests/fixture.rs` 12–21 `eprintln` and `return` on a content mismatch, so the cross-build hash gate is off after any data change until someone regenerates. | **Verified.** We regenerate them by hand at every merge, but a forgotten one stays green. Make a mismatch fail, and keep regeneration explicit |
| 3 | **CI runs the fast tier only.** `.github/workflows/rust.yml` never runs `--ignored`, and the i686 and ARM jobs test only `jane-core`. | **Verified.** Our slow tier runs locally, so CI isn't the real gate today, but cross-target sim and world hashes are never checked. Add the fixture tests on i686 and one ARM target, plus a small nightly `--ignored` set (one story, one crawl, the albedo gate) with `--test-threads=1` |
| 6 | **`panic = "abort"` in release, plus `expect("unit")` in the tick** (`ai.rs`, `combat.rs`, `flush.rs`, `snake.rs`, `life.rs`). A content bug kills the host with no save, and every guest with it. | **Plausible; `panic = "abort"` verified.** Skip a missing actor for the tick and emit a dev event. Keep abort for the checked profile |

## Worth doing in the planned passes

| # | Finding | Verdict |
| --- | --- | --- |
| 4 | **Memory:** the 2000×2000 county's tile grid is held twice (blueprint plus the runtime clone, about 4 MB each, plus a 4 MB flag grid), all 13 blueprints stay resident, and T1/T2 reserve about 33 MB of chunk cache even indoors. | **Useful input for the RAM pass** (PLAY-PLAN §7). Read `bp.tiles` plus a delta set instead of cloning; use an LRU chunk cache that shrinks indoors |
| 5 | **Click-targeting picks a person-sized box**, not the body the sim hits (`jane-present/src/input.rs` ~805–830, still a `TODO(P6 scene)`). Snakes and tall foes mis-target. | **Check against the combat branch** (Phase 1 rewrites targeting). If it survives there, select with `body_dist_sq` and the prop footprint |
| 7 | **Content decimals go through `f64`** in `jane-schema/src/compile/fraction.rs` (the one allowed float), then `round`. An edge `.5` or a libm difference could change content hashes. | **Valid, low risk.** Parse decimal digits and scale with integer `mul_div`; cheap to do |
| 8 | **Two schema checks are only warnings:** a name with no provider, and an unknown `.room` header. A typo passes `jane check`. | **Agree.** Make both errors now that the county and rooms are data |
| 9 | **On one core, the lockstep hash runs on the game thread every 60 ticks**, which is several ms on a Pi, and `MAX_BEHIND` then drops backlog. | **Relevant for the low-end targets.** Hash less often on one core. Not urgent for PC |
| 11 | **The slow tier is heavy:** each story test spawns 8 games in parallel with other threaded sweeps, so it is memory-hungry and long. | **Agree; we feel it** (the disk and time this week). Split it into a nightly small set and an on-demand full sweep |
| 12 | **Saves have a version but no migration.** Every bump strands every slot (it is at v14 now). | **Agree, before any public build.** Add the migration chain, and avoid bumps for field-order-only changes |

## Housekeeping (cheap)

- **10. Stale docs.**
  - `ARCHITECTURE.md` line 5 says "Nothing here exists yet."
  - `README.md` lists built bots as "not built".
  - `MISSING-SYSTEMS.md` still describes the TypeScript engine.
  - The `blueprints.rs` comment is stale.

  One pass. **Verified** for the `ARCHITECTURE.md` line.
- **13.** T1 never checks `glGetError`. Check once a frame in debug and surface the first error.
- **14.** `Guest::corrupt_at`, a test hook that forces a desync, is a public field on the shipping type. Gate it behind `cfg(test)`.
- **15.** `dev-data` leaks a catalog on each `/data` reload. Leak once and replace only on a content-hash change.

## Grok's top 5, with my agreement

1. Refuse unproven blueprints and delete the county fallback. **Yes, first.**
2. Stale fixtures must fail; cross-target fixture tests in CI. **Yes.**
3. A small nightly `--ignored` job, single-threaded. **Yes.**
4. Target by the sim's body distance. **After the combat merge, if still true.**
5. Don't abort on a missing unit, and drop the second county tile copy. **Yes; the second is part of the RAM pass.**
