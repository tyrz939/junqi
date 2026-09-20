// Replays fall out of the architecture: a run is (seed, per-tick input frames for
// every seat, commands stamped with the tick and the seat they arrived on). Frames
// are run-length encoded because held input barely changes. Playing a recording
// back through a fresh Sim must land on the same state hash; test/replay.test.ts
// holds the engine to that.
//
// This is also the wire format for lockstep co-op (PLATFORM.md): a network session
// is a replay being written by several hands at once. Sitting down and getting up
// are commands in the stream like any other, so a replay with four players in it
// re-simulates exactly.

import type { Catalog } from "@/sim/catalog";
import { hashState } from "@/sim/save";
import { HOST, NO_INPUT, Sim, type Command, type InputFrame } from "@/sim/sim";
import { MAX_PLAYERS } from "@/sim/state";

/** Five numbers per seat: mx, my, ax, ay, flags (bit0 sprint, bit1 useHeld). */
const PER_SEAT = 5;

export type Replay = {
  format: "jane-replay";
  version: 3;
  seed: number;
  /** The heroine's name, the host's choice. It is part of the world, so it is part of the recording. */
  name: string;
  /** [runLength, ...PER_SEAT numbers for seat 0, ...seat 1, ...]. Absent seats are NO_INPUT. */
  frames: number[][];
  /** [tick, seat, command]: applied before the tick with that number runs. Seat -1 is a join. */
  commands: [number, number, Command][];
  ticks: number;
  finalHash: string;
};

function pack(inputs: readonly InputFrame[]): number[] {
  const row: number[] = [1];
  for (const f of inputs) row.push(f.mx, f.my, f.ax, f.ay, (f.sprint ? 1 : 0) | (f.useHeld ? 2 : 0));
  // Trailing idle seats add nothing; trimming keeps solo recordings as small as before.
  while (row.length > 1 + PER_SEAT && row.slice(-PER_SEAT).every((v) => v === 0)) row.length -= PER_SEAT;
  return row;
}

function unpack(row: readonly number[]): InputFrame[] {
  const out: InputFrame[] = [];
  for (let i = 1; i + PER_SEAT <= row.length; i += PER_SEAT) {
    const flags = row[i + 4];
    out.push({ mx: row[i], my: row[i + 1], ax: row[i + 2], ay: row[i + 3], sprint: (flags & 1) !== 0, useHeld: (flags & 2) !== 0 });
  }
  while (out.length < MAX_PLAYERS) out.push(NO_INPUT);
  return out;
}

export class Recorder {
  private readonly frames: number[][] = [];
  private readonly commands: [number, number, Command][] = [];
  private ticks = 0;
  readonly seed: number;
  readonly name: string;

  /** Seat 0 is sat down by Sim.newGame, so the recording opens with the host's join. */
  constructor(seed: number, name = "") {
    this.seed = seed;
    this.name = name;
    this.commands.push([0, -1, { t: "join", who: HOST }]);
  }

  /** One call per tick that actually ran. `inputs` is indexed by seat. */
  frame(inputs: InputFrame | readonly InputFrame[]): void {
    const row = pack(Array.isArray(inputs) ? inputs : [inputs as InputFrame]);
    const last = this.frames[this.frames.length - 1];
    if (last && last.length === row.length && last.every((v, i) => i === 0 || v === row[i])) last[0]++;
    else this.frames.push(row);
    this.ticks++;
  }

  command(seat: number, c: Command): void {
    this.commands.push([this.ticks, seat, c]);
  }

  finish(sim: Sim): Replay {
    return {
      format: "jane-replay",
      version: 3,
      seed: this.seed,
      name: this.name,
      frames: this.frames,
      commands: this.commands,
      ticks: this.ticks,
      finalHash: hashState(sim.state),
    };
  }
}

/** Re-simulate a recording from scratch. Returns the sim so callers can inspect or hash it. */
export function playReplay(catalog: Catalog, replay: Replay): Sim {
  const sim = Sim.newGame(catalog, replay.seed, replay.name);
  let next = 1;
  let tick = 0;
  for (const row of replay.frames) {
    const inputs = unpack(row);
    for (let i = 0; i < row[0]; i++) {
      while (next < replay.commands.length && replay.commands[next][0] <= tick) {
        const [, seat, c] = replay.commands[next++];
        if (seat >= 0) sim.setAim(inputs[seat].ax, inputs[seat].ay, seat);
        sim.command(seat, c);
      }
      // A frozen sim (one player, talking) ignores ticks; the recorder only counts ticks that ran.
      sim.tick(inputs);
      tick++;
    }
  }
  while (next < replay.commands.length) {
    const [, seat, c] = replay.commands[next++];
    sim.command(seat, c);
  }
  return sim;
}
