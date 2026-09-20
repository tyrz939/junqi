// Title screen: shown whenever the host has no sim.

import type { Ctx } from "@/ui/ctx";
import { h } from "@/ui/dom";
import { latestSlot, slotSummary } from "@/ui/format";
import type { UiAction } from "@/ui/host";
import { controlsPage, loadPage, MenuStack, type MenuPage } from "@/ui/menu";

export class TitleScreen {
  readonly el: HTMLDivElement;
  private readonly menu: MenuStack;

  constructor(private readonly ctx: Ctx) {
    this.el = h("div", "jq-title");
    this.el.hidden = true;

    const sky = h("div", "jq-title-sky", this.el);
    h("div", "jq-title-stars", sky);
    h("div", "jq-title-moon", sky);
    const castle = h("div", "jq-title-castle", sky);
    h("div", "jq-title-window", castle);

    const col = h("div", "jq-title-col", this.el);
    // Working title; the real name comes later. The version string carries it too.
    h("div", "jq-wordmark", col, "PROJECT JANE");
    const tag = h("div", "jq-tagline", col);
    h("span", "jq-tagline-rule", tag);
    h("span", "", tag, "Castle, Sunday.");
    h("span", "jq-tagline-rule", tag);

    this.menu = new MenuStack(() => {});
    this.menu.el.classList.add("jq-panel", "jq-title-menu");
    col.appendChild(this.menu.el);

    h("div", "jq-version", this.el, ctx.host.version);

    // The name step: who got off the train. Blank means Jane.
    this.namePanel = h("div", "jq-panel jq-name", col);
    this.namePanel.hidden = true;
    h("div", "jq-name-q", this.namePanel, "The ticket is made out to:");
    this.nameInput = document.createElement("input");
    this.nameInput.className = "jq-name-input";
    this.nameInput.maxLength = 16;
    this.nameInput.value = "Jane";
    this.nameInput.spellcheck = false;
    this.nameInput.autocomplete = "off";
    this.namePanel.appendChild(this.nameInput);
    h("div", "jq-name-hint", this.namePanel, "[Enter] board the train   [Esc] back");
    this.nameInput.addEventListener("keydown", (ev) => {
      ev.stopPropagation();
      if (ev.key === "Enter") this.begin();
      else if (ev.key === "Escape") this.askName(false);
    });
  }

  private readonly namePanel: HTMLDivElement;
  private readonly nameInput: HTMLInputElement;

  private askName(on: boolean): void {
    this.namePanel.hidden = !on;
    this.menu.el.hidden = on;
    if (on) {
      this.nameInput.focus();
      this.nameInput.select();
    } else this.nameInput.blur();
  }

  private begin(): void {
    const name = this.nameInput.value;
    this.askName(false);
    this.ctx.host.newGame(undefined, name);
  }

  show(): void {
    this.el.hidden = false;
    this.askName(false);
    this.menu.reset(() => this.mainPage());
  }

  hide(): void {
    this.el.hidden = true;
  }

  action(a: UiAction): boolean {
    // A pad cannot type: confirm takes the name as it stands, cancel goes back.
    if (!this.namePanel.hidden) {
      if (a === "confirm") this.begin();
      else if (a === "cancel" || a === "pause") this.askName(false);
      return true;
    }
    this.menu.action(a);
    return true;
  }

  private mainPage(): MenuPage {
    const { host } = this.ctx;
    const slots = host.slots();
    const latest = latestSlot(slots);
    return {
      items: [
        { label: "New Game", pick: () => this.askName(true) },
        {
          label: "Continue",
          detail: latest >= 0 ? slotSummary(slots[latest]) : "No saves yet",
          disabled: latest < 0,
          pick: () => {
            const before = this.ctx.toastCount();
            if (!host.load(latest) && this.ctx.toastCount() === before) this.ctx.toast("That save could not be loaded");
          },
        },
        { label: "Load", disabled: latest < 0, pick: () => this.menu.push(loadPage(this.ctx, this.menu, () => {})) },
        { label: "Controls", pick: () => this.menu.push(controlsPage(this.ctx, this.menu)) },
      ],
    };
  }
}
