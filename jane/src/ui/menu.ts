// A stack of vertical menus, used by the title screen and the pause menu.
// Pages are factories, so going back (or refreshing after a save) re-reads the
// host. DOM is rebuilt on navigation only, never per frame.

import type { Ctx } from "@/ui/ctx";
import { clear, h } from "@/ui/dom";
import { slotSummary, stepIndex } from "@/ui/format";
import type { UiAction } from "@/ui/host";

export type MenuItem = { label: string; detail?: string; disabled?: boolean; danger?: boolean; pick(): void };
export type MenuPage = {
  title?: string;
  note?: string;
  /** Extra block above the items (the controls table). */
  extra?: HTMLElement;
  /** With `extra`: up / down scroll this element instead of moving focus. */
  scroll?: HTMLElement;
  items: MenuItem[];
};
export type PageFactory = () => MenuPage;

export class MenuStack {
  readonly el: HTMLDivElement;
  private pages: PageFactory[] = [];
  private focusAt: number[] = [];
  private page: MenuPage = { items: [] };
  private rows: HTMLElement[] = [];

  /** `onExit` runs when cancel is pressed on the root page. */
  constructor(private readonly onExit: () => void) {
    this.el = h("div", "jq-menu");
  }

  reset(root: PageFactory): void {
    this.pages = [root];
    this.focusAt = [0];
    this.render();
  }

  push(p: PageFactory): void {
    this.pages.push(p);
    this.focusAt.push(0);
    this.render();
  }

  pop(): void {
    if (this.pages.length <= 1) return;
    this.pages.pop();
    this.focusAt.pop();
    this.render();
  }

  refresh(): void {
    this.render();
  }

  action(a: UiAction): boolean {
    const items = this.page.items;
    const top = this.focusAt.length - 1;
    const disabled = items.map((i) => !!i.disabled);
    switch (a) {
      case "up":
      case "down": {
        const dir = a === "up" ? -1 : 1;
        if (this.page.scroll && items.length <= 1) {
          this.page.scroll.scrollTop += dir * Math.max(24, this.page.scroll.clientHeight * 0.4);
          return true;
        }
        this.setFocus(stepIndex(disabled, this.focusAt[top], dir));
        return true;
      }
      case "confirm": {
        const it = items[this.focusAt[top]];
        if (it && !it.disabled) it.pick();
        return true;
      }
      case "cancel":
        if (this.pages.length > 1) this.pop();
        else this.onExit();
        return true;
      default:
        return true;
    }
  }

  private setFocus(i: number): void {
    this.focusAt[this.focusAt.length - 1] = i;
    this.rows.forEach((r, n) => r.classList.toggle("focus", n === i));
  }

  private render(): void {
    const top = this.pages.length - 1;
    this.page = this.pages[top]();
    const page = this.page;
    clear(this.el);
    if (page.title) h("div", "jq-menu-title", this.el, page.title);
    if (page.note) h("div", "jq-menu-note", this.el, page.note);
    if (page.extra) this.el.appendChild(page.extra);
    const list = h("div", "jq-menu-list", this.el);
    this.rows = page.items.map((it, i) => {
      const row = h("div", `jq-mi${it.disabled ? " disabled" : ""}${it.danger ? " danger" : ""}`, list);
      h("span", "jq-mi-label", row, it.label);
      if (it.detail) h("span", "jq-mi-detail", row, it.detail);
      row.addEventListener("pointerenter", (ev) => {
        if (ev.pointerType === "mouse" && !it.disabled) this.setFocus(i);
      });
      row.addEventListener("click", () => {
        if (it.disabled) return;
        this.setFocus(i);
        it.pick();
      });
      return row;
    });
    let f = Math.min(this.focusAt[top], Math.max(0, page.items.length - 1));
    if (page.items[f]?.disabled) f = stepIndex(page.items.map((i) => !!i.disabled), f, 1);
    this.setFocus(f);
  }
}

// --- pages shared by title and pause ---------------------------------------------

export function controlsPage(ctx: Ctx, menu: MenuStack): PageFactory {
  return () => {
    const wrap = h("div", "jq-controls");
    const table = h("table", "jq-controls-table", wrap);
    const head = h("tr", "", h("thead", "", table));
    h("th", "", head, "Action");
    h("th", "", head, "Keys");
    h("th", "", head, "Pad");
    const body = h("tbody", "", table);
    for (const row of ctx.host.bindings()) {
      const tr = h("tr", "", body);
      h("td", "", tr, row.action);
      h("td", "jq-gold", tr, row.keys);
      h("td", "", tr, row.pad);
    }
    return { title: "Controls", extra: wrap, scroll: wrap, items: [{ label: "Back", pick: () => menu.pop() }] };
  };
}

export function loadPage(ctx: Ctx, menu: MenuStack, onLoaded: () => void): PageFactory {
  return () => ({
    title: "Load Game",
    items: [
      ...ctx.host.slots().map((info, i) => ({
        label: `Slot ${i + 1}`,
        detail: slotSummary(info),
        disabled: !info,
        pick: () => {
          const before = ctx.toastCount();
          // Reading a slot is asynchronous. The host holds the world still until it answers.
          void ctx.host.load(i).then((ok) => {
            if (ok) onLoaded();
            else if (ctx.toastCount() === before) ctx.toast(`Could not load slot ${i + 1}`);
          });
        },
      })),
      { label: "Back", pick: () => menu.pop() },
    ],
  });
}

export function savePage(ctx: Ctx, menu: MenuStack): PageFactory {
  return () => ({
    title: "Save Game",
    items: [
      ...ctx.host.slots().map((info, i) => ({
        label: `Slot ${i + 1}`,
        detail: slotSummary(info),
        pick: () => {
          const before = ctx.toastCount();
          // Resolves when the write has committed, so "Saved" is true and the refreshed row shows it.
          void ctx.host.save(i).then((ok) => {
            // The host may already have said so; identical live toasts merge, so this never doubles up.
            if (ok) ctx.toast(`Saved to slot ${i + 1}`);
            else if (ctx.toastCount() === before) ctx.toast("Could not save");
            menu.refresh();
          });
        },
      })),
      { label: "Back", pick: () => menu.pop() },
    ],
  });
}
