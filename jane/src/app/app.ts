// The shell. Owns the four parts and the only place they meet:
//
//   Input  --edges-->  App --commands-->  Sim  --events-->  Renderer, UI
//          --frame--->      --tick(f)--->
//
// The sim never sees a device, a canvas or the clock on the wall. The renderer and
// the UI never write to the sim. Everything that changes the game goes through
// `command()` or `tick()`, which is also what makes a session recordable.

import { ICONS } from "@/art/icons";
import { PROP_SPRITES } from "@/art/props";
import { seatSheets, UNIT_SPRITES } from "@/art/units";
import { Loop } from "@/app/loop";
import { readSlot, slotInfos, writeSlot } from "@/app/storage";
import { runTerminal } from "@/app/terminal";
import { bindingRows, Input } from "@/input/input";
import { Renderer } from "@/render/renderer";
import { TILE_COLORS } from "@/render/tiles";
import { buildCatalog, type Catalog } from "@/sim/catalog";
import { TICK_RATE } from "@/sim/constants";
import { nearRest } from "@/sim/interact";
import { playReplay, Recorder } from "@/sim/replay";
import { cloneState, hashState } from "@/sim/save";
import { NO_INPUT, Sim, type Command, type InputFrame, type PlayerView } from "@/sim/sim";
import type { SlotInfo, UiApi, UiHost } from "@/ui/host";
import { createUi } from "@/ui/ui";

/** Working title. The game gets its real name later; the folder and package are `jane`. */
export const WORKING_TITLE = "Project Jane";
export const VERSION = "Project Jane 0.3.0";

export class App implements UiHost {
  readonly catalog: Catalog = buildCatalog();
  readonly version = VERSION;
  private readonly renderer: Renderer;
  private readonly input: Input;
  private readonly ui: UiApi;
  private readonly loop: Loop;
  private current: Sim | null = null;
  private recorder: Recorder | null = null;
  private frame: InputFrame = NO_INPUT;
  /** The slot this run saves into. */
  private slot = 0;
  private fps = 0;
  private fpsAcc = 0;
  private fpsFrames = 0;

  constructor(canvas: HTMLCanvasElement, uiRoot: HTMLElement) {
    this.renderer = new Renderer(canvas, UNIT_SPRITES, seatSheets(), PROP_SPRITES, ICONS);
    this.input = new Input(canvas);
    this.ui = createUi(uiRoot, this);
    window.addEventListener("resize", () => this.renderer.resize());
    this.loop = new Loop({ begin: () => this.begin(), tick: () => this.tick(), frame: (alpha, dt) => this.draw(alpha, dt) });
    this.loop.start();
  }

  /**
   * Which seat this machine is. Alone it is 0. Over a LAN it is whatever seat the host
   * handed out; nothing else in the app, the UI or the renderer needs to change.
   */
  private seat = 0;

  // --- UiHost ------------------------------------------------------------------
  /** This seat's view of the world. The UI and the renderer never see the whole sim. */
  sim(): PlayerView | null {
    return this.current ? this.current.view(this.seat) : null;
  }

  /** The whole simulation, for the console and tests. */
  world(): Sim | null {
    return this.current;
  }

  command(c: Command): void {
    const sim = this.current;
    if (!sim) return;
    sim.setAim(this.frame.ax, this.frame.ay, this.seat);
    // With a mouse, a cast carries who the cursor was over (0 = nobody). Friendly spells land on her.
    const view = this.sim();
    if ((c.t === "bar" || c.t === "cast") && view && this.input.aimingWithMouse) c = { ...c, on: this.renderer.hoveredFriend(view) };
    this.recorder?.command(this.seat, c);
    sim.command(this.seat, c);
  }

  /** A command for another seat (the console's `join` / `leave`; later, the network). Recorded like any other. */
  seatCommand(seat: number, c: Command): number {
    const sim = this.current;
    if (!sim) return -1;
    this.recorder?.command(seat, c);
    return sim.command(seat, c);
  }

  newGame(seed?: number, name?: string): void {
    const s = seed ?? (Math.floor(Math.random() * 0xffffffff) ^ Date.now()) >>> 0;
    this.current = Sim.newGame(this.catalog, s, name);
    this.recorder = new Recorder(s, name);
    // Resting writes here. A new run takes the first empty slot, else the oldest save.
    const infos = slotInfos();
    const empty = infos.findIndex((i) => i === null);
    this.slot = empty >= 0 ? empty : infos.reduce((best, i, n) => (i && i.savedAt < (infos[best]?.savedAt ?? "~") ? n : best), 0);
  }

  slots(): (SlotInfo | null)[] {
    return slotInfos();
  }

  canSave(): boolean {
    const view = this.sim();
    return view !== null && view.player.alive && nearRest(view);
  }

  /** Saving is a place, not a menu: a bed or a fire. No fast travel, no save-scumming a bad night. */
  save(slot: number): boolean {
    if (!this.current) return false;
    if (!this.canSave()) {
      this.ui.toast("You can only save at a bed or a fire");
      return false;
    }
    this.slot = slot;
    const ok = writeSlot(slot, this.current);
    this.ui.toast(ok ? `Saved to slot ${slot + 1}` : "Could not save: storage is unavailable");
    return ok;
  }

  load(slot: number): boolean {
    const file = readSlot(slot);
    if (!file) {
      this.ui.toast("Nothing in that slot");
      return false;
    }
    try {
      this.current = Sim.fromState(this.catalog, cloneState(file.state));
      this.slot = slot;
      this.recorder = null; // a loaded game has no recording from tick 0
      this.ui.toast(`Loaded slot ${slot + 1}`);
      return true;
    } catch (err) {
      this.ui.toast("That save could not be loaded");
      console.error(err);
      return false;
    }
  }

  toTitle(): void {
    this.current = null;
    this.recorder = null;
  }

  iconUrl(icon: string): string {
    return this.renderer.iconUrl(icon);
  }

  tileColors(): readonly string[] {
    return TILE_COLORS;
  }

  terminal(line: string): string[] {
    return runTerminal(this, line);
  }

  hints(): { confirm: string; cancel: string; choose: string; use: string } {
    return this.input.padActive
      ? { confirm: "A", cancel: "B", choose: "D-pad", use: "B" }
      : { confirm: "E", cancel: "Esc", choose: "W S", use: "E" };
  }

  bindings(): { action: string; keys: string; pad: string }[] {
    return bindingRows();
  }

  debugText(): string {
    const sim = this.sim();
    if (!sim) return "";
    const p = sim.player;
    const r = this.renderer.stats;
    const awake = sim.zone.units.filter((u) => u.awake).length;
    const path = sim.rt.path.stats;
    return [
      `${VERSION}   seed ${sim.state.seed}   tick ${sim.state.tick}`,
      `fps ${this.fps.toFixed(0)}   sim ${sim.lastTickMs.toFixed(2)} ms   draw ${this.renderer.frameMs.toFixed(2)} ms   x${r.scale}`,
      `seat ${this.seat + 1} of ${sim.party.size()}   zone ${sim.me.zone} ${sim.rt.grid.w}x${sim.rt.grid.h}   cell ${Math.floor(p.x / 8)},${Math.floor(p.y / 8)}   ${formatHour(sim.hour)} day ${sim.state.day}`,
      `units ${awake}/${sim.zone.units.length} awake   props ${sim.zone.props.length}   bolts ${sim.zone.projectiles.length}   drops ${sim.zone.drops.length}`,
      `chunks ${r.chunks} live / ${r.chunksBuilt} built   particles ${r.particles}`,
      `paths ${path.searches} searches, ${path.failed} failed, ${path.searches ? Math.round(path.expanded / path.searches) : 0} nodes avg`,
      `dropped ticks ${this.loop.droppedTicks}   hash ${sim.state.tick % 30 === 0 ? (this.lastHash = hashState(sim.state)) : this.lastHash}`,
    ].join("\n");
  }
  private lastHash = "--------";

  // --- terminal helpers ----------------------------------------------------------
  setTimeScale(x: number): void {
    this.loop.timeScale = x;
  }

  replay(mode: string): string[] {
    const sim = this.current;
    if (!sim || !this.recorder) return ["No recording: replays start at New Game and end at the first load."];
    const rec = this.recorder.finish(sim);
    if (mode === "export") {
      const blob = new Blob([JSON.stringify(rec)], { type: "application/json" });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = `jane-${rec.seed}-${rec.ticks}.replay.json`;
      a.click();
      URL.revokeObjectURL(a.href);
      return [`exported ${rec.ticks} ticks, ${rec.frames.length} input runs, ${rec.commands.length} commands`];
    }
    const t0 = performance.now();
    const again = playReplay(this.catalog, rec);
    const hash = hashState(again.state);
    const ms = performance.now() - t0;
    return [
      `${rec.ticks} ticks (${(rec.ticks / TICK_RATE).toFixed(0)} s) re-simulated in ${ms.toFixed(0)} ms`,
      `live ${rec.finalHash}   replay ${hash}   ${hash === rec.finalHash ? "MATCH" : "DIVERGED"}`,
    ];
  }

  // --- loop ------------------------------------------------------------------------
  /** Alone, a window or menu pauses the world. With company nothing can: others are still playing. */
  private paused(): boolean {
    const sim = this.current;
    return !sim || sim.frozen || (this.ui.blocksWorld() && sim.party.size() <= 1);
  }

  private begin(): void {
    const sim = this.sim();
    // `blocked` is about THIS seat's keys going to the UI, whether or not the world is paused.
    const blocked = this.ui.blocksWorld() || !sim || sim.me.dialogue !== null;
    this.input.captured = this.ui.capturesKeys();
    this.input.uiMode = blocked;
    const cursor = sim && this.input.aimingWithMouse ? this.renderer.screenToWorld(this.input.mouseX, this.input.mouseY) : null;
    this.frame = this.input.frame(sim ? sim.player : null, cursor);
    this.renderer.cursor = sim && !blocked && this.input.aimingWithMouse ? { x: this.input.mouseX, y: this.input.mouseY } : null;

    for (const edge of this.input.drain()) {
      if ("ui" in edge) {
        this.ui.action(edge.ui);
        continue;
      }
      const g = edge.game;
      if (g.t === "grid") this.renderer.showGrid = !this.renderer.showGrid;
      else if (g.t === "quicksave") this.save(this.slot);
      else if (g.t === "quickload") this.load(this.slot);
      else if (!blocked) this.command(g);
    }
  }

  private tick(): void {
    const sim = this.current;
    const view = this.sim();
    if (!sim || !view || this.paused()) return;
    this.renderer.snapshot(view);
    // Inputs are indexed by seat. A window open in co-op means this seat stands still.
    const inputs: InputFrame[] = [];
    inputs[this.seat] = this.ui.blocksWorld() ? NO_INPUT : this.frame;
    for (let i = 0; i < inputs.length; i++) inputs[i] ??= NO_INPUT;
    this.recorder?.frame(inputs);
    sim.tick(inputs);
  }

  private draw(alpha: number, dt: number): void {
    this.fpsAcc += dt;
    if (++this.fpsFrames >= 30) {
      this.fps = this.fpsFrames / this.fpsAcc;
      this.fpsAcc = 0;
      this.fpsFrames = 0;
    }
    const view = this.sim();
    // Only what this seat should see and hear: her own, her zone's, the party's.
    const events = this.current && view ? Sim.eventsFor(this.current.drainEvents(), view.me) : [];
    // Resting at a bed or a fire is the save.
    if (view && events.some((e) => e.e === "rest")) this.save(this.slot);
    if (view) {
      this.renderer.handle(events, view.me.unitId);
      this.renderer.draw(view, this.paused() ? 1 : alpha);
    }
    this.ui.frame(events);
  }
}

export function formatHour(hour: number): string {
  const h = Math.floor(hour) % 24;
  const m = Math.floor((hour - Math.floor(hour)) * 60);
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}
