// Dialogue box. The sim owns the conversation (and freezes the world while it
// runs); this only reveals the current line and sends advance / choose / close.

import { awaitingChoice, currentNode } from "@/sim/dialogue";
import type { PlayerView as Sim } from "@/sim/sim";
import { expandText } from "@/sim/text";
import type { Ctx } from "@/ui/ctx";
import { cached, classOf, clear, h, textOf, visOf } from "@/ui/dom";
import type { UiAction } from "@/ui/host";

/** Characters revealed per rendered frame: a 60-character line takes a third of a second. */
const REVEAL_PER_FRAME = 3;

export class DialogueBox {
  readonly el: HTMLDivElement;
  private readonly setVis: (on: boolean) => void;
  private readonly setSpeaker: (s: string) => void;
  private readonly setSpeakerVis: (on: boolean) => void;
  private readonly setShown: (s: string) => void;
  private readonly setRest: (s: string) => void;
  private readonly setMore: (s: string) => void;
  private readonly setChoosing: (on: boolean) => void;
  private readonly setOption: (i: number) => void;
  private readonly options: HTMLDivElement;
  private optionRows: HTMLElement[] = [];

  private sig = "";
  private full = "";
  private shown = 0;
  private option = 0;

  constructor(private readonly ctx: Ctx) {
    this.el = h("div", "jq-dialogue jq-panel");
    this.el.hidden = true;
    this.setVis = visOf(this.el);
    const speaker = h("div", "jq-speaker", this.el);
    this.setSpeaker = textOf(speaker);
    this.setSpeakerVis = visOf(speaker);
    const line = h("div", "jq-line", this.el);
    this.setShown = textOf(h("span", "", line));
    // The unrevealed tail is laid out but invisible, so words never jump lines mid-reveal.
    this.setRest = textOf(h("span", "jq-line-rest", line));
    this.options = h("div", "jq-options", this.el);
    this.setChoosing = classOf(this.el, "choosing");
    this.setMore = textOf(h("div", "jq-more", this.el));
    this.setOption = cached<number>((i) => {
      this.optionRows.forEach((r, n) => r.classList.toggle("focus", n === i));
    });
    this.el.addEventListener("click", () => this.confirm());
  }

  /** The sim went away or was replaced. */
  reset(): void {
    this.sig = "";
    this.setVis(false);
  }

  frame(sim: Sim): void {
    const d = sim.me.dialogue;
    this.setVis(d !== null);
    if (!d) {
      this.sig = "";
      return;
    }
    const node = currentNode(sim);
    const sig = `${d.tree}|${d.node}|${d.line}`;
    if (sig !== this.sig) {
      this.sig = sig;
      this.full = expandText(sim.state, node?.lines[d.line] ?? "");
      this.shown = 0;
      this.option = 0;
      const speaker = sim.catalog.dialogue[d.tree]?.speaker ?? "";
      this.setSpeaker(speaker);
      this.setSpeakerVis(speaker !== "");
      clear(this.options);
      this.optionRows = (node?.options ?? []).slice(0, 2).map((o, i) => {
        const row = h("div", "jq-mi jq-option", this.options, o.label);
        row.addEventListener("pointerenter", (ev) => {
          if (ev.pointerType === "mouse") this.option = i;
        });
        row.addEventListener("click", (ev) => {
          ev.stopPropagation();
          if (this.revealing()) this.shown = this.full.length;
          else this.ctx.host.command({ t: "choose", option: i });
        });
        return row;
      });
      this.setOption(-1);
    }
    if (this.revealing()) this.shown = Math.min(this.full.length, this.shown + REVEAL_PER_FRAME);
    this.setShown(this.full.slice(0, this.shown));
    this.setRest(this.full.slice(this.shown));

    const done = !this.revealing();
    const choosing = done && awaitingChoice(sim);
    this.setChoosing(choosing);
    if (choosing) this.setOption(Math.min(this.option, this.optionRows.length - 1));
    const last = !!node && d.line >= node.lines.length - 1 && !node.goto;
    // Say which button does what, for whichever device was touched last.
    const k = this.ctx.host.hints();
    this.setMore(
      !done
        ? `[${k.confirm}] skip`
        : choosing
          ? `[${k.choose}] choose   [${k.confirm}] answer`
          : last
            ? `[${k.confirm}] close ■`
            : `[${k.confirm}] next ▼   [${k.cancel}] leave`,
    );
  }

  /** Every action is swallowed while someone is talking: the world is frozen anyway. */
  action(a: UiAction, sim: Sim): boolean {
    const choosing = !this.revealing() && awaitingChoice(sim);
    if (a === "confirm") this.confirm();
    else if (a === "cancel" || a === "pause") {
      if (!awaitingChoice(sim)) this.ctx.host.command({ t: "closeDialogue" });
    } else if (choosing && (a === "up" || a === "left")) this.option = Math.max(0, this.option - 1);
    else if (choosing && (a === "down" || a === "right")) this.option = Math.min(this.optionRows.length - 1, this.option + 1);
    return true;
  }

  private revealing(): boolean {
    return this.shown < this.full.length;
  }

  private confirm(): void {
    const sim = this.ctx.host.sim();
    if (!sim || !sim.me.dialogue) return;
    if (this.revealing()) {
      this.shown = this.full.length;
      return;
    }
    if (awaitingChoice(sim)) this.ctx.host.command({ t: "choose", option: Math.min(this.option, Math.max(0, this.optionRows.length - 1)) });
    else this.ctx.host.command({ t: "advance" });
  }
}
