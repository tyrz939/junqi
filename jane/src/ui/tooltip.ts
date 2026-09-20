// One floating tooltip for the whole UI. Content is rebuilt only when the thing
// or its anchor changes, never per frame.

import type { Catalog } from "@/sim/catalog";
import type { BarSlot } from "@/sim/state";
import { clear, h } from "@/ui/dom";
import { costText, durationText, rangeText } from "@/ui/format";

export class Tooltip {
  readonly el: HTMLDivElement;
  private key = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly catalog: Catalog,
  ) {
    this.el = h("div", "jq-tip jq-panel", root);
    this.el.hidden = true;
  }

  item(id: string, anchor: HTMLElement): void {
    const def = this.catalog.items[id];
    if (!def) return this.hide();
    if (!this.begin(`item:${id}`, anchor)) return;
    h("div", "jq-tip-name", this.el, def.name);
    const tags: string[] = [];
    if (def.usable) tags.push("Usable");
    if (def.bound) tags.push("Bound");
    if (def.maxStack > 1) tags.push(`Stacks to ${def.maxStack}`);
    if (tags.length > 0) h("div", "jq-tip-tags", this.el, tags.join(" - "));
    if (def.description) h("div", "jq-tip-body", this.el, def.description);
    if (def.opens) h("div", "jq-tip-line jq-gold", this.el, `Opens: ${def.opens.replace(/[_-]+/g, " ")}`);
    if (def.usable && def.cooldown > 0) h("div", "jq-tip-line", this.el, `Cooldown: ${durationText(def.cooldown)}`);
    this.place(anchor);
  }

  spell(id: string, anchor: HTMLElement): void {
    const def = this.catalog.spells[id];
    if (!def) return this.hide();
    if (!this.begin(`spell:${id}`, anchor)) return;
    h("div", "jq-tip-name", this.el, def.name);
    h("div", "jq-tip-tags", this.el, `${def.school} - ${costText(def)} - ${rangeText(def)}`);
    if (def.description) h("div", "jq-tip-body", this.el, def.description);
    if (def.cooldown > 0) h("div", "jq-tip-line", this.el, `Cooldown: ${durationText(def.cooldown)}`);
    this.place(anchor);
  }

  bar(slot: BarSlot, anchor: HTMLElement): void {
    if (!slot) return this.hide();
    if (slot.source === "spell") this.spell(slot.id, anchor);
    else this.item(slot.id, anchor);
  }

  hide(): void {
    if (this.key === "") return;
    this.key = "";
    this.el.hidden = true;
  }

  /** False when this exact tooltip is already up. */
  private begin(what: string, anchor: HTMLElement): boolean {
    const r = anchor.getBoundingClientRect();
    const key = `${what}@${Math.round(r.left)},${Math.round(r.top)}`;
    if (key === this.key) return false;
    this.key = key;
    clear(this.el);
    this.el.hidden = false;
    return true;
  }

  private place(anchor: HTMLElement): void {
    const a = anchor.getBoundingClientRect();
    const root = this.root.getBoundingClientRect();
    const gap = Math.max(4, a.width * 0.2);
    const w = this.el.offsetWidth;
    const hh = this.el.offsetHeight;
    let x = a.right + gap;
    if (x + w > root.right - gap) x = a.left - gap - w;
    if (x < root.left + gap) x = Math.max(root.left + gap, Math.min(a.left, root.right - gap - w));
    let y = a.top;
    if (y + hh > root.bottom - gap) y = root.bottom - gap - hh;
    // A tooltip that had to fall back over its own anchor goes above it instead.
    if (x < a.right && x + w > a.left && y < a.bottom && y + hh > a.top) y = a.top - gap - hh;
    if (y < root.top + gap) y = root.top + gap;
    this.el.style.transform = `translate(${Math.round(x - root.left)}px, ${Math.round(y - root.top)}px)`;
  }
}
