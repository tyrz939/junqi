// Drop-down console. The only text field in the UI: while it is open the input
// layer must leave the keyboard alone (UiApi.capturesKeys).

import type { Ctx } from "@/ui/ctx";
import { h } from "@/ui/dom";

const MAX_LINES = 400;
const MAX_HISTORY = 100;

export class Terminal {
  readonly el: HTMLDivElement;
  private readonly log: HTMLDivElement;
  private readonly input: HTMLInputElement;
  private opened = false;
  private history: string[] = [];
  private historyAt = 0;
  private draft = "";

  constructor(private readonly ctx: Ctx) {
    this.el = h("div", "jq-term jq-panel");
    this.el.hidden = true;
    const head = h("div", "jq-term-head", this.el);
    h("span", "", head, "console");
    const close = h("span", "jq-term-close", head, "x");
    close.addEventListener("click", () => this.close());
    this.log = h("div", "jq-term-log", this.el);
    const row = h("div", "jq-term-row", this.el);
    h("span", "jq-term-caret", row, ">");
    this.input = h("input", "jq-term-input", row);
    this.input.type = "text";
    this.input.spellcheck = false;
    this.input.autocomplete = "off";
    this.input.setAttribute("autocapitalize", "off");
    this.input.setAttribute("aria-label", "Console input");
    const run = h("span", "jq-btn jq-term-run", row, "Run");
    run.addEventListener("click", () => {
      this.submit();
      this.input.focus();
    });

    this.input.addEventListener("keydown", (ev) => {
      // Ours alone: the game's key handler must not also see Escape and open the pause menu.
      ev.stopPropagation();
      if (ev.key === "Enter") {
        ev.preventDefault();
        this.submit();
      } else if (ev.key === "Escape" || ev.code === "Backquote") {
        ev.preventDefault();
        if (!ev.repeat) this.close();
      } else if (ev.key === "ArrowUp") {
        ev.preventDefault();
        this.recall(-1);
      } else if (ev.key === "ArrowDown") {
        ev.preventDefault();
        this.recall(1);
      }
    });
    this.el.addEventListener("pointerdown", (ev) => {
      if (ev.target !== this.input && !this.log.contains(ev.target as Node)) ev.preventDefault();
    });
    this.el.addEventListener("click", (ev) => {
      if (!this.log.contains(ev.target as Node)) this.input.focus();
    });
    this.print("Jane console. Enter runs a line, Up / Down recall, Esc closes.", "dim");
  }

  get isOpen(): boolean {
    return this.opened;
  }

  /** True when the text field owns the keyboard. */
  get focused(): boolean {
    return this.opened || document.activeElement === this.input;
  }

  toggle(): void {
    if (this.opened) this.close();
    else this.open();
  }

  open(): void {
    if (this.opened) return;
    this.opened = true;
    this.el.hidden = false;
    this.log.scrollTop = this.log.scrollHeight;
    // Focus after the key event that opened us has finished, so its character is not typed in.
    setTimeout(() => {
      if (this.opened) this.input.focus();
    }, 0);
  }

  close(): void {
    if (!this.opened) return;
    this.opened = false;
    this.el.hidden = true;
    this.input.blur();
  }

  private submit(): void {
    const line = this.input.value.trim();
    this.input.value = "";
    this.draft = "";
    if (line === "") return;
    if (this.history[this.history.length - 1] !== line) this.history.push(line);
    if (this.history.length > MAX_HISTORY) this.history.shift();
    this.historyAt = this.history.length;
    this.print(`> ${line}`, "echo");
    let out: string[];
    try {
      out = this.ctx.host.terminal(line);
    } catch (err) {
      out = [`error: ${err instanceof Error ? err.message : String(err)}`];
    }
    for (const l of out) this.print(l, "");
  }

  private recall(dir: 1 | -1): void {
    if (this.history.length === 0) return;
    if (this.historyAt === this.history.length) this.draft = this.input.value;
    this.historyAt = Math.max(0, Math.min(this.history.length, this.historyAt + dir));
    this.input.value = this.historyAt === this.history.length ? this.draft : this.history[this.historyAt];
    const end = this.input.value.length;
    this.input.setSelectionRange(end, end);
  }

  private print(text: string, cls: string): void {
    h("div", `jq-term-line ${cls}`.trim(), this.log, text === "" ? " " : text);
    while (this.log.childElementCount > MAX_LINES) this.log.firstElementChild?.remove();
    this.log.scrollTop = this.log.scrollHeight;
  }
}
