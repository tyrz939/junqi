# PSP: where it stands, and what's next

Read this before any PSP work. Keep it short and current: when a step lands or a hardware run comes in, update this file in the same commit. Put the dated detail in `docs/psp-log.md`. The reasoning, budgets and options are in `PLAN-PSP-SPEED.md`, cited by section.

## The goal and the line

- **Goal:** 60 fps or better on a PSP-1000 at 333 MHz at the full look, ideally even at 222 MHz. Then the 100-mob target: 100 hostiles on screen at 60 fps (`PLAN-PSP-SPEED.md`, "The 100-mob target").
- **The line** (owner): "do what's sensible. No hacks." That means no custom Media Engine code and no kernel mode. User-mode firmware services (SAS, the audio channels) and the VFPU are fine.
- The PSP may look slightly different from the PC, but any change to the look needs an A/B first. The sim stays integer and bit-exact with the PC.
- Shadows and lights may be capped in crowds.

## Latest hardware run: 2026-10-28, build e67fab6 (`progress/2026-10-28_88_hw-bench/`)

| scene | fps | worst ms | main costs (ms) |
|---|---|---|---|
| sprint | 88.2 | 101 | one 3 s hang at start-up (every run since 85) |
| wood | 40.5 | 62 | list 7.8 (sun casters 2.5, sprites 2.6), draw 5.9, ge_build 4.1 |
| town-night | 34.3 | 144 | list 11.4 (lamp pools 7.7), ge_build 4.7 |
| cellar | 91.9 | 26 | unpainted swatches 0; sim worst 77 on entry |
| mob | 12.0 | 498 | sim 23.6 (p95 104), list 25.2 (lamp pools 11.2), ge_build 13.1 |
| churn | 53.2 | **2705** | a zone built on the game's step (sim worst 421) |
| lake | 58.8 | 31 | water_fx 2.3 list, 2.0 GE |

On hardware, everything runs 3–4.6× slower than in the emulator, and the sim about 10× slower (cache misses the emulator doesn't model).

## Next steps, in order (not started)

1. **Split churn's 2.7 s worst frame.** Run the emulator churn scene with the painter entry boost off, then on (`spikes/psp-game/src/main.rs`, `PAINTER_ENTRY_FRAMES`). Log each time the step builds a zone itself. If the boost is to blame, keep it from outranking the game while a zone build waits.
2. **ahead.rs ready before the door.** Build the zone behind each door within reach ahead of time. If it isn't ready, hold a frame or two behind a fade rather than build inline. This fixes the cellar's 77 ms entry tick too.
3. **Find the sprint start-up hang.** Add crash-log events around the first world load and the first pack open, then ask for a hardware run. If it happens only at start-up, put it behind a loading screen and stop the bench counting it as a hang.
4. **Bake the interiors.** One JCK1 pack per zone, plus `-night` variants. The loader switches pack at the door.
5. **Mob sim:** S1–S4 and the 100-mob work (flow field, AI level of detail, spatial hash, LOS cache), with a `horde` bench scene.
6. **Then:**
   - night lamp pools;
   - wood `draw` and sprites;
   - audio (13.5–22% of the CPU);
   - mob `water_fx`;
   - ge_build;
   - one image per tree shadow (needs an A/B);
   - town-night's slow reads.

Each step: an emulator bench before and after, then a hardware build for the owner (`CLAUDE.md`, "The PSP loop").

## Recently done

- **2026-10-28:** the painter is raised at once on entering a zone (e67fab6): cellar swatches went from 501 to 0.
- **2026-10-27:**
  - The kept county is keyed on its ground digest (1497f96): no more 3 s stall coming back outdoors.
  - Sun casters off the canvas are culled before their rows lookup (2b10765): wood sun casters 5.5 → 2.5 ms.
  - The rows lookup uses a hash (c953740).
  - The bench captures per-part timings by itself (W0), and the VFPU pan is on.
