// The DOM UI root. Builds every layer once, then `frame` patches them.
//
// Action routing, top layer first. Whoever is open gets the action and eats it:
//
//   debug, terminal      always toggle
//   terminal open        cancel closes it; everything else is swallowed
//   title (no sim)       the title menu
//   pause menu open      pause closes; the rest drives the menu
//   window open          pause closes; the rest drives tabs / cursor / popover
//   dialogue open        confirm / cancel / up / down
//   otherwise            bags|book|quests|map open a tab, pause opens the menu,
//                        and confirm / cancel / directions return false (the game's)

import "./ui.css";

import type { SimEvent } from "@/sim/events";
import type { PlayerView as Sim } from "@/sim/sim";
import type { Ctx } from "@/ui/ctx";
import { DialogueBox } from "@/ui/dialogue";
import { h, textOf, visOf } from "@/ui/dom";
import { DragManager } from "@/ui/drag";
import { lootText, uiScale } from "@/ui/format";
import type { UiAction, UiApi, UiHost } from "@/ui/host";
import { ActionBar, Hud, Toasts } from "@/ui/hud";
import { PauseMenu } from "@/ui/pause";
import { Terminal } from "@/ui/terminal";
import { TitleScreen } from "@/ui/title";
import { Tooltip } from "@/ui/tooltip";
import { ACTION_TAB, GameWindow } from "@/ui/windows";

export function createUi(root: HTMLElement, host: UiHost): UiApi {
  root.classList.add("jq-ui");

  const rescale = (): void => {
    root.style.setProperty("--ui", String(uiScale(window.innerWidth, window.innerHeight)));
  };
  rescale();
  window.addEventListener("resize", rescale);

  const icons = new Map<string, string>();
  const toasts = new Toasts();
  const drag = new DragManager(root, (id) => ctx.icon(id));
  const tip = new Tooltip(root, host.catalog);

  const ctx: Ctx = {
    host,
    root,
    drag,
    tip,
    icon(id: string): string {
      if (!id) return "";
      let url = icons.get(id);
      if (url === undefined) {
        try {
          url = host.iconUrl(id) ?? "";
        } catch {
          url = "";
        }
        icons.set(id, url);
      }
      return url;
    },
    toast: (text) => toasts.push(text),
    toastCount: () => toasts.count,
    act: (a) => {
      const used = route(a);
      win.usingKeys(false);
      return used;
    },
  };

  const hud = new Hud(ctx);
  // The bar lives outside the window (it is HUD), but is only draggable while the window is open.
  const bar: ActionBar = new ActionBar(ctx, (): boolean => win.isOpen);
  const win: GameWindow = new GameWindow(ctx, bar);
  const dialogue = new DialogueBox(ctx);
  const pause = new PauseMenu(ctx);
  const title = new TitleScreen(ctx);
  const terminal = new Terminal(ctx);
  const debug = h("pre", "jq-debug");
  debug.hidden = true;
  const setDebug = textOf(debug);

  // Paint order is CSS z-index; DOM order only breaks ties.
  root.prepend(hud.el, win.el, bar.el, dialogue.el, pause.el, title.el, debug, terminal.el, toasts.el);

  const setGameVis = [visOf(hud.el), visOf(bar.el)];
  let lastSim: Sim | null | undefined;
  let frameNo = 0;
  let closedOn = -1;
  let debugOn = false;

  function onSimChanged(sim: Sim | null): void {
    win.close(false);
    pause.close();
    dialogue.reset();
    hud.reset();
    tip.hide();
    for (const set of setGameVis) set(sim !== null);
    if (sim) title.hide();
    else title.show();
  }

  function route(a: UiAction): boolean {
    if (a === "debug") {
      debugOn = !debugOn;
      debug.hidden = !debugOn;
      if (debugOn) setDebug(host.debugText());
      return true;
    }
    if (a === "terminal") {
      terminal.toggle();
      return true;
    }
    if (terminal.isOpen) {
      if (a === "cancel" || a === "pause") terminal.close();
      return true;
    }
    const sim = host.sim();
    if (!sim) return title.action(a);
    if (pause.isOpen) {
      if (a === "pause") pause.close();
      else pause.action(a);
      return true;
    }
    if (win.isOpen) {
      if (a === "pause" && !win.popover.isOpen) win.close();
      else win.action(a);
      return true;
    }
    if (sim.me.dialogue) return dialogue.action(a, sim);
    const tab = ACTION_TAB[a];
    if (tab) {
      win.show(tab);
      return true;
    }
    if (a === "pause") {
      // A key mapped to both "cancel" and "pause" must not close one thing and open another.
      if (closedOn !== frameNo) pause.open();
      return true;
    }
    return false;
  }

  return {
    frame(events: SimEvent[]): void {
      frameNo++;
      const now = performance.now();
      const sim = host.sim();
      if (sim !== lastSim) {
        lastSim = sim;
        onSimChanged(sim);
      }
      toasts.frame(now);
      if (debugOn && frameNo % 10 === 0) setDebug(host.debugText());
      if (!sim) return;

      for (const ev of events) {
        if (ev.e === "toast") toasts.push(ev.text);
        else if (ev.e === "loot") toasts.push(lootText(host.catalog.items[ev.item]?.name ?? ev.item, ev.qty));
        else if (ev.e === "learn") toasts.push(`Learned ${host.catalog.spells[ev.spell]?.name ?? ev.spell}`);
      }
      // Using an item can start a conversation (Julie's letter): the window gives way to it.
      if (sim.me.dialogue && win.isOpen) win.close();
      if (sim.me.dialogue && pause.isOpen) pause.close();

      const blocked = win.isOpen || pause.isOpen || terminal.isOpen;
      hud.frame(sim, events, now, frameNo, blocked);
      bar.frame(sim, events, now);
      win.frame(sim, events, frameNo);
      dialogue.frame(sim);
    },

    action(a: UiAction): boolean {
      const before = (win.isOpen ? 1 : 0) + (pause.isOpen ? 2 : 0) + (terminal.isOpen ? 4 : 0) + (host.sim()?.me.dialogue ? 8 : 0);
      win.usingKeys(true);
      const used = route(a);
      const after = (win.isOpen ? 1 : 0) + (pause.isOpen ? 2 : 0) + (terminal.isOpen ? 4 : 0) + (host.sim()?.me.dialogue ? 8 : 0);
      if ((before & ~after) !== 0) closedOn = frameNo;
      return used;
    },

    blocksWorld(): boolean {
      return host.sim() === null || win.isOpen || pause.isOpen || terminal.isOpen;
    },

    capturesKeys(): boolean {
      if (terminal.focused) return true;
      const active = document.activeElement;
      return active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement;
    },

    toast(text: string): void {
      toasts.push(text);
    },
  };
}
