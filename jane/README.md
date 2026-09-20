# jane

The game. See `../README.md` to play it and `../ENGINE.md` for how it works.

```bash
npm install
npm run dev       # Vite dev server
npm test          # vitest, headless: the sim plays itself
npm run check     # tsc --noEmit && vitest run
npm run build     # production bundle into dist/
```

```
src/sim/      headless deterministic simulation. No DOM, no clock, no Math.random, no runtime trig
src/world/    zone builders and the lock-and-key validator
src/data/     content rows (JSON), validated at boot
src/art/      sprites and icons as palette-character grids
src/render/   Canvas2D renderer
src/input/    keyboard, mouse, gamepad -> one action layer
src/ui/       DOM UI behind ui/host.ts
src/app/      shell: fixed-step loop, save slots, console, replay
test/         engine, sim, dungeons, world, replay, art
```

Adding content is adding rows: `../SYSTEMS.md` rule 1. Adding a zone: `../WORLDGEN.md` §8. If `npm test` is red, the change is wrong.

In the browser console, `jane` is the running `App` (`jane.sim()` is the live simulation).
