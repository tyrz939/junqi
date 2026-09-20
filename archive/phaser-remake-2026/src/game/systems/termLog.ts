/** CGA palette, 2020 markup, and the println sink. */

export const CGA = {
  black: "#000000",
  gray: "#555555",
  blue: "#0000aa",
  ltblue: "#5555ff",
  green: "#00aa00",
  ltgreen: "#55ff55",
  cyan: "#00aaaa",
  ltcyan: "#55ffff",
  red: "#aa0000",
  ltred: "#ff5555",
  magenta: "#aa00aa",
  ltmagenta: "#ff55ff",
  brown: "#aa5500",
  yellow: "#ffff55",
  ltgray: "#aaaaaa",
  white: "#ffffff",
} as const;

export type CgaName = keyof typeof CGA;

export const TERM_VERSION = "0.1.0";
export const TERM_BOOT = new Date();
export const LOG_CAP = 500;

const lines: string[] = [];
const listeners = new Set<() => void>();

export function paint(color: CgaName, text: string): string {
  return `[$=${color}]${text}[$=white]`;
}

export function onTermChange(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function notify(): void {
  for (const fn of listeners) fn();
}

export function termLines(): readonly string[] {
  return lines;
}

export function println(text: string): void {
  const value = String(text);
  for (const part of value.split("\n")) {
    lines.push(part);
  }
  if (lines.length > LOG_CAP) lines.splice(0, lines.length - LOG_CAP);
  notify();
}

export function printError(text: string): boolean {
  println(paint("ltmagenta", String(text)));
  return false;
}

export function tClear(): void {
  lines.length = 0;
  notify();
}

export function stripColor(text: string): string {
  return text.replace(/\[\$=[^\]]*\]/g, "");
}

export function lineToHtml(raw: string): string {
  let html = "";
  let color: string = CGA.white;
  const re = /\[\$=([^\]]*)\]/g;
  let last = 0;
  let match: RegExpExecArray | null;
  while ((match = re.exec(raw))) {
    html += span(color, raw.slice(last, match.index));
    color = resolveColor(match[1] ?? "white");
    last = match.index + match[0].length;
  }
  html += span(color, raw.slice(last));
  return html || span(CGA.white, "");
}

function resolveColor(token: string): string {
  const key = token.trim().toLowerCase() as CgaName;
  if (key in CGA) return CGA[key];
  if (/^#?[0-9a-f]{6}$/i.test(token.trim())) {
    const hex = token.trim();
    return hex.startsWith("#") ? hex : `#${hex}`;
  }
  return CGA.white;
}

function span(color: string, text: string): string {
  if (!text) return "";
  return `<span style="color:${color}">${escapeHtml(text)}</span>`;
}

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

export function bannerLines(): string[] {
  const built = TERM_BOOT.toLocaleString();
  return [
    `${paint("ltmagenta", "JUNQI")}${paint("ltred", ` v${TERM_VERSION}`)}`,
    `Compiled on: ${paint("ltred", built)}  Runtime: ${paint("ltred", "TypeScript / Phaser 3")}`,
    "-------------------------------------",
    `type ${paint("yellow", "help")}  ·  ${paint("yellow", "cmdlist")}  ·  ${paint("yellow", "ls")}`,
  ];
}

export function printBanner(): void {
  for (const line of bannerLines()) println(line);
}
