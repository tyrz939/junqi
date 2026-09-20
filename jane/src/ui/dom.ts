// Tiny DOM kit. The rule for the whole UI: build once, then patch through
// `cached` setters so a frame that changes nothing touches no DOM at all.

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  cls = "",
  parent?: HTMLElement | null,
  text?: string,
): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  if (parent) parent.appendChild(e);
  return e;
}

/** Wraps a DOM write so it only runs when the value differs from the last one written. */
export function cached<T>(apply: (v: T) => void): (v: T) => void {
  let last: T | undefined;
  let first = true;
  return (v: T): void => {
    if (!first && v === last) return;
    first = false;
    last = v;
    apply(v);
  };
}

export const textOf = (e: HTMLElement): ((s: string) => void) =>
  cached<string>((s) => {
    e.textContent = s;
  });

export const visOf = (e: HTMLElement): ((on: boolean) => void) =>
  cached<boolean>((on) => {
    e.hidden = !on;
  });

export const classOf = (e: HTMLElement, cls: string): ((on: boolean) => void) =>
  cached<boolean>((on) => {
    e.classList.toggle(cls, on);
  });

export const iconOf = (e: HTMLElement): ((url: string) => void) =>
  cached<string>((url) => {
    e.style.backgroundImage = url ? `url("${url}")` : "";
  });

export const varOf = (e: HTMLElement, name: string): ((v: string) => void) =>
  cached<string>((v) => {
    e.style.setProperty(name, v);
  });

export function clear(e: HTMLElement): void {
  e.textContent = "";
}

/** N design pixels (1x = 640x360) as a CSS length. */
export function u(n: number): string {
  return `calc(${Math.round(n * 100) / 100} * var(--u))`;
}

/** An inventory / bar cell: framed icon plus a quantity. Patched by signature. */
export class SlotView {
  readonly el: HTMLDivElement;
  readonly icon: HTMLDivElement;
  readonly qty: HTMLSpanElement;
  private sig = "\u0000";

  constructor(cls = "") {
    this.el = h("div", `jq-slot ${cls}`.trim());
    this.icon = h("div", "jq-icon", this.el);
    this.qty = h("span", "jq-qty", this.el);
  }

  set(sig: string, iconUrl: string, qty: string): void {
    if (sig === this.sig) return;
    this.sig = sig;
    this.icon.style.backgroundImage = iconUrl ? `url("${iconUrl}")` : "";
    this.qty.textContent = qty;
    this.el.classList.toggle("filled", sig !== "");
  }
}
