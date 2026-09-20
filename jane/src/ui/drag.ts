// Drag and drop on Pointer Events: one path for mouse, pen and touch. The HTML5
// drag API does not fire on touch screens and cannot style its ghost, so this is
// pointerdown -> (move past a threshold) -> floating ghost -> pointerup, with the
// drop target found by `elementFromPoint` and a `data-drop` attribute.
//
// The same manager also tells clicks, double clicks / double taps and context
// clicks (right button, long press) apart, because they start the same way.

import { h } from "@/ui/dom";
import { parseDropTarget, type DragPayload, type DropTarget } from "@/ui/nav";

export type DragSource = {
  /** What a drag from this element carries right now; null = not draggable at the moment. */
  payload(): DragPayload | null;
  /** Icon id for the ghost. */
  iconId(): string;
  click?(): void;
  double?(): void;
  context?(): void;
};

const THRESHOLD = 5;
const DOUBLE_MS = 380;

type Live = {
  el: HTMLElement;
  src: DragSource;
  pointer: number;
  x0: number;
  y0: number;
  payload: DragPayload | null;
  dragging: boolean;
};

export class DragManager {
  /** Set by the window: what a finished drag (or pad put-down) means. */
  onDrop: (p: DragPayload, t: DropTarget, x: number, y: number) => void = () => {};
  /** Called when a drag starts, so tooltips and popovers can get out of the way. */
  onStart: () => void = () => {};

  private readonly ghost: HTMLDivElement;
  private readonly held: HTMLDivElement;
  private live: Live | null = null;
  private hover: Element | null = null;
  private lastClickEl: HTMLElement | null = null;
  private lastClickAt = 0;
  private heldKey = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly icon: (id: string) => string,
  ) {
    this.ghost = h("div", "jq-ghost", root);
    this.ghost.hidden = true;
    this.held = h("div", "jq-ghost jq-ghost-held", root);
    this.held.hidden = true;
  }

  active(): boolean {
    return this.live?.dragging ?? false;
  }

  attach(el: HTMLElement, src: DragSource): void {
    el.addEventListener("pointerdown", (ev) => {
      if (ev.button !== 0 || this.live) return;
      this.live = { el, src, pointer: ev.pointerId, x0: ev.clientX, y0: ev.clientY, payload: src.payload(), dragging: false };
      try {
        el.setPointerCapture(ev.pointerId);
      } catch {
        // Capture is a nicety (keeps moves coming when the pointer leaves the element).
      }
    });
    el.addEventListener("pointermove", (ev) => {
      const l = this.live;
      if (!l || l.el !== el || l.pointer !== ev.pointerId) return;
      if (!l.dragging) {
        if (!l.payload) return;
        const dx = ev.clientX - l.x0;
        const dy = ev.clientY - l.y0;
        if (dx * dx + dy * dy < THRESHOLD * THRESHOLD) return;
        l.dragging = true;
        el.classList.add("jq-drag-src");
        this.root.classList.add("jq-dragging");
        const url = this.icon(src.iconId());
        this.ghost.style.backgroundImage = url ? `url("${url}")` : "";
        this.ghost.hidden = false;
        this.onStart();
      }
      this.place(this.ghost, ev.clientX, ev.clientY);
      this.setHover(this.elementAt(ev.clientX, ev.clientY));
    });
    el.addEventListener("pointerup", (ev) => {
      const l = this.live;
      if (!l || l.el !== el || l.pointer !== ev.pointerId) return;
      const wasDragging = l.dragging;
      const payload = l.payload;
      this.finish();
      if (wasDragging && payload) {
        const target = parseDropTarget(this.elementAt(ev.clientX, ev.clientY)?.getAttribute("data-drop"));
        this.onDrop(payload, target, ev.clientX, ev.clientY);
        return;
      }
      const now = performance.now();
      if (src.double && this.lastClickEl === el && now - this.lastClickAt < DOUBLE_MS) {
        this.lastClickEl = null;
        src.double();
        return;
      }
      this.lastClickEl = el;
      this.lastClickAt = now;
      src.click?.();
    });
    el.addEventListener("pointercancel", (ev) => {
      if (this.live && this.live.el === el && this.live.pointer === ev.pointerId) this.finish();
    });
    el.addEventListener("contextmenu", (ev) => {
      ev.preventDefault();
      if (this.live?.dragging) return;
      if (this.live && this.live.el === el) this.finish();
      src.context?.();
    });
  }

  /** Pad "picked up" indicator: the ghost parked on the cursor cell. "" hides it. */
  showHeld(iconId: string, over: HTMLElement | null): void {
    const r = over?.getBoundingClientRect();
    const key = iconId && r ? `${iconId}@${Math.round(r.left)},${Math.round(r.top)}` : "";
    if (key === this.heldKey) return;
    this.heldKey = key;
    if (!key || !r) {
      this.held.hidden = true;
      return;
    }
    const url = this.icon(iconId);
    this.held.style.backgroundImage = url ? `url("${url}")` : "";
    this.held.hidden = false;
    this.place(this.held, r.left + r.width * 0.75, r.top + r.height * 0.2);
  }

  private place(el: HTMLElement, clientX: number, clientY: number): void {
    const r = this.root.getBoundingClientRect();
    el.style.transform = `translate(${Math.round(clientX - r.left)}px, ${Math.round(clientY - r.top)}px) translate(-50%, -50%)`;
  }

  private elementAt(x: number, y: number): Element | null {
    const hit = document.elementFromPoint(x, y);
    if (!hit || !this.root.contains(hit)) return null;
    return hit.closest("[data-drop]");
  }

  private setHover(el: Element | null): void {
    if (el === this.hover) return;
    this.hover?.classList.remove("jq-drop-hover");
    this.hover = el;
    el?.classList.add("jq-drop-hover");
  }

  private finish(): void {
    const l = this.live;
    this.live = null;
    if (!l) return;
    try {
      l.el.releasePointerCapture(l.pointer);
    } catch {
      // Already released.
    }
    l.el.classList.remove("jq-drag-src");
    this.root.classList.remove("jq-dragging");
    this.ghost.hidden = true;
    this.setHover(null);
  }
}
