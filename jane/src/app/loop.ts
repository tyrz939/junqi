// Fixed-step accumulator. The sim advances in whole 1/60 s ticks no matter what
// the display does; the renderer gets `alpha` (0..1 into the next tick) and
// interpolates. A 144 Hz monitor and a throttled background tab run the same game.

import { TICK_SECONDS } from "@/sim/constants";

export type LoopHooks = {
  /** Once per display frame, before any ticks: sample devices, route edge-triggered input. */
  begin: () => void;
  /** Advance the sim exactly one tick. */
  tick: () => void;
  /** Draw. `alpha` is how far real time is past the last completed tick. */
  frame: (alpha: number, dtSeconds: number) => void;
};

/** Never simulate more than this many ticks in one frame; drop the rest (spiral-of-death guard). */
const MAX_CATCH_UP = 5;

export class Loop {
  private readonly hooks: LoopHooks;
  private raf = 0;
  private last = 0;
  private acc = 0;
  private running = false;
  /** 1 = real time. The terminal's `speed` command writes this. */
  timeScale = 1;
  /** Ticks simulated in the most recent frame, for the debug overlay. */
  ticksLastFrame = 0;
  droppedTicks = 0;

  constructor(hooks: LoopHooks) {
    this.hooks = hooks;
  }

  start(): void {
    if (this.running) return;
    this.running = true;
    this.last = performance.now();
    this.acc = 0;
    this.raf = requestAnimationFrame(this.step);
  }

  stop(): void {
    this.running = false;
    cancelAnimationFrame(this.raf);
  }

  private readonly step = (now: number): void => {
    if (!this.running) return;
    const dt = Math.min(0.25, (now - this.last) / 1000);
    this.last = now;
    this.acc += dt * this.timeScale;
    this.hooks.begin();
    let n = 0;
    while (this.acc >= TICK_SECONDS) {
      this.acc -= TICK_SECONDS;
      if (n < MAX_CATCH_UP * Math.max(1, Math.ceil(this.timeScale))) {
        this.hooks.tick();
        n++;
      } else {
        this.droppedTicks++;
      }
    }
    this.ticksLastFrame = n;
    this.hooks.frame(this.acc / TICK_SECONDS, dt);
    this.raf = requestAnimationFrame(this.step);
  };
}
