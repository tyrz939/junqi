// Pure helpers: text formatting, sweep maths, signatures. No DOM in here, so
// test/ui.test.ts can run it under node.

import { TICK_RATE } from "@/sim/constants";
import type { BarSlot, Stack } from "@/sim/state";
import type { SlotInfo } from "@/ui/host";

/** Integer UI scale. Height-driven (360 design lines); width only matters on portrait screens. */
export function uiScale(width: number, height: number): number {
  return Math.max(1, Math.min(Math.floor(height / 360), Math.floor(width / 400)));
}

export function clamp01(n: number): number {
  return n < 0 ? 0 : n > 1 ? 1 : n;
}

/** Float hour 0..24 -> "HH:MM". */
export function clockText(hour: number): string {
  const total = (((Math.floor(hour * 60 + 1e-6) % 1440) + 1440) % 1440) | 0;
  const hh = Math.floor(total / 60);
  const mm = total % 60;
  return `${hh < 10 ? "0" : ""}${hh}:${mm < 10 ? "0" : ""}${mm}`;
}

/** Lamp-post hours: the HUD shows a moon between 18:30 and 06:30. */
export function isNight(hour: number): boolean {
  return hour < 6.5 || hour >= 18.5;
}

export function dayText(day: number): string {
  return `Day ${day + 1}`;
}

/** Remaining time on a status chip: whole seconds, minutes once past 99 s. */
export function statusTime(ticks: number): string {
  const s = Math.ceil(ticks / TICK_RATE);
  return s > 99 ? `${Math.ceil(s / 60)}m` : String(Math.max(0, s));
}

/** A catalog duration (ticks) for tooltips: "1.5 s", "12 s", "2 min". */
export function durationText(ticks: number): string {
  const s = ticks / TICK_RATE;
  if (s >= 120) return `${Math.round((s / 60) * 10) / 10} min`;
  return `${Math.round(s * 10) / 10} s`;
}

/**
 * Cooldown sweep as ELAPSED percent, 0..100 in half-percent steps so the style
 * only changes ~200 times per cooldown. 100 means "ready" (overlay invisible).
 */
export function sweepPercent(remaining: number, total: number): number {
  if (!(remaining > 0) || !(total > 0)) return 100;
  const left = clamp01(remaining / total);
  return Math.round((1 - left) * 200) / 2;
}

export function costText(spell: { mp: number; energy: number }): string {
  const parts: string[] = [];
  if (spell.mp > 0) parts.push(`${spell.mp} MP`);
  if (spell.energy > 0) parts.push(`${spell.energy} EN`);
  return parts.length > 0 ? parts.join(" + ") : "Free";
}

export function rangeText(spell: { kind: string; range: number }): string {
  if (spell.kind === "self") return "Self";
  if (spell.kind === "ally") return `Yourself, or the friend under the cursor within ${spell.range} m`;
  if (spell.kind === "melee") return `Melee ${spell.range} m`;
  return `${spell.range} m`;
}

export function lootText(name: string, qty: number): string {
  return `+${qty} ${name}`;
}

export function stackSig(s: Stack | null | undefined): string {
  return s ? `${s.item}*${s.qty}` : "";
}

export function barSlotSig(slot: BarSlot, qty: number): string {
  if (!slot) return "";
  return slot.source === "spell" ? `s:${slot.id}` : `i:${slot.id}*${qty}`;
}

export function firstEmpty(bar: readonly BarSlot[]): number {
  for (let i = 0; i < bar.length; i++) if (!bar[i]) return i;
  return -1;
}

export function slotSummary(info: SlotInfo | null): string {
  if (!info) return "Empty";
  return `${info.zone} - ${dayText(info.day)}, ${clockText(info.hour)} - HP ${Math.ceil(info.hp)}/${Math.round(info.maxhp)}`;
}

/** Index of the most recently written slot, -1 when all are empty. */
export function latestSlot(slots: readonly (SlotInfo | null)[]): number {
  let best = -1;
  let bestKey = -Infinity;
  let bestText = "";
  slots.forEach((s, i) => {
    if (!s) return;
    const t = Date.parse(s.savedAt);
    const key = Number.isNaN(t) ? -Infinity : t;
    if (best < 0 || key > bestKey || (key === bestKey && s.savedAt > bestText)) {
      best = i;
      bestKey = key;
      bestText = s.savedAt;
    }
  });
  return best;
}

/** Next enabled index from `from` in direction `dir`, wrapping. Returns `from` when nothing else is enabled. */
export function stepIndex(disabled: readonly boolean[], from: number, dir: 1 | -1): number {
  const n = disabled.length;
  if (n === 0) return 0;
  for (let k = 1; k <= n; k++) {
    const i = (((from + dir * k) % n) + n) % n;
    if (!disabled[i]) return i;
  }
  return from;
}

/** "#rgb", "#rrggbb", "rgb(r,g,b)" / "rgba(...)". null = let the browser parse it. */
export function parseCssColor(s: string): [number, number, number] | null {
  const t = s.trim();
  let m = /^#([0-9a-f]{3})$/i.exec(t);
  if (m) {
    const v = m[1];
    return [parseInt(v[0] + v[0], 16), parseInt(v[1] + v[1], 16), parseInt(v[2] + v[2], 16)];
  }
  m = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(t);
  if (m) {
    const n = parseInt(m[1], 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  }
  m = /^rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)/i.exec(t);
  if (m) return [Math.min(255, +m[1]), Math.min(255, +m[2]), Math.min(255, +m[3])];
  return null;
}

/** Little-endian RGBA word for a Uint32Array view over ImageData. */
export function packRgb(r: number, g: number, b: number): number {
  return ((255 << 24) | (b << 16) | (g << 8) | r) >>> 0;
}

/** First key of the binding row that looks like the USE action, for the "[E] Talk" prompt. */
export function useKeyLabel(rows: readonly { action: string; keys: string }[]): string {
  const row = rows.find((r) => /\b(use|interact|talk)\b/i.test(r.action));
  if (!row) return "E";
  const first = row.keys.split(/[\s,/|]+/).find((k) => k.length > 0);
  return first ?? "E";
}
