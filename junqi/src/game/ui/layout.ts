import { ACTION_BAR_SIZE, SCREEN_H, SCREEN_W } from "@/game/constants";
import type { GuiWindow } from "@/game/types";

/** 2020 GUI space: height 256, width follows aspect (`display_set_gui_size`). */
export const GH = 256;
export const GW = 256 * (SCREEN_W / SCREEN_H);

export function gx(n: number): number {
  return Math.round((n / GW) * SCREEN_W);
}

export function gy(n: number): number {
  return Math.round((n / GH) * SCREEN_H);
}

export type Rect = { x: number; y: number; w: number; h: number };

export function padRect(r: Rect, padX: number, padY = padX): Rect {
  return { x: r.x + padX, y: r.y + padY, w: Math.max(0, r.w - padX * 2), h: Math.max(0, r.h - padY * 2) };
}

export function inRect(px: number, py: number, r: Rect): boolean {
  return px >= r.x && px <= r.x + r.w && py >= r.y && py <= r.y + r.h;
}

export const ICON = gx(28);
export const INNER = gx(24);
export const SPACING = gx(30);
export const PAD = gx(2);

const iconOff = SPACING * 4 - gx(1);

export const barX = Math.round(SCREEN_W / 2 - iconOff);
export const barY = gy(GH - 32);
export const invX = barX;
export const invY = gy(GH - 162);

export const hpX = gx(8);
export const hpY = gy(8);
export const hpW = gx(96);
export const hpH = gy(22);
export const energyY = gy(31);
export const targetX = gx(106);
export const clockX = SCREEN_W - gx(8);
export const clockY = gy(6);
export const trackX = SCREEN_W - gx(8);
export const trackY = gy(20);
export const trackW = gx(110);
export const trackH = gy(22);

export const TAB_IDS: GuiWindow[] = ["inventory", "spellbook", "quest", "map"];
export const TAB_LABELS = ["Bag", "Book", "Quest", "Map"];
export const TAB_W = gx(38);
export const TAB_H = gy(12);
export const COLS = 8;
export const BOOK_CARDS = 3;

/** One sheet for Bag / Book / Quest / Map. Under HP, above the bar. Tab strip sits in the gap. */
export const panel: Rect = {
  x: gx(8),
  y: hpY + hpH + gy(6) + TAB_H,
  w: SCREEN_W - gx(16),
  h: barY - gy(10) - (hpY + hpH + gy(6) + TAB_H),
};

export const winX = panel.x;
export const winY = panel.y;
export const winW = panel.w;
export const winH = panel.h;
export const pocket = panel;

export const panelTabs: Rect[] = TAB_IDS.map((_, i) => ({
  x: panel.x + i * (TAB_W + gx(3)),
  y: panel.y - TAB_H + 1,
  w: TAB_W,
  h: TAB_H,
}));

const gridW = SPACING * (COLS - 1) + ICON;

/** Stats + craft, sitting over the action-bar column. */
export const header: Rect = {
  x: invX,
  y: panel.y + gy(4),
  w: gridW,
  h: ICON,
};

export const stats: Rect = { x: header.x, y: header.y, w: gx(118), h: header.h };

export function craftSlot(i: number): Rect {
  return { x: invX + gx(120) + SPACING * i, y: header.y, w: ICON, h: ICON };
}

export function bagSlot(i: number): Rect {
  const col = i % COLS;
  const row = Math.floor(i / COLS);
  const gridY = header.y + header.h + gy(4);
  return { x: invX + SPACING * col, y: gridY + row * SPACING, w: ICON, h: ICON };
}

export const pager: Rect = {
  x: panel.x + gx(4),
  y: panel.y + panel.h - gy(18),
  w: panel.w - gx(8),
  h: gy(16),
};

export const pagerPrev: Rect = { x: pager.x, y: pager.y, w: gx(48), h: pager.h };
export const pagerNext: Rect = { x: pager.x + pager.w - gx(48), y: pager.y, w: gx(48), h: pager.h };

export function bookCard(i: number): Rect {
  const bodyTop = panel.y + gy(4);
  const bodyH = pager.y - gy(2) - bodyTop;
  const gap = gy(2);
  const h = Math.floor((bodyH - gap * (BOOK_CARDS - 1)) / BOOK_CARDS);
  return { x: panel.x + gx(4), y: bodyTop + i * (h + gap), w: panel.w - gx(8), h };
}

export const mapInner = padRect(panel, gx(4), gy(4));

export const questList: Rect = {
  x: panel.x + gx(4),
  y: panel.y + gy(6),
  w: Math.floor(panel.w * 0.38),
  h: panel.h - gy(12),
};

export const questDetail: Rect = padRect(
  {
    x: questList.x + questList.w,
    y: panel.y,
    w: panel.w - questList.w - gx(4),
    h: panel.h,
  },
  gx(6),
  gy(8),
);

export const questRowH = gy(16);

export function barSlot(i: number): Rect {
  return { x: barX + i * SPACING, y: barY, w: ICON, h: ICON };
}

export const barTray: Rect = {
  x: barX - 8,
  y: barY - 6,
  w: SPACING * (ACTION_BAR_SIZE - 1) + ICON + 16,
  h: ICON + 12,
};
