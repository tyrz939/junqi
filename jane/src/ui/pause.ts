// Pause menu. Blocks the world while open.

import type { Ctx } from "@/ui/ctx";
import { h } from "@/ui/dom";
import type { UiAction } from "@/ui/host";
import { controlsPage, loadPage, MenuStack, savePage, type MenuPage } from "@/ui/menu";

export class PauseMenu {
  readonly el: HTMLDivElement;
  private readonly menu: MenuStack;
  private opened = false;

  constructor(private readonly ctx: Ctx) {
    this.el = h("div", "jq-pause");
    this.el.hidden = true;
    const box = h("div", "jq-pause-box jq-panel", this.el);
    this.menu = new MenuStack(() => this.close());
    box.appendChild(this.menu.el);
  }

  get isOpen(): boolean {
    return this.opened;
  }

  open(): void {
    if (this.opened) return;
    this.opened = true;
    this.el.hidden = false;
    this.menu.reset(() => this.mainPage());
  }

  close(): void {
    if (!this.opened) return;
    this.opened = false;
    this.el.hidden = true;
  }

  action(a: UiAction): boolean {
    this.menu.action(a);
    return true;
  }

  private mainPage(): MenuPage {
    const { ctx, menu } = this;
    const anySave = ctx.host.slots().some((s) => s !== null);
    return {
      title: "Paused",
      items: [
        { label: "Resume", pick: () => this.close() },
        { label: "Save", pick: () => menu.push(savePage(ctx, menu)) },
        { label: "Load", disabled: !anySave, pick: () => menu.push(loadPage(ctx, menu, () => this.close())) },
        { label: "Controls", pick: () => menu.push(controlsPage(ctx, menu)) },
        { label: "Quit to Title", pick: () => menu.push(() => this.quitPage()) },
      ],
    };
  }

  private quitPage(): MenuPage {
    return {
      title: "Quit to Title",
      note: "Anything since your last save is lost.",
      items: [
        { label: "Stay", pick: () => this.menu.pop() },
        {
          label: "Quit",
          danger: true,
          pick: () => {
            this.close();
            this.ctx.host.toTitle();
          },
        },
      ],
    };
  }
}
