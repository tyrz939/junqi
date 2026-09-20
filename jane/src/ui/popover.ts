// A small modal choice box: the bag's pad context menu ("Use / Move / Destroy")
// and the "Destroy X?" confirmation. Pointer: click an entry, or click away to
// dismiss. Pad: directions move, confirm picks, cancel dismisses.

import { clear, h } from "@/ui/dom";
import type { UiAction } from "@/ui/host";

export type PopItem = { label: string; danger?: boolean; pick(): void };

export class Popover {
  readonly el: HTMLDivElement;
  private readonly box: HTMLDivElement;
  private items: PopItem[] = [];
  private rows: HTMLElement[] = [];
  private index = 0;
  private opened = false;

  constructor(private readonly root: HTMLElement) {
    this.el = h("div", "jq-pop-layer", root);
    this.el.dataset.drop = "window";
    this.el.hidden = true;
    this.box = h("div", "jq-pop jq-panel", this.el);
    this.el.addEventListener("pointerdown", (ev) => {
      if (ev.target === this.el) this.close();
    });
  }

  get isOpen(): boolean {
    return this.opened;
  }

  /**
   * `x`,`y` are client coordinates. Default: centred above that point (a drop position).
   * `beside`: the point is the box's top-left corner (next to a slot).
   */
  open(title: string, items: PopItem[], x: number, y: number, focus = 0, beside = false): void {
    this.items = items;
    this.index = Math.max(0, Math.min(items.length - 1, focus));
    this.opened = true;
    clear(this.box);
    if (title) h("div", "jq-pop-title", this.box, title);
    this.rows = items.map((it, i) => {
      const row = h("div", `jq-mi${it.danger ? " danger" : ""}`, this.box, it.label);
      row.addEventListener("pointerenter", (ev) => {
        if (ev.pointerType === "mouse") this.focus(i);
      });
      row.addEventListener("click", () => this.pick(i));
      return row;
    });
    this.el.hidden = false;
    this.focus(this.index);
    const root = this.root.getBoundingClientRect();
    const w = this.box.offsetWidth;
    const hh = this.box.offsetHeight;
    const px = Math.max(root.left + 4, Math.min(root.right - w - 4, beside ? x : x - w / 2));
    const py = Math.max(root.top + 4, Math.min(root.bottom - hh - 4, beside ? y : y - hh - 8));
    this.box.style.transform = `translate(${Math.round(px - root.left)}px, ${Math.round(py - root.top)}px)`;
  }

  close(): void {
    if (!this.opened) return;
    this.opened = false;
    this.el.hidden = true;
  }

  action(a: UiAction): boolean {
    if (!this.opened) return false;
    const n = this.items.length;
    if (a === "up" || a === "left") this.focus((this.index + n - 1) % n);
    else if (a === "down" || a === "right") this.focus((this.index + 1) % n);
    else if (a === "confirm") this.pick(this.index);
    else if (a === "cancel" || a === "pause") this.close();
    return true;
  }

  private focus(i: number): void {
    this.index = i;
    this.rows.forEach((r, n) => r.classList.toggle("focus", n === i));
  }

  private pick(i: number): void {
    const it = this.items[i];
    this.close();
    it?.pick();
  }
}
