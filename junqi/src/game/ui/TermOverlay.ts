import Phaser from "phaser";
import { session } from "@/game/systems/session";
import {
  bootTerminal,
  completeLine,
  runLine,
  setTermClose,
  termCwd,
  termHistory,
} from "@/game/systems/terminal";
import { lineToHtml, onTermChange, println, tClear, termLines } from "@/game/systems/termLog";

export function mountConsole(game: Phaser.Game): void {
  const root = document.getElementById("console");
  const log = document.getElementById("console-log");
  const row = document.getElementById("console-row");
  const prompt = document.getElementById("console-prompt");
  const input = document.getElementById("term");
  if (!(root instanceof HTMLElement) || !(log instanceof HTMLElement)) throw new Error("console");
  if (!(row instanceof HTMLElement)) throw new Error("console-row");
  if (!(prompt instanceof HTMLElement) || !(input instanceof HTMLInputElement)) throw new Error("term");

  let histAt = -1;
  let draft = "";

  const paintLog = (): void => {
    log.innerHTML = termLines().map((line) => `<div class="term-line">${lineToHtml(line)}</div>`).join("");
    log.scrollTop = log.scrollHeight;
    prompt.textContent = termCwd();
  };

  const setGameKeys = (on: boolean): void => {
    const kb = game.input.keyboard;
    if (!kb) return;
    kb.enabled = on;
    kb.preventDefault = on;
  };

  const focusLine = (): void => {
    input.focus();
  };

  const setOpen = (open: boolean): void => {
    session.terminalOpen = open;
    root.classList.toggle("on", open);
    root.setAttribute("aria-hidden", open ? "false" : "true");
    setGameKeys(!open);
    if (open) {
      bootTerminal();
      paintLog();
      requestAnimationFrame(focusLine);
    } else {
      input.blur();
      histAt = -1;
    }
  };

  setTermClose(() => setOpen(false));
  onTermChange(() => {
    if (session.terminalOpen) paintLog();
  });

  const onCanvasDown = (event: MouseEvent): void => {
    if (!session.terminalOpen) return;
    event.preventDefault();
    focusLine();
  };

  const bindCanvas = (): void => {
    game.canvas?.addEventListener("mousedown", onCanvasDown);
  };
  if (game.isBooted) bindCanvas();
  else game.events.once(Phaser.Core.Events.READY, bindCanvas);

  window.addEventListener(
    "keydown",
    (event) => {
      const key = event.key;
      if (key === "`" || key === "~") {
        event.preventDefault();
        event.stopImmediatePropagation();
        setOpen(!session.terminalOpen);
        return;
      }
      if (!session.terminalOpen) return;

      if (key === "Escape") {
        event.preventDefault();
        event.stopImmediatePropagation();
        setOpen(false);
        return;
      }
      if (key === "Tab") {
        event.preventDefault();
        event.stopImmediatePropagation();
        const { insert, options } = completeLine(input.value);
        input.value = insert;
        if (options.length === 1 && !insert.endsWith("\\") && !insert.endsWith("/")) {
          input.value = `${insert} `;
        } else if (options.length > 1) {
          println(options.join("  "));
          input.value = insert;
        }
        return;
      }
      if (key === "ArrowUp") {
        event.preventDefault();
        const hist = termHistory();
        if (!hist.length) return;
        if (histAt < 0) draft = input.value;
        histAt = histAt < 0 ? hist.length - 1 : Math.max(0, histAt - 1);
        input.value = hist[histAt] ?? "";
        input.setSelectionRange(input.value.length, input.value.length);
        return;
      }
      if (key === "ArrowDown") {
        event.preventDefault();
        const hist = termHistory();
        if (histAt < 0) return;
        histAt += 1;
        if (histAt >= hist.length) {
          histAt = -1;
          input.value = draft;
        } else {
          input.value = hist[histAt] ?? "";
        }
        input.setSelectionRange(input.value.length, input.value.length);
        return;
      }
      if (key === "PageUp") {
        event.preventDefault();
        log.scrollTop -= log.clientHeight * 0.8;
        return;
      }
      if (key === "PageDown") {
        event.preventDefault();
        log.scrollTop += log.clientHeight * 0.8;
      }
    },
    true,
  );

  input.addEventListener("keydown", (event) => {
    if (!session.terminalOpen) return;
    if (event.key === "Enter") {
      event.preventDefault();
      const value = input.value;
      input.value = "";
      histAt = -1;
      draft = "";
      runLine(value);
      return;
    }
    if (!event.ctrlKey || event.altKey || event.metaKey) return;
    const key = event.key.toLowerCase();
    if (key === "l") {
      event.preventDefault();
      tClear();
      return;
    }
    if (key === "a") {
      event.preventDefault();
      input.setSelectionRange(0, 0);
      return;
    }
    if (key === "e") {
      event.preventDefault();
      const n = input.value.length;
      input.setSelectionRange(n, n);
      return;
    }
    if (key === "u") {
      event.preventDefault();
      const caret = input.selectionStart ?? 0;
      input.value = input.value.slice(caret);
      input.setSelectionRange(0, 0);
      return;
    }
    if (key === "k") {
      event.preventDefault();
      const caret = input.selectionStart ?? 0;
      input.value = input.value.slice(0, caret);
      input.setSelectionRange(caret, caret);
      return;
    }
    if (key === "w") {
      event.preventDefault();
      killWord(input);
    }
  });

  row.addEventListener("mousedown", () => {
    if (session.terminalOpen) focusLine();
  });

  input.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    void navigator.clipboard.readText().then((text) => {
      const start = input.selectionStart ?? input.value.length;
      const end = input.selectionEnd ?? start;
      input.value = input.value.slice(0, start) + text + input.value.slice(end);
      const at = start + text.length;
      input.setSelectionRange(at, at);
    }).catch(() => {});
  });
}

function killWord(input: HTMLInputElement): void {
  const start = input.selectionStart ?? 0;
  const end = input.selectionEnd ?? start;
  if (start !== end) {
    input.value = input.value.slice(0, start) + input.value.slice(end);
    input.setSelectionRange(start, start);
    return;
  }
  const before = input.value.slice(0, start);
  const after = input.value.slice(start);
  const next = before.replace(/\s*\S+$/, "");
  input.value = next + after;
  input.setSelectionRange(next.length, next.length);
}
