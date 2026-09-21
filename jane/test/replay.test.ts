import { yardCatalog } from "./bot";
import { describe, expect, it } from "vitest";
import { playReplay, Recorder } from "@/sim/replay";
import { hashState } from "@/sim/save";
import { Sim, type Command, type InputFrame } from "@/sim/sim";

const catalog = yardCatalog();
const q = (v: number): number => Math.round(v * 127) / 127;

describe("replay", () => {
  it("re-simulating a recording lands on the same hash, commands and dialogue included", () => {
    const seed = 31337;
    const sim = Sim.newGame(catalog, seed);
    const rec = new Recorder(seed);
    const send = (c: Command, f: InputFrame): void => {
      sim.setAim(f.ax, f.ay);
      rec.command(0, c);
      sim.command(c);
    };
    for (let t = 0; t < 1500; t++) {
      const a = Math.floor(t / 30) * 0.7; // held input changes twice a second, as a hand does
      const f: InputFrame = { mx: q(Math.cos(a)), my: q(Math.sin(a)), ax: q(Math.sin(a)), ay: q(Math.cos(a)), sprint: t % 300 < 100, useHeld: false };
      if (t % 40 === 0) send({ t: "bar", slot: 0 }, f);
      if (t % 170 === 0) send({ t: "use" }, f);
      if (t === 600) send({ t: "dev", dev: { op: "give", item: "apple", qty: 3 } }, f);
      if (t === 700) send({ t: "bar", slot: 6 }, f);
      if (sim.me.dialogue) send({ t: "closeDialogue" }, f);
      if (sim.frozen) continue;
      rec.frame(f);
      sim.tick(f);
    }
    const replay = rec.finish(sim);
    expect(replay.frames.length).toBeLessThan(replay.ticks); // run-length encoding did something
    const again = playReplay(catalog, JSON.parse(JSON.stringify(replay)));
    expect(again.state.tick).toBe(sim.state.tick);
    expect(hashState(again.state)).toBe(replay.finalHash);
  });
});
