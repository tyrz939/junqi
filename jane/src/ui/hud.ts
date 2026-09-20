// The in-game HUD: vitals, statuses, target / boss frame, zone + clock + quest
// tracker, action bar, interact prompt, toasts, death veil. Everything is built
// once; `frame` only pushes values through cached setters.

import { BAR_SLOTS, ENERGY_MAX, GCD_TICKS } from "@/sim/constants";
import type { SimEvent } from "@/sim/events";
import { focusOf } from "@/sim/interact";
import { bagCount } from "@/sim/inventory";
import { requirementCount } from "@/sim/quests";
import type { PlayerView as Sim } from "@/sim/sim";
import type { BarSlot, Unit } from "@/sim/state";
import { isEnemy, maxHp, maxMp } from "@/sim/units";
import type { Ctx } from "@/ui/ctx";
import { cached, classOf, clear, h, iconOf, SlotView, textOf, varOf, visOf } from "@/ui/dom";
import { barSlotSig, clamp01, clockText, dayText, isNight, statusTime, sweepPercent, useKeyLabel } from "@/ui/format";
import type { UiAction } from "@/ui/host";

const TOAST_MS = 3000;
const TOAST_FADE_MS = 400;
const TOAST_MAX = 3;
const TARGET_MS = 6000;
const STATUS_CHIPS = 8;
const TRACKER_QUESTS = 4;

// --- meters ------------------------------------------------------------------------

class Meter {
  readonly el: HTMLDivElement;
  private readonly setFill: (v: number) => void;
  private readonly setNum: (s: string) => void;

  /**
   * `numbers` is for Jane's own vitals only. An enemy's health is a bar and never a
   * figure: threat in this game is felt, not read (PLAN.md 2.6).
   */
  constructor(parent: HTMLElement, kind: string, label: string, private readonly numbers = true) {
    this.el = h("div", `jq-meter jq-meter-${kind}`, parent);
    if (label) h("span", "jq-meter-label", this.el, label);
    const track = h("div", "jq-meter-track", this.el);
    const fill = h("div", "jq-meter-fill", track);
    const num = h("span", "jq-meter-num", track);
    this.setFill = cached<number>((v) => {
      fill.style.transform = `scaleX(${v})`;
    });
    this.setNum = textOf(num);
  }

  set(value: number, max: number): void {
    const f = max > 0 ? clamp01(value / max) : 0;
    this.setFill(Math.round(f * 250) / 250);
    this.setNum(this.numbers ? `${Math.max(0, Math.ceil(value))}/${Math.round(max)}` : "");
  }
}

// --- toasts ------------------------------------------------------------------------

type LiveToast = { el: HTMLElement; text: string; born: number; dying: boolean };

export class Toasts {
  readonly el: HTMLDivElement;
  private live: LiveToast[] = [];
  private pushed = 0;

  constructor() {
    this.el = h("div", "jq-toasts");
  }

  get count(): number {
    return this.pushed;
  }

  push(text: string): void {
    this.pushed++;
    const now = performance.now();
    const same = this.live.find((t) => t.text === text && !t.dying);
    if (same) {
      same.born = now;
      return;
    }
    const el = h("div", "jq-toast", this.el, text);
    this.live.push({ el, text, born: now, dying: false });
    while (this.live.length > TOAST_MAX) this.live.shift()?.el.remove();
  }

  frame(now: number): void {
    if (this.live.length === 0) return;
    for (let i = this.live.length - 1; i >= 0; i--) {
      const t = this.live[i];
      const age = now - t.born;
      if (age >= TOAST_MS) {
        t.el.remove();
        this.live.splice(i, 1);
      } else if (!t.dying && age >= TOAST_MS - TOAST_FADE_MS) {
        t.dying = true;
        t.el.classList.add("out");
      }
    }
  }
}

// --- action bar --------------------------------------------------------------------

class BarSlotView extends SlotView {
  readonly setCd: (v: string) => void;
  readonly setGcd: (v: string) => void;
  readonly setDim: (on: boolean) => void;
  flashAt = 0;

  constructor(index: number) {
    super("jq-barslot");
    this.el.dataset.drop = `bar:${index}`;
    const cd = h("div", "jq-cd", this.el);
    const gcd = h("div", "jq-cd jq-gcd", this.el);
    h("span", "jq-key", this.el, String(index + 1));
    this.setCd = varOf(cd, "--p");
    this.setGcd = varOf(gcd, "--p");
    this.setDim = classOf(this.el, "dim");
  }
}

export function barIconId(sim: Sim, slot: BarSlot): string {
  if (!slot) return "";
  return (slot.source === "spell" ? sim.catalog.spells[slot.id]?.icon : sim.catalog.items[slot.id]?.icon) ?? "";
}

export class ActionBar {
  readonly el: HTMLDivElement;
  readonly slots: BarSlotView[] = [];

  /** `editing` is true while the window is open: that is when bar slots can be dragged. */
  constructor(
    private readonly ctx: Ctx,
    editing: () => boolean,
  ) {
    this.el = h("div", "jq-bar jq-panel");
    this.el.dataset.drop = "barbg";
    const { host, drag, tip } = ctx;
    for (let i = 0; i < BAR_SLOTS; i++) {
      const v = new BarSlotView(i);
      this.el.appendChild(v.el);
      this.slots.push(v);
      drag.attach(v.el, {
        payload: () => {
          const sim = host.sim();
          return sim && editing() && sim.me.bar[i] ? { kind: "bar", slot: i } : null;
        },
        iconId: () => {
          const sim = host.sim();
          return sim ? barIconId(sim, sim.me.bar[i]) : "";
        },
        click: () => host.command({ t: "bar", slot: i }),
      });
      v.el.addEventListener("pointerenter", (ev) => {
        const sim = host.sim();
        if (ev.pointerType === "mouse" && sim && !drag.active()) tip.bar(sim.me.bar[i], v.el);
      });
      v.el.addEventListener("pointerleave", () => tip.hide());
    }
  }

  frame(sim: Sim, events: SimEvent[], now: number): void {
    const p = sim.player;
    const cat = sim.catalog;
    for (const ev of events) {
      if (ev.e !== "castFailed" || ev.unit !== p.id) continue;
      sim.me.bar.forEach((b, i) => {
        if (b && b.source === "spell" && b.id === ev.spell) this.flash(i, now);
      });
    }
    for (let i = 0; i < this.slots.length; i++) {
      const v = this.slots[i];
      const b = sim.me.bar[i] ?? null;
      if (v.flashAt > 0 && now - v.flashAt > 360) {
        v.flashAt = 0;
        v.el.classList.remove("flash");
      }
      if (!b) {
        v.set("", "", "");
        v.setCd("100%");
        v.setGcd("100%");
        v.setDim(false);
        continue;
      }
      if (b.source === "spell") {
        const def = cat.spells[b.id];
        v.set(barSlotSig(b, 0), def ? this.ctx.icon(def.icon) : "", "");
        if (!def) continue;
        v.setCd(`${sweepPercent(p.cooldowns[b.id] ?? 0, def.cooldown)}%`);
        v.setGcd(`${def.gcdImmune ? 100 : sweepPercent(p.gcd, GCD_TICKS)}%`);
        v.setDim(p.mp < def.mp || p.energy < def.energy || !p.book.includes(b.id));
      } else {
        const def = cat.items[b.id];
        const qty = bagCount(p, b.id);
        v.set(barSlotSig(b, qty), def ? this.ctx.icon(def.icon) : "", String(qty));
        if (!def) continue;
        v.setCd(`${sweepPercent(p.itemCooldowns[b.id] ?? 0, def.cooldown)}%`);
        v.setGcd(`${sweepPercent(p.gcd, GCD_TICKS)}%`);
        v.setDim(qty === 0);
      }
    }
  }

  private flash(i: number, now: number): void {
    const v = this.slots[i];
    v.el.classList.remove("flash");
    void v.el.offsetWidth; // restart the animation
    v.el.classList.add("flash");
    v.flashAt = now;
  }
}

// --- the HUD proper ----------------------------------------------------------------

type Chip = {
  setVis: (on: boolean) => void;
  setIcon: (url: string) => void;
  setTime: (s: string) => void;
  setHarm: (on: boolean) => void;
  setTitle: (s: string) => void;
};

export class Hud {
  readonly el: HTMLDivElement;

  private readonly setName: (s: string) => void;
  private readonly hp: Meter;
  private readonly mp: Meter;
  private readonly en: Meter;
  private readonly setLocked: (on: boolean) => void;
  private readonly chips: Chip[] = [];

  private readonly setTargetVis: (on: boolean) => void;
  private readonly setBoss: (on: boolean) => void;
  private readonly setTargetName: (s: string) => void;
  private readonly targetHp: Meter;
  private targetId = 0;
  private targetAt = 0;

  private readonly setZone: (s: string) => void;
  private readonly setClock: (s: string) => void;
  private readonly setDay: (s: string) => void;
  private readonly setNight: (on: boolean) => void;
  private readonly tracker: HTMLDivElement;
  private trackerSig = "\u0000";

  private readonly setPromptVis: (on: boolean) => void;
  private readonly setPrompt: (s: string) => void;
  private promptText = "";
  private useKey = "";

  private readonly setDead: (on: boolean) => void;
  private readonly banner: HTMLDivElement;
  private readonly setButtonsVis: (on: boolean) => void;

  constructor(private readonly ctx: Ctx) {
    this.el = h("div", "jq-hud");

    // vitals
    const vit = h("div", "jq-vitals jq-panel", this.el);
    this.setName = textOf(h("div", "jq-vitals-name", vit));
    this.hp = new Meter(vit, "hp", "HP");
    this.mp = new Meter(vit, "mp", "MP");
    this.en = new Meter(vit, "en", "EN");
    this.setLocked = classOf(this.en.el, "locked");
    const chipRow = h("div", "jq-statuses", this.el);
    for (let i = 0; i < STATUS_CHIPS; i++) {
      const chip = h("div", "jq-status", chipRow);
      const icon = h("div", "jq-icon", chip);
      const time = h("span", "jq-status-time", chip);
      chip.hidden = true;
      this.chips.push({
        setVis: visOf(chip),
        setIcon: iconOf(icon),
        setTime: textOf(time),
        setHarm: classOf(chip, "harm"),
        setTitle: cached<string>((s) => {
          chip.title = s;
        }),
      });
    }

    // target / boss
    const target = h("div", "jq-target jq-panel", this.el);
    target.hidden = true;
    this.setTargetVis = visOf(target);
    this.setBoss = classOf(target, "boss");
    this.setTargetName = textOf(h("div", "jq-target-name", target));
    this.targetHp = new Meter(target, "hp", "", false);

    // zone, clock, quests
    const zone = h("div", "jq-zone", this.el);
    this.setZone = textOf(h("div", "jq-zone-name", zone));
    const timeRow = h("div", "jq-zone-time", zone);
    const sun = h("span", "jq-sun", timeRow);
    this.setNight = classOf(sun, "night");
    this.setClock = textOf(h("span", "jq-clock", timeRow));
    this.setDay = textOf(h("span", "jq-day", timeRow));
    this.tracker = h("div", "jq-tracker", zone);

    // interact prompt (clickable: a pointer-only player has no E key)
    const prompt = h("div", "jq-prompt jq-panel", this.el);
    prompt.hidden = true;
    this.setPromptVis = visOf(prompt);
    this.setPrompt = textOf(prompt);
    prompt.addEventListener("click", () => ctx.host.command({ t: "use" }));

    // window buttons (pointer access to everything the keyboard opens)
    const buttons = h("div", "jq-hudbtns", this.el);
    this.setButtonsVis = visOf(buttons);
    const defs: [string, UiAction][] = [
      ["Bag", "bags"],
      ["Book", "book"],
      ["Quests", "quests"],
      ["Map", "map"],
      ["Menu", "pause"],
    ];
    for (const [label, action] of defs) {
      const b = h("div", "jq-btn", buttons, label);
      b.addEventListener("click", () => ctx.act(action));
    }

    this.banner = h("div", "jq-banner", this.el);

    const dead = h("div", "jq-death", this.el);
    dead.hidden = true;
    h("div", "jq-death-big", dead, "You fell.");
    h("div", "jq-death-small", dead, "Waking where you came in...");
    this.setDead = visOf(dead);
  }

  /** New sim (new game, load): forget anything that referred to the old one. */
  reset(): void {
    this.targetId = 0;
    this.trackerSig = "\u0000";
    this.promptText = "";
  }

  frame(sim: Sim, events: SimEvent[], now: number, frameNo: number, blocked: boolean): void {
    const p = sim.player;
    const cat = sim.catalog;

    this.setName(sim.state.name);
    this.hp.set(p.hp, maxHp(p));
    this.mp.set(p.mp, maxMp(p));
    this.en.set(p.energy, ENERGY_MAX);
    this.setLocked(p.energyLocked);

    for (let i = 0; i < this.chips.length; i++) {
      const chip = this.chips[i];
      const st = p.statuses[i];
      const def = st ? cat.effects[st.effect] : undefined;
      chip.setVis(!!def);
      if (!st || !def) continue;
      chip.setIcon(this.ctx.icon(def.icon));
      chip.setTime(statusTime(st.left));
      chip.setHarm(def.harmful);
      chip.setTitle(def.name);
    }

    this.updateTarget(sim, p, events, now);

    this.setZone(sim.rt.bp.name);
    this.setClock(clockText(sim.hour));
    this.setDay(dayText(sim.state.day));
    this.setNight(isNight(sim.hour));

    let questTouched = false;
    for (const ev of events) {
      if (ev.e === "quest" || ev.e === "bag" || ev.e === "loot") questTouched = true;
      else if (ev.e === "zone") this.showBanner(ev.name);
    }
    if (questTouched || frameNo % 10 === 0) this.updateTracker(sim);

    const quiet = blocked || !p.alive || sim.me.dialogue !== null;
    if (quiet) this.promptText = "";
    else if (frameNo % 3 === 0) this.promptText = focusOf(sim, p)?.prompt ?? "";
    this.setPromptVis(this.promptText !== "");
    if (this.promptText !== "") {
      // Follows the device touched last: [E] on a keyboard, [B] on a pad.
      this.useKey = this.ctx.host.hints().use || useKeyLabel(this.ctx.host.bindings());
      this.setPrompt(`[${this.useKey}] ${this.promptText}`);
    }

    this.setButtonsVis(!blocked && sim.me.dialogue === null);
    this.setDead(!p.alive);
  }

  private updateTarget(sim: Sim, p: Unit, events: SimEvent[], now: number): void {
    const units = sim.rt.units;
    for (const ev of events) {
      if (ev.e === "damage") {
        if (ev.unit === p.id) {
          const cur = units.get(this.targetId);
          if (cur && cur.alive && cur.target === p.id) this.targetAt = now;
          else {
            const foe = this.attackerOf(sim, p);
            if (foe) {
              this.targetId = foe.id;
              this.targetAt = now;
            }
          }
        } else {
          const victim = units.get(ev.unit);
          if (victim && victim.alive && isEnemy(p, victim)) {
            this.targetId = victim.id;
            this.targetAt = now;
          }
        }
      } else if (ev.e === "death" && ev.unit === this.targetId) {
        this.targetId = 0;
      }
    }

    let shown: Unit | null = null;
    let boss = false;
    for (const other of sim.zone.units) {
      if (other.alive && other.awake && other.combat === "combat" && sim.catalog.units[other.def]?.boss) {
        shown = other;
        boss = true;
        break;
      }
    }
    if (!shown && this.targetId !== 0) {
      const t = units.get(this.targetId);
      if (t && t.alive && now - this.targetAt < TARGET_MS) shown = t;
      else this.targetId = 0;
    }
    this.setTargetVis(shown !== null);
    if (!shown) return;
    this.setBoss(boss);
    this.setTargetName(sim.catalog.units[shown.def]?.name ?? shown.def);
    this.targetHp.set(shown.hp, maxHp(shown));
  }

  /** Damage events do not name their source; whoever is closest and hunting the player is the best reading. */
  private attackerOf(sim: Sim, p: Unit): Unit | null {
    let best: Unit | null = null;
    let bestD = Infinity;
    for (const other of sim.zone.units) {
      if (!other.alive || !other.awake || other.target !== p.id || !isEnemy(p, other)) continue;
      const dx = other.x - p.x;
      const dy = other.y - p.y;
      const d = dx * dx + dy * dy;
      if (d < bestD) {
        bestD = d;
        best = other;
      }
    }
    return best;
  }

  private updateTracker(sim: Sim): void {
    const cat = sim.catalog;
    type Row = { name: string; ready: boolean; reqs: { text: string; done: boolean }[] };
    const rows: Row[] = [];
    let sig = "";
    const active = sim.state.quests.active;
    for (let q = 0; q < active.length && rows.length < TRACKER_QUESTS; q++) {
      const prog = active[q];
      const def = cat.quests[prog.quest];
      if (!def) continue;
      let ready = true;
      const reqs = def.requirements.map((r, i) => {
        const n = requirementCount(sim, def, prog, i);
        const done = n >= r.qty;
        if (!done) ready = false;
        return { text: `${r.text} ${n}/${r.qty}`, done };
      });
      rows.push({ name: def.name, ready, reqs });
      sig += `${prog.quest}:${reqs.map((r) => r.text).join(",")};`;
    }
    const more = active.length - rows.length;
    sig += more;
    if (sig === this.trackerSig) return;
    this.trackerSig = sig;
    clear(this.tracker);
    for (const row of rows) {
      const q = h("div", `jq-tq${row.ready ? " ready" : ""}`, this.tracker);
      h("div", "jq-tq-name", q, row.ready ? `${row.name} - ready` : row.name);
      if (!row.ready) for (const r of row.reqs) h("div", `jq-tq-req${r.done ? " done" : ""}`, q, r.text);
    }
    if (more > 0) h("div", "jq-tq-more", this.tracker, `+${more} more`);
  }

  private showBanner(name: string): void {
    this.banner.textContent = name;
    this.banner.classList.remove("show");
    void this.banner.offsetWidth; // restart the animation
    this.banner.classList.add("show");
  }
}
