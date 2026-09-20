// One action layer. 2020's `input_check(interaction.USE)` walked keyboard then
// gamepads and let the game ask about actions, never devices; its GUI code then
// undid that by triplicating every window for KBM / pad / touch. Here devices
// are sources that fill the same two outputs and nothing downstream knows which:
//
//   held   -> InputFrame (move vector, sprint, use-held, aim), sampled once per tick
//   edges  -> GameAction | UiAction, queued on press, drained once per frame
//
// Pad layout is the 2020 "xbox controller layout.jpg": left stick / d-pad move,
// right stick aim, A X Y LB RB = spells 1-5, B = Use/Talk/Push/Pull, RT sprint,
// View = bags, Menu = pause.

import type { InputFrame } from "@/sim/sim";
import type { UiAction } from "@/ui/host";

export type GameAction = { t: "use" } | { t: "bar"; slot: number } | { t: "quicksave" } | { t: "quickload" } | { t: "grid" };
export type Edge = { ui: UiAction } | { game: GameAction };

type Binding = { label: string; keys: string[]; pad: string; edges?: Edge[] };

const BAR_KEYS = ["Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8"];

/** The table the controls screen prints. Key codes are KeyboardEvent.code, layout independent. */
export const BINDINGS: Binding[] = [
  { label: "Move", keys: ["KeyW", "KeyA", "KeyS", "KeyD", "Arrows", "Hold RMB"], pad: "Left stick / D-pad" },
  { label: "Aim", keys: ["Mouse"], pad: "Right stick" },
  { label: "Sprint", keys: ["ShiftLeft", "ShiftRight"], pad: "RT" },
  { label: "Use / Talk / hold: Push, Pull", keys: ["KeyE", "KeyF"], pad: "B" },
  { label: "Bar slot 1 (melee)", keys: ["Space", "LMB", "Digit1"], pad: "A" },
  { label: "Bar slots 2-5", keys: ["Digit2", "Digit3", "Digit4", "Digit5"], pad: "X  Y  LB  RB" },
  { label: "Bar slots 6-8", keys: ["Digit6", "Digit7", "Digit8"], pad: "-" },
  { label: "Bags", keys: ["KeyI", "Tab"], pad: "View" },
  { label: "Spellbook / Quests / Map", keys: ["KeyK", "KeyJ", "KeyM"], pad: "View, then LB / RB" },
  { label: "Pause / back", keys: ["Escape"], pad: "Menu" },
  { label: "Quick save / load (slot 1)", keys: ["F5", "F9"], pad: "-" },
  { label: "Console", keys: ["Backquote"], pad: "-" },
  { label: "Debug overlay / path grid", keys: ["F2", "F3"], pad: "-" },
];

const PRETTY: Record<string, string> = {
  Backquote: "`",
  Escape: "Esc",
  ShiftLeft: "Shift",
  ShiftRight: "",
  Space: "Space",
};

export function bindingRows(): { action: string; keys: string; pad: string }[] {
  return BINDINGS.map((b) => ({
    action: b.label,
    keys: b.keys
      .map((k) => PRETTY[k] ?? k.replace(/^Key|^Digit/, ""))
      .filter((k) => k !== "")
      .join("  "),
    pad: b.pad,
  }));
}

const DEADZONE = 0.2; // 2020's value
const q = (v: number): number => Math.round(Math.max(-1, Math.min(1, v)) * 127) / 127;

export class Input {
  private readonly down = new Set<string>();
  private readonly edges: Edge[] = [];
  private readonly padWas: boolean[] = [];
  private mouseLeft = false;
  private mouseRight = false;
  private lastAimDevice: "mouse" | "pad" = "mouse";
  /** True when the last thing touched was a gamepad. On-screen hints follow it. */
  padActive = false;
  mouseX = 0;
  mouseY = 0;
  /** Set by the app each frame: true while a text field owns the keyboard. */
  captured = false;
  /** Set by the app each frame: true while a UI window or dialogue wants navigation keys. */
  uiMode = false;

  constructor(target: HTMLElement) {
    window.addEventListener("keydown", (e) => this.onKey(e, true));
    window.addEventListener("keyup", (e) => this.onKey(e, false));
    window.addEventListener("blur", () => {
      this.down.clear();
      this.mouseLeft = false;
      this.mouseRight = false;
    });
    target.addEventListener("pointerdown", (e) => {
      if (e.button === 0) {
        this.mouseLeft = true;
        if (!this.uiMode) this.edges.push({ game: { t: "bar", slot: 0 } });
      }
      if (e.button === 2) this.mouseRight = true;
      this.lastAimDevice = "mouse";
    });
    window.addEventListener("pointerup", (e) => {
      if (e.button === 0) this.mouseLeft = false;
      if (e.button === 2) this.mouseRight = false;
    });
    window.addEventListener("pointermove", (e) => {
      this.mouseX = e.clientX;
      this.mouseY = e.clientY;
      if (Math.abs(e.movementX) + Math.abs(e.movementY) > 1) this.lastAimDevice = "mouse";
    });
    target.addEventListener("contextmenu", (e) => e.preventDefault());
  }

  get aimingWithMouse(): boolean {
    return this.lastAimDevice === "mouse";
  }

  private onKey(e: KeyboardEvent, isDown: boolean): void {
    if (this.captured) {
      this.down.clear();
      return;
    }
    const code = e.code;
    if (["Tab", "Space", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "F2", "F3", "F5", "F9", "Backquote"].includes(code)) e.preventDefault();
    if (!isDown) {
      this.down.delete(code);
      return;
    }
    if (e.repeat) return;
    this.down.add(code);
    this.padActive = false;

    // Always-on UI toggles.
    if (code === "Backquote") return void this.edges.push({ ui: "terminal" });
    if (code === "Escape") return void this.edges.push({ ui: this.uiMode ? "cancel" : "pause" });
    if (code === "F2") return void this.edges.push({ ui: "debug" });
    if (code === "F3") return void this.edges.push({ game: { t: "grid" } });
    if (code === "F5") return void this.edges.push({ game: { t: "quicksave" } });
    if (code === "F9") return void this.edges.push({ game: { t: "quickload" } });
    if (code === "KeyI" || code === "Tab") return void this.edges.push({ ui: "bags" });
    if (code === "KeyK") return void this.edges.push({ ui: "book" });
    if (code === "KeyJ" || code === "KeyL") return void this.edges.push({ ui: "quests" });
    if (code === "KeyM") return void this.edges.push({ ui: "map" });

    if (this.uiMode) {
      // 2020 cycled windows with A / D; keep that, and arrows.
      const nav: Record<string, UiAction> = {
        ArrowUp: "up",
        KeyW: "up",
        ArrowDown: "down",
        KeyS: "down",
        ArrowLeft: "left",
        ArrowRight: "right",
        KeyA: "tabLeft",
        KeyD: "tabRight",
        KeyQ: "tabLeft",
        Enter: "confirm",
        Space: "confirm",
        KeyE: "confirm",
        KeyF: "confirm",
      };
      const a = nav[code];
      if (a) this.edges.push({ ui: a });
      return;
    }
    if (code === "KeyE" || code === "KeyF") return void this.edges.push({ game: { t: "use" } });
    if (code === "Space") return void this.edges.push({ game: { t: "bar", slot: 0 } });
    const slot = BAR_KEYS.indexOf(code);
    if (slot >= 0) this.edges.push({ game: { t: "bar", slot } });
  }

  private pollPad(frame: InputFrame): void {
    const pads = typeof navigator.getGamepads === "function" ? navigator.getGamepads() : [];
    const pad = pads.find((p) => p && p.connected && p.mapping === "standard") ?? pads.find((p) => p && p.connected);
    if (!pad) return;
    const pressed = (i: number): boolean => !!pad.buttons[i] && (pad.buttons[i].pressed || pad.buttons[i].value > 0.5);
    const edge = (i: number, e: Edge): void => {
      const now = pressed(i);
      if (now && !this.padWas[i]) {
        this.edges.push(e);
        this.padActive = true;
      }
      this.padWas[i] = now;
    };
    const lx = pad.axes[0] ?? 0;
    const ly = pad.axes[1] ?? 0;
    if (Math.sqrt(lx * lx + ly * ly) > DEADZONE) {
      frame.mx = lx;
      frame.my = ly;
    }
    if (pressed(14)) frame.mx = -1;
    if (pressed(15)) frame.mx = 1;
    if (pressed(12)) frame.my = -1;
    if (pressed(13)) frame.my = 1;
    const rx = pad.axes[2] ?? 0;
    const ry = pad.axes[3] ?? 0;
    if (Math.sqrt(rx * rx + ry * ry) > 0.35) {
      frame.ax = rx;
      frame.ay = ry;
      this.lastAimDevice = "pad";
    } else if (this.lastAimDevice === "pad") {
      frame.ax = 0;
      frame.ay = 0;
    }
    frame.sprint ||= pressed(7);
    frame.useHeld ||= pressed(1);

    if (this.uiMode) {
      edge(0, { ui: "confirm" });
      edge(1, { ui: "cancel" });
      edge(4, { ui: "tabLeft" });
      edge(5, { ui: "tabRight" });
      edge(12, { ui: "up" });
      edge(13, { ui: "down" });
      edge(14, { ui: "left" });
      edge(15, { ui: "right" });
    } else {
      edge(0, { game: { t: "bar", slot: 0 } });
      edge(2, { game: { t: "bar", slot: 1 } });
      edge(3, { game: { t: "bar", slot: 2 } });
      edge(4, { game: { t: "bar", slot: 3 } });
      edge(5, { game: { t: "bar", slot: 4 } });
      edge(1, { game: { t: "use" } });
    }
    edge(8, { ui: "bags" });
    edge(9, { ui: "pause" });
  }

  /**
   * Held state for this tick. `playerScreen` and `cursorWorld` let the mouse become
   * an aim vector and (right button held) a virtual stick toward the cursor, which
   * is how 2020's right-mouse walk worked: distance / 32 px = stick strength.
   */
  frame(player: { x: number; y: number } | null, cursorWorld: { x: number; y: number } | null): InputFrame {
    const f: InputFrame = { mx: 0, my: 0, sprint: false, useHeld: false, ax: 0, ay: 0 };
    if (!this.captured && !this.uiMode) {
      const d = this.down;
      if (d.has("KeyA") || d.has("ArrowLeft")) f.mx -= 1;
      if (d.has("KeyD") || d.has("ArrowRight")) f.mx += 1;
      if (d.has("KeyW") || d.has("ArrowUp")) f.my -= 1;
      if (d.has("KeyS") || d.has("ArrowDown")) f.my += 1;
      f.sprint = d.has("ShiftLeft") || d.has("ShiftRight");
      f.useHeld = d.has("KeyE") || d.has("KeyF");
      if (player && cursorWorld && this.lastAimDevice === "mouse") {
        const dx = cursorWorld.x - player.x;
        const dy = cursorWorld.y - (player.y - 6);
        const len = Math.sqrt(dx * dx + dy * dy);
        if (len > 2) {
          f.ax = dx / len;
          f.ay = dy / len;
          if (this.mouseRight && f.mx === 0 && f.my === 0) {
            const strength = Math.min(1, len / 32);
            f.mx = (dx / len) * strength;
            f.my = (dy / len) * strength;
          }
        }
      }
    }
    this.pollPad(f);
    f.mx = q(f.mx);
    f.my = q(f.my);
    f.ax = q(f.ax);
    f.ay = q(f.ay);
    return f;
  }

  drain(): Edge[] {
    return this.edges.splice(0, this.edges.length);
  }

  get leftHeld(): boolean {
    return this.mouseLeft;
  }
}
