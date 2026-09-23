// The one tabbed window: Inventory / Spellbook / Quests / Map. The world is
// paused while it is open. Pointer and pad share one vocabulary: a DragPayload
// lands on a DropTarget, whether it got there by dragging or by "pick up, move
// the cursor, put down" (see nav.ts for the focus model).

import { BAG_SLOTS, CRAFT_INPUTS } from "@/sim/constants";
import type { SimEvent } from "@/sim/events";
import { nearBench } from "@/sim/interact";
import { craftOutput } from "@/sim/inventory";
import { questReady, requirementCount } from "@/sim/quests";
import { expandText } from "@/sim/text";
import type { PlayerView as Sim } from "@/sim/sim";
import { maxHp, maxMp } from "@/sim/units";
import type { Ctx } from "@/ui/ctx";
import { clear, h, SlotView, textOf } from "@/ui/dom";
import { costText, durationText, firstEmpty, rangeText, stackSig } from "@/ui/format";
import { barIconId, type ActionBar } from "@/ui/hud";
import type { UiAction } from "@/ui/host";
import { MapPane } from "@/ui/map";
import {
  cursorTarget,
  cycleTab,
  homeCursor,
  moveCursor,
  TABS,
  type Cursor,
  type DragPayload,
  type DropTarget,
  type NavLayout,
  type TabId,
} from "@/ui/nav";
import { Popover, type PopItem } from "@/ui/popover";

const TAB_LABEL: Record<TabId, string> = { inventory: "Inventory", book: "Spellbook", quests: "Quests", map: "Map" };

export const ACTION_TAB: Partial<Record<UiAction, TabId>> = { bags: "inventory", book: "book", quests: "quests", map: "map" };

const POINTER_HINT: Record<TabId, string> = {
  inventory: "Drag to move - right-click or double-tap to use - drag out of the window to destroy",
  book: "Drag a spell onto the bar, or click it to bind the first empty slot",
  quests: "",
  map: "Wheel or + / -: zoom - drag: look round - 0: back to Jane",
};

export class GameWindow {
  readonly el: HTMLDivElement;
  readonly popover: Popover;

  private opened = false;
  private tab: TabId = "inventory";
  private readonly tabBtns = new Map<TabId, HTMLElement>();
  private readonly panes = new Map<TabId, HTMLElement>();
  private readonly setHint: (s: string) => void;

  // inventory
  private readonly bagSlots: SlotView[] = [];
  private readonly craftSlots: SlotView[] = [];
  private readonly craftStrip: HTMLDivElement;
  private readonly setCraftHint: (s: string) => void;
  private readonly statRows: ((s: string) => void)[] = [];
  private setCardName: (s: string) => void = () => {};
  private benchOk = false;

  // spellbook
  private readonly bookList: HTMLDivElement;
  private bookRows: HTMLElement[] = [];
  private bookSig = "\u0000";

  // quests
  private readonly questList: HTMLDivElement;
  private readonly questDetail: HTMLDivElement;
  private questRows: HTMLElement[] = [];
  private questIds: string[] = [];
  private questListSig = "\u0000";
  private questDetailSig = "\u0000";
  private questSel = "";

  private readonly map: MapPane;

  // pad focus
  private cursor: Cursor = { region: "bag", index: 0 };
  private held: DragPayload | null = null;
  private keysMode = false;
  private cursorEl: HTMLElement | null = null;
  private cursorKey = "";

  constructor(
    private readonly ctx: Ctx,
    private readonly bar: ActionBar,
  ) {
    this.el = h("div", "jq-winlayer");
    this.el.dataset.drop = "outside";
    this.el.hidden = true;

    const win = h("div", "jq-window jq-panel", this.el);
    win.dataset.drop = "window";

    const head = h("div", "jq-tabs", win);
    for (const t of TABS) {
      const b = h("div", "jq-tab", head, TAB_LABEL[t]);
      b.addEventListener("click", () => this.setTab(t));
      this.tabBtns.set(t, b);
    }
    const close = h("div", "jq-tab jq-close", head, "x");
    close.addEventListener("click", () => this.close());

    const body = h("div", "jq-winbody", win);
    for (const t of TABS) {
      const pane = h("div", `jq-pane jq-pane-${t}`, body);
      pane.hidden = true;
      this.panes.set(t, pane);
    }
    this.setHint = textOf(h("div", "jq-winhint", win));

    // --- inventory pane
    const inv = this.pane("inventory");
    const left = h("div", "jq-inv-left", inv);
    const grid = h("div", "jq-bag", left);
    for (let i = 0; i < BAG_SLOTS; i++) {
      const v = new SlotView("jq-bagslot");
      v.el.dataset.drop = `bag:${i}`;
      grid.appendChild(v.el);
      this.bagSlots.push(v);
      this.wireBagSlot(v, i);
    }
    this.craftStrip = h("div", "jq-craft", left);
    const craftHead = h("div", "jq-craft-head", this.craftStrip);
    h("span", "jq-craft-label", craftHead, "Craft");
    this.setCraftHint = textOf(h("span", "jq-craft-hint", craftHead));
    const craftRow = h("div", "jq-craft-row", this.craftStrip);
    for (let i = 0; i < CRAFT_INPUTS; i++) {
      const v = new SlotView("jq-craftslot");
      v.el.dataset.drop = `craft:${i}`;
      v.el.addEventListener("click", () => {
        if (this.benchOk) ctx.host.command({ t: "craftClear", slot: i });
      });
      craftRow.appendChild(v.el);
      this.craftSlots.push(v);
    }
    h("span", "jq-craft-arrow", craftRow);
    const out = new SlotView("jq-craftslot jq-craftout");
    out.el.addEventListener("click", () => {
      if (this.benchOk) ctx.host.command({ t: "craftTake" });
    });
    out.el.addEventListener("pointerenter", (ev) => {
      const sim = ctx.host.sim();
      const o = sim ? craftOutput(sim) : null;
      if (ev.pointerType === "mouse" && o) ctx.tip.item(o.item, out.el);
    });
    out.el.addEventListener("pointerleave", () => ctx.tip.hide());
    craftRow.appendChild(out.el);
    this.craftSlots.push(out);

    const card = h("div", "jq-card", inv);
    this.setCardName = textOf(h("div", "jq-card-title", card, "Jane"));
    for (const label of ["Health", "Mana", "Strength", "Spirit", "Kills", "Falls"]) {
      const row = h("div", "jq-card-row", card);
      h("span", "", row, label);
      this.statRows.push(textOf(h("span", "jq-gold", row)));
    }

    // --- spellbook pane
    this.bookList = h("div", "jq-book", this.pane("book"));

    // --- quests pane
    const qp = this.pane("quests");
    this.questList = h("div", "jq-qlist", qp);
    this.questDetail = h("div", "jq-qdetail", qp);

    // --- map pane
    this.map = new MapPane(ctx);
    this.pane("map").appendChild(this.map.el);

    this.popover = new Popover(ctx.root);

    ctx.drag.onDrop = (p, t, x, y) => this.drop(p, t, x, y);
    ctx.drag.onStart = () => ctx.tip.hide();

    // Any real pointer activity hands focus back to the pointer: the pad cursor ring goes away.
    const toPointer = (ev: PointerEvent): void => {
      if (!this.opened || !this.keysMode) return;
      if (ev.type === "pointermove" && ev.movementX === 0 && ev.movementY === 0) return;
      this.keysMode = false;
      this.held = null;
      ctx.tip.hide();
    };
    ctx.root.addEventListener("pointermove", toPointer);
    ctx.root.addEventListener("pointerdown", toPointer, true);
  }

  get isOpen(): boolean {
    return this.opened;
  }

  /** Who is driving: true shows the pad cursor ring, false leaves hover to the pointer. */
  usingKeys(on: boolean): void {
    if (this.keysMode === on) return;
    this.keysMode = on;
    if (!on) this.held = null;
  }

  toggle(tab: TabId): void {
    if (this.opened && this.tab === tab) this.close();
    else this.show(tab);
  }

  show(tab: TabId): void {
    const sim = this.ctx.host.sim();
    if (!sim) return;
    if (!this.opened) {
      this.opened = true;
      this.el.hidden = false;
      this.benchOk = nearBench(sim);
      this.map.invalidate();
    }
    this.setTab(tab);
  }

  /** `send` false when the sim itself went away (load, quit): there is nothing to clear. */
  close(send = true): void {
    if (!this.opened) return;
    this.opened = false;
    this.el.hidden = true;
    this.held = null;
    this.popover.close();
    this.ctx.tip.hide();
    this.ctx.drag.showHeld("", null);
    this.setCursorEl(null);
    this.cursorKey = "";
    if (send) this.ctx.host.command({ t: "craftClearAll" });
  }

  action(a: UiAction): boolean {
    if (!this.opened) return false;
    this.keysMode = true;
    if (this.popover.isOpen) return this.popover.action(a);
    const tab = ACTION_TAB[a];
    if (tab) {
      this.toggle(tab);
      return true;
    }
    if (this.tab === "map" && (a === "up" || a === "down" || a === "left" || a === "right" || a === "confirm")) {
      this.map.action(a);
      return true;
    }
    switch (a) {
      case "tabLeft":
        this.setTab(cycleTab(this.tab, -1));
        break;
      case "tabRight":
        this.setTab(cycleTab(this.tab, 1));
        break;
      case "cancel":
        if (this.held) this.dropHeld();
        else this.close();
        break;
      case "up":
      case "down":
      case "left":
      case "right":
        this.cursor = moveCursor(this.cursor, a, this.layout());
        if (this.tab === "quests" && this.cursor.region === "list") this.questSel = this.questIds[this.cursor.index] ?? this.questSel;
        break;
      case "confirm":
        this.confirm();
        break;
      default:
        break;
    }
    return true;
  }

  frame(sim: Sim, events: SimEvent[], frameNo: number): void {
    if (!this.opened) return;
    if (frameNo % 30 === 0) this.benchOk = nearBench(sim);
    switch (this.tab) {
      case "inventory":
        this.patchInventory(sim);
        break;
      case "book":
        this.patchBook(sim);
        break;
      case "quests":
        if (frameNo % 6 === 0 || this.questListSig === "\u0000") this.patchQuests(sim);
        break;
      case "map":
        for (const ev of events) {
          if (ev.e === "zone") this.map.invalidate();
          else if (ev.e === "tiles") this.map.tilesChanged();
        }
        this.map.frame(sim);
        break;
    }
    this.patchCursor(sim);
    this.setHint(this.hintText());
  }

  // --- tabs --------------------------------------------------------------------------

  private pane(t: TabId): HTMLElement {
    return this.panes.get(t) as HTMLElement;
  }

  private setTab(tab: TabId): void {
    this.tab = tab;
    for (const t of TABS) {
      this.tabBtns.get(t)?.classList.toggle("active", t === tab);
      this.pane(t).hidden = t !== tab;
    }
    this.held = null;
    this.popover.close();
    this.ctx.tip.hide();
    this.cursorKey = "";
    this.bookSig = "\u0000";
    this.questListSig = "\u0000";
    this.questDetailSig = "\u0000";
    this.cursor = homeCursor(this.layout());
    if (tab === "map") this.map.invalidate();
  }

  private layout(): NavLayout {
    const sim = this.ctx.host.sim();
    const listLen = this.tab === "book" ? (sim?.player.book.length ?? 0) : this.tab === "quests" ? this.questIds.length : 0;
    return { tab: this.tab, craft: this.benchOk, listLen };
  }

  // --- drops: the one place a payload meets a target -----------------------------------

  private drop(p: DragPayload, t: DropTarget, x: number, y: number): void {
    const { host } = this.ctx;
    const sim = host.sim();
    if (!sim || !this.opened) return;
    switch (p.kind) {
      case "bag": {
        const stack = sim.player.bag?.[p.slot];
        if (!stack) return;
        if (t.kind === "bag") {
          if (t.slot !== p.slot) host.command({ t: "bagMove", from: p.slot, to: t.slot });
        } else if (t.kind === "craft") {
          if (this.benchOk) host.command({ t: "craftPut", bag: p.slot, slot: t.slot });
          else this.ctx.toast("Stand at a bench to craft");
        } else if (t.kind === "bar") {
          host.command({ t: "bind", slot: t.slot, source: "item", id: stack.item });
        } else if (t.kind === "outside") {
          this.askDestroy(sim, p.slot, x, y);
        }
        return;
      }
      case "spell":
        if (t.kind === "bar") host.command({ t: "bind", slot: t.slot, source: "spell", id: p.id });
        return;
      case "bar":
        if (t.kind === "bar") {
          if (t.slot !== p.slot) host.command({ t: "barSwap", a: p.slot, b: t.slot });
        } else if (t.kind !== "barbg") {
          host.command({ t: "unbind", slot: p.slot });
        }
        return;
    }
  }

  private askDestroy(sim: Sim, slot: number, x: number, y: number): void {
    const stack = sim.player.bag?.[slot];
    if (!stack) return;
    const def = sim.catalog.items[stack.item];
    if (def?.bound) {
      this.ctx.toast("I should keep that");
      return;
    }
    const what = stack.qty > 1 ? `${def?.name ?? stack.item} x${stack.qty}` : (def?.name ?? stack.item);
    this.popover.open(
      `Destroy ${what}?`,
      [
        { label: "Keep", pick: () => {} },
        { label: "Destroy", danger: true, pick: () => this.ctx.host.command({ t: "bagDestroy", slot }) },
      ],
      x,
      y,
      0,
    );
  }

  // --- pad / keyboard ------------------------------------------------------------------

  private confirm(): void {
    const { host } = this.ctx;
    const sim = host.sim();
    if (!sim) return;
    const c = this.cursor;
    if (this.held) {
      const held = this.held;
      const r = this.cellEl(c)?.getBoundingClientRect();
      this.dropHeld();
      // Putting a bar slot back where it came from is "never mind", not "unbind".
      if (held.kind === "bar" && c.region === "bar" && c.index === held.slot) return;
      const target = held.kind === "bar" && c.region !== "bar" ? ({ kind: "window" } as DropTarget) : cursorTarget(c);
      this.drop(held, target, r ? r.left + r.width / 2 : 0, r ? r.top : 0);
      return;
    }
    switch (c.region) {
      case "bag": {
        const stack = sim.player.bag?.[c.index];
        if (stack) this.bagMenu(sim, c.index);
        return;
      }
      case "craft":
        if (!this.benchOk) return;
        if (c.index < CRAFT_INPUTS) host.command({ t: "craftClear", slot: c.index });
        else host.command({ t: "craftTake" });
        return;
      case "bar":
        if (sim.me.bar[c.index]) this.held = { kind: "bar", slot: c.index };
        return;
      case "list":
        if (this.tab === "book") {
          const spell = sim.player.book[c.index];
          if (!spell) return;
          this.held = { kind: "spell", id: spell };
          const empty = firstEmpty(sim.me.bar);
          this.cursor = { region: "bar", index: empty >= 0 ? empty : 0 };
        }
        return;
      default:
        return;
    }
  }

  private dropHeld(): void {
    const held = this.held;
    this.held = null;
    if (held?.kind === "spell" && this.tab === "book") {
      const sim = this.ctx.host.sim();
      const i = sim ? sim.player.book.indexOf(held.id) : -1;
      this.cursor = { region: "list", index: Math.max(0, i) };
    }
  }

  private bagMenu(sim: Sim, slot: number): void {
    const stack = sim.player.bag?.[slot];
    if (!stack) return;
    const def = sim.catalog.items[stack.item];
    const r = this.bagSlots[slot].el.getBoundingClientRect();
    const x = r.left + r.width / 2;
    const items: PopItem[] = [];
    if (def?.usable) items.push({ label: "Use", pick: () => this.ctx.host.command({ t: "item", item: stack.item }) });
    items.push({
      label: "Move",
      pick: () => {
        // Pointer users drag instead; "held" only means something while the pad cursor is showing.
        if (this.keysMode) this.held = { kind: "bag", slot };
      },
    });
    if (!def?.bound) items.push({ label: "Destroy", danger: true, pick: () => this.askDestroy(sim, slot, x, r.top) });
    this.ctx.tip.hide();
    this.popover.open(def?.name ?? stack.item, items, r.right + r.width * 0.2, r.top, 0, true);
  }

  private cellEl(c: Cursor): HTMLElement | null {
    switch (c.region) {
      case "bag":
        return this.bagSlots[c.index]?.el ?? null;
      case "craft":
        return this.craftSlots[c.index]?.el ?? null;
      case "bar":
        return this.bar.slots[c.index]?.el ?? null;
      case "list":
        return (this.tab === "book" ? this.bookRows[c.index] : this.questRows[c.index]) ?? null;
      default:
        return null;
    }
  }

  private setCursorEl(el: HTMLElement | null): void {
    if (el === this.cursorEl) return;
    this.cursorEl?.classList.remove("jq-cursor");
    this.cursorEl = el;
    el?.classList.add("jq-cursor");
  }

  private heldIcon(sim: Sim): string {
    const held = this.held;
    if (!held) return "";
    if (held.kind === "spell") return sim.catalog.spells[held.id]?.icon ?? "";
    if (held.kind === "bar") return barIconId(sim, sim.me.bar[held.slot]);
    const stack = sim.player.bag?.[held.slot];
    return stack ? (sim.catalog.items[stack.item]?.icon ?? "") : "";
  }

  private patchCursor(sim: Sim): void {
    const c = this.cursor;
    const stack = c.region === "bag" ? sim.player.bag?.[c.index] : null;
    const bound = c.region === "bar" ? sim.me.bar[c.index] : null;
    const held = this.held;
    const heldSig = held ? `${held.kind}:${held.kind === "spell" ? held.id : held.slot}` : "";
    const key = this.keysMode
      ? `${this.tab}|${c.region}|${c.index}|${heldSig}|${stackSig(stack)}|${bound ? bound.id : ""}|${this.popover.isOpen ? 1 : 0}`
      : "";
    if (key === this.cursorKey) return;
    this.cursorKey = key;
    if (!this.keysMode) {
      this.setCursorEl(null);
      this.ctx.drag.showHeld("", null);
      return;
    }
    const el = this.cellEl(c);
    this.setCursorEl(el);
    if (el && c.region === "list") el.scrollIntoView({ block: "nearest" });
    this.ctx.drag.showHeld(this.heldIcon(sim), el);
    const { tip } = this.ctx;
    if (!el || this.popover.isOpen || this.held) tip.hide();
    else if (c.region === "bag" && stack) tip.item(stack.item, el);
    else if (c.region === "bar") tip.bar(sim.me.bar[c.index] ?? null, el);
    else if (c.region === "craft" && c.index === CRAFT_INPUTS) {
      const o = craftOutput(sim);
      if (o) tip.item(o.item, el);
      else tip.hide();
    } else tip.hide();
  }

  private hintText(): string {
    if (!this.keysMode) return POINTER_HINT[this.tab];
    if (this.tab === "map") return "Arrows: look round - Confirm: zoom - Tab keys: switch page - Cancel: close";
    if (this.popover.isOpen) return "Up / Down: choose - Confirm: select - Cancel: back";
    if (this.held) {
      if (this.held.kind === "bag") return "Confirm: put down on a bag slot, craft input or bar slot - Cancel: never mind";
      if (this.held.kind === "spell") return "Left / Right: pick a bar slot - Confirm: bind - Cancel: never mind";
      return "Confirm on another bar slot: swap - Confirm anywhere else: unbind - Cancel: never mind";
    }
    switch (this.cursor.region) {
      case "bag":
        return "Confirm: use, move or destroy - Tab keys: switch page - Cancel: close";
      case "craft":
        return this.cursor.index < CRAFT_INPUTS ? "Confirm: take the ingredient back" : "Confirm: take the result";
      case "bar":
        return "Confirm: pick up this bar slot - Up: back to the window";
      case "list":
        return this.tab === "book" ? "Confirm: bind to the bar - Down past the end: edit the bar" : "Up / Down: choose a quest";
      default:
        return "Tab keys: switch page - Cancel: close";
    }
  }

  // --- inventory -----------------------------------------------------------------------

  private wireBagSlot(v: SlotView, i: number): void {
    const { host, drag, tip } = this.ctx;
    const stackAt = (): { item: string; qty: number } | null => host.sim()?.player.bag?.[i] ?? null;
    const use = (): void => {
      const s = stackAt();
      if (!s) return;
      tip.hide();
      host.command({ t: "item", item: s.item });
    };
    drag.attach(v.el, {
      payload: () => (stackAt() ? { kind: "bag", slot: i } : null),
      iconId: () => {
        const s = stackAt();
        return s ? (host.catalog.items[s.item]?.icon ?? "") : "";
      },
      // A tap pins the tooltip: touch has no hover.
      click: () => {
        const s = stackAt();
        if (s) tip.item(s.item, v.el);
        else tip.hide();
      },
      double: use,
      context: use,
    });
    v.el.addEventListener("pointerenter", (ev) => {
      const s = stackAt();
      if (ev.pointerType === "mouse" && s && !drag.active() && !this.popover.isOpen) tip.item(s.item, v.el);
    });
    v.el.addEventListener("pointerleave", (ev) => {
      if (ev.pointerType === "mouse") tip.hide();
    });
  }

  private patchInventory(sim: Sim): void {
    const { icon } = this.ctx;
    const cat = sim.catalog;
    const p = sim.player;
    const bag = p.bag ?? [];
    for (let i = 0; i < this.bagSlots.length; i++) {
      const s = bag[i] ?? null;
      this.bagSlots[i].set(stackSig(s), s ? icon(cat.items[s.item]?.icon ?? "") : "", s && s.qty > 1 ? String(s.qty) : "");
    }
    for (let i = 0; i < CRAFT_INPUTS; i++) {
      const s = sim.me.craft[i] ?? null;
      this.craftSlots[i].set(stackSig(s), s ? icon(cat.items[s.item]?.icon ?? "") : "", "");
    }
    const out = this.benchOk ? craftOutput(sim) : null;
    this.craftSlots[CRAFT_INPUTS].set(
      out ? `${out.item}*${out.qty}` : "",
      out ? icon(cat.items[out.item]?.icon ?? "") : "",
      out && out.qty > 1 ? String(out.qty) : "",
    );
    this.craftStrip.classList.toggle("disabled", !this.benchOk);
    this.setCraftHint(this.benchOk ? (out ? (cat.items[out.item]?.name ?? "") : "Add ingredients") : "Stand at a bench to craft");

    const rows = [
      `${Math.max(0, Math.ceil(p.hp))}/${maxHp(p)}`,
      `${Math.floor(p.mp)}/${maxMp(p)}`,
      String(p.strength),
      String(p.spirit),
      String(sim.me.stats.kills),
      String(sim.me.stats.deaths),
    ];
    rows.forEach((text, i) => this.statRows[i]?.(text));
    this.setCardName(sim.state.name);
  }

  // --- spellbook -----------------------------------------------------------------------

  private patchBook(sim: Sim): void {
    const book = sim.player.book;
    const sig = book.join(",");
    if (sig === this.bookSig) return;
    this.bookSig = sig;
    this.cursorKey = "";
    this.setCursorEl(null);
    clear(this.bookList);
    this.bookRows = [];
    const { host, drag, icon } = this.ctx;
    if (book.length === 0) h("div", "jq-empty", this.bookList, "No spells yet.");
    for (const id of book) {
      const def = sim.catalog.spells[id];
      if (!def) continue;
      const row = h("div", "jq-spell", this.bookList);
      const ic = h("div", "jq-slot jq-spell-icon", row);
      h("div", "jq-icon", ic).style.backgroundImage = `url("${icon(def.icon)}")`;
      const text = h("div", "jq-spell-text", row);
      const top = h("div", "jq-spell-top", text);
      h("span", "jq-spell-name", top, def.name);
      const cd = def.cooldown > 0 ? ` - ${durationText(def.cooldown)} cooldown` : "";
      h("span", "jq-spell-meta", top, `${costText(def)} - ${rangeText(def)}${cd}`);
      h("div", "jq-spell-desc", text, def.description);
      drag.attach(row, {
        payload: () => ({ kind: "spell", id }),
        iconId: () => def.icon,
        click: () => {
          const s = host.sim();
          if (!s) return;
          const slot = firstEmpty(s.me.bar);
          if (slot < 0) {
            this.ctx.toast("The bar is full: drag the spell onto a slot");
            return;
          }
          host.command({ t: "bind", slot, source: "spell", id });
          this.ctx.toast(`${def.name} bound to ${slot + 1}`);
        },
      });
      this.bookRows.push(row);
    }
  }

  // --- quests --------------------------------------------------------------------------

  private patchQuests(sim: Sim): void {
    const cat = sim.catalog;
    const active = sim.state.quests.active.filter((q) => cat.quests[q.quest]);
    const done = sim.state.quests.done.filter((id) => cat.quests[id]);
    const ids = [...active.map((q) => q.quest), ...done];
    if (!ids.includes(this.questSel)) this.questSel = ids[0] ?? "";

    const listSig = `${active.map((q) => `${q.quest}${questReady(sim, q.quest) ? "!" : ""}`).join(",")}|${done.join(",")}|${this.questSel}`;
    if (listSig !== this.questListSig) {
      this.questListSig = listSig;
      this.questIds = ids;
      this.cursorKey = "";
      this.setCursorEl(null);
      clear(this.questList);
      this.questRows = [];
      if (ids.length === 0) h("div", "jq-empty", this.questList, "Nothing in the log.");
      if (active.length > 0) h("div", "jq-qhead", this.questList, "Active");
      ids.forEach((id, n) => {
        if (n === active.length) h("div", "jq-qhead", this.questList, "Completed");
        const isDone = n >= active.length;
        const ready = !isDone && questReady(sim, id);
        const row = h("div", `jq-qrow${isDone ? " done" : ""}${ready ? " ready" : ""}${id === this.questSel ? " sel" : ""}`, this.questList);
        h("span", "jq-qmark", row, isDone ? "x" : ready ? "!" : "·");
        h("span", "", row, cat.quests[id].name);
        row.addEventListener("click", () => {
          this.questSel = id;
          this.cursor = { region: "list", index: n };
          this.questListSig = "\u0000";
        });
        this.questRows.push(row);
      });
      if (this.cursor.region === "list") this.cursor = { region: "list", index: Math.max(0, ids.indexOf(this.questSel)) };
      else if (ids.length > 0 && this.tab === "quests") this.cursor = { region: "list", index: 0 };
    }

    const id = this.questSel;
    const def = id ? cat.quests[id] : undefined;
    const prog = active.find((q) => q.quest === id);
    const counts = def && prog ? def.requirements.map((_, i) => requirementCount(sim, def, prog, i)) : [];
    const detailSig = `${id}|${prog ? "a" : "d"}|${counts.join(",")}`;
    if (detailSig === this.questDetailSig) return;
    this.questDetailSig = detailSig;
    clear(this.questDetail);
    if (!def) return;
    h("div", "jq-qd-name", this.questDetail, def.name);
    h("div", "jq-qd-body", this.questDetail, expandText(sim.state, def.description));
    const reqs = h("div", "jq-qd-reqs", this.questDetail);
    def.requirements.forEach((r, i) => {
      const n = prog ? counts[i] : r.qty;
      h("div", `jq-qd-req${n >= r.qty ? " done" : ""}`, reqs, `${n >= r.qty ? "[x]" : "[ ]"} ${expandText(sim.state, r.text)} ${n}/${r.qty}`);
    });
    if (!prog) h("div", "jq-qd-done", this.questDetail, expandText(sim.state, def.completion));
    else if (counts.every((n, i) => n >= def.requirements[i].qty)) {
      const back = expandText(sim.state, (def as { returnTo?: string }).returnTo ?? "");
      h("div", "jq-qd-ready", this.questDetail, back ? `Ready to hand in: ${back}.` : "Ready to hand in.");
    }
  }
}
