import Phaser from "phaser";
import { SCREEN_H } from "@/game/constants";
import { iconFor } from "@/game/systems/icons";
import { INNER, PAD, gx, gy, type Rect } from "@/game/ui/layout";
import { DEPTH, type Layers } from "@/game/ui/layers";
import {
  hpDark,
  hpFill,
  ink,
  mpDark,
  mpFill,
  parchment,
  parchmentDark,
  slotBg,
  slotEdge,
  slotHover,
  slotSel,
  uiText,
  windowEdge,
  windowHi,
} from "@/game/ui/theme";

export type { Rect } from "@/game/ui/layout";
export { padRect, inRect } from "@/game/ui/layout";

/** Wrap to width, crop to height. Use this instead of setText on boxed copy. */
export function fillText(t: Phaser.GameObjects.Text, rect: Rect, value: string): void {
  t.setVisible(true);
  t.setCrop();
  t.setPosition(rect.x, rect.y);
  t.setWordWrapWidth(Math.max(1, rect.w), true);
  t.setFixedSize(rect.w, rect.h);
  t.setText(value);
}

export function drawScrollThumb(
  g: Phaser.GameObjects.Graphics,
  rect: Rect,
  contentH: number,
  scrollY: number,
): void {
  if (contentH <= rect.h + 1) return;
  const trackW = 4;
  const x = rect.x + rect.w - trackW;
  const maxScroll = contentH - rect.h;
  const thumbH = Math.max(12, Math.round((rect.h / contentH) * rect.h));
  const y = rect.y + Math.round((scrollY / maxScroll) * (rect.h - thumbH));
  g.fillStyle(0x1a140c, 0.35);
  g.fillRect(x, rect.y, trackW, rect.h);
  g.fillStyle(0x2a1c10, 0.85);
  g.fillRect(x, y, trackW, thumbH);
}

/** Masked text pane. Offset by scrollY; thumb drawn separately. */
export class ScrollPane {
  readonly text: Phaser.GameObjects.Text;
  private readonly maskGfx: Phaser.GameObjects.Graphics;
  contentH = 1;
  scrollY = 0;

  constructor(scene: Phaser.Scene, layers: Layers, depth = DEPTH.guiText) {
    this.maskGfx = layers.addUi(scene.add.graphics().setAlpha(0).setDepth(depth));
    this.text = layers.addUi(
      scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(8)}px`, color: ink, lineSpacing: 4 }).setDepth(depth),
    );
    this.text.setMask(this.maskGfx.createGeometryMask());
  }

  show(rect: Rect, value: string, scrollY: number): { contentH: number; scrollY: number; maxScroll: number } {
    this.maskGfx.clear();
    this.maskGfx.fillStyle(0xffffff, 1);
    this.maskGfx.fillRect(rect.x, rect.y, rect.w, rect.h);
    this.text.setVisible(true);
    this.text.setFixedSize(0, 0);
    this.text.setWordWrapWidth(Math.max(1, rect.w - 8), true);
    this.text.setText(value);
    this.contentH = Math.max(1, Math.ceil(this.text.height));
    const maxScroll = Math.max(0, this.contentH - rect.h);
    this.scrollY = Phaser.Math.Clamp(Math.round(scrollY), 0, maxScroll);
    this.text.setPosition(rect.x, rect.y - this.scrollY);
    return { contentH: this.contentH, scrollY: this.scrollY, maxScroll };
  }

  hide(): void {
    this.maskGfx.clear();
    this.text.setVisible(false);
    this.text.setText("");
  }
}

export function clearText(t: Phaser.GameObjects.Text): void {
  t.setCrop();
  t.setFixedSize(0, 0);
  t.setText("");
  t.setVisible(false);
}

/**
 * Hover bubble copy. Wrap at TIP_MAX_W; grow height to the wrapped text; crop only past TIP_MAX_H
 * (~10 lines at gy(8)). Longer lore belongs in dialogue or the item's use-toast — Julie's letter has
 * no effect, so tryUseItem already returns the full description.
 */
export const TIP_MIN_W = gx(80);
export const TIP_MAX_W = gx(168);
export const TIP_MAX_H = gy(88);

export function paintTooltip(
  g: Phaser.GameObjects.Graphics,
  title: Phaser.GameObjects.Text,
  body: Phaser.GameObjects.Text,
  px: number,
  py: number,
  name: string,
  desc: string,
  titleColor: string,
  screenW: number,
): void {
  const padX = gx(4);
  const padY = gy(3);
  const innerW = TIP_MAX_W - padX * 2;
  title.setVisible(true).setCrop().setFixedSize(0, 0).setWordWrapWidth(innerW, true).setText(name);
  body.setVisible(true).setCrop().setFixedSize(0, 0).setWordWrapWidth(innerW, true).setText(desc);
  const nameH = Math.max(gy(8), Math.ceil(title.height));
  const descH = Math.max(gy(8), Math.ceil(body.height));
  const contentW = Math.max(TIP_MIN_W - padX * 2, Math.ceil(Math.max(title.width, body.width)));
  const w = Math.min(TIP_MAX_W, contentW + padX * 2);
  const needed = padY + nameH + gy(2) + descH + padY;
  const h = Math.min(TIP_MAX_H, needed);
  const x = Math.max(4, Math.min(px, screenW - w - 8));
  let boxY = py - h;
  if (boxY < 8) boxY = py + 12;
  if (boxY + h > SCREEN_H - 4) boxY = Math.max(4, SCREEN_H - h - 4);
  drawWindow(g, x, boxY, w, h, 3);
  const innerH = h - padY * 2;
  const titleH = Math.min(nameH, innerH);
  const bodyH = Math.max(1, innerH - titleH - gy(1));
  title.setColor(titleColor);
  fillText(title, { x: x + padX, y: boxY + padY, w: w - padX * 2, h: titleH }, name);
  fillText(body, { x: x + padX, y: boxY + padY + titleH + gy(1), w: w - padX * 2, h: bodyH }, desc);
}

export function drawWindow(
  g: Phaser.GameObjects.Graphics,
  x: number,
  y: number,
  w: number,
  h: number,
  style: 1 | 2 | 3 = 1,
): void {
  const fill = style === 1 ? parchment : style === 2 ? parchmentDark : 0x9a8860;
  g.fillStyle(0x000000, 0.28);
  g.fillRect(x + 3, y + 4, w, h);
  g.fillStyle(windowEdge, 1);
  g.fillRect(x, y, w, h);
  g.fillStyle(windowHi, 1);
  g.fillRect(x + 1, y + 1, w - 2, 2);
  g.fillStyle(0x2a1c10, 0.35);
  g.fillRect(x + 1, y + h - 3, w - 2, 2);
  g.fillStyle(fill, 0.96);
  g.fillRect(x + 3, y + 4, w - 6, h - 7);
}

export function drawBarTray(g: Phaser.GameObjects.Graphics, x: number, y: number, w: number, h: number): void {
  g.fillStyle(0x000000, 0.38);
  g.fillRect(x - 8, y - 5, w + 16, h + 12);
  g.fillStyle(0x16120c, 0.78);
  g.fillRect(x - 6, y - 4, w + 12, h + 10);
  g.lineStyle(1, 0x6a5a38, 0.55);
  g.strokeRect(x - 6, y - 4, w + 12, h + 10);
}

export function drawEnergy(
  g: Phaser.GameObjects.Graphics,
  x: number,
  y: number,
  w: number,
  h: number,
  frac: number,
  empty: boolean,
  fill = 0xd4b44a,
  warn = 0xe8c878,
): void {
  g.fillStyle(0x0c0a08, 0.9);
  g.fillRect(x, y, w, h);
  g.fillStyle(empty ? warn : fill, 1);
  g.fillRect(x + 1, y + 1, Math.max(0, (w - 2) * Phaser.Math.Clamp(frac, 0, 1)), h - 2);
}

export function drawHealthStack(
  g: Phaser.GameObjects.Graphics,
  x: number,
  y: number,
  w: number,
  h: number,
  top: number,
  bot: number,
  mage: boolean,
): void {
  g.fillStyle(0x000000, 0.35);
  g.fillRect(x + 2, y + 2, w, h);
  g.fillStyle(0x0c0a08, 0.9);
  g.fillRect(x, y, w, h);
  g.lineStyle(1, 0x6a5a38, 0.9);
  g.strokeRect(x, y, w, h);
  const mid = y + h * 0.5;
  const pad = 2;
  const innerW = w - pad * 2;
  g.fillStyle(hpDark, 1);
  g.fillRect(x + pad, y + pad, innerW, mid - y - pad - 1);
  g.fillStyle(hpFill, 1);
  const tw = Math.max(0, innerW * Phaser.Math.Clamp(top, 0, 1));
  g.fillRect(x + pad, y + pad, tw, mid - y - pad - 1);
  if (tw > 2) {
    g.fillStyle(0xffffff, 0.18);
    g.fillRect(x + pad, y + pad, tw, 2);
  }
  const botDark = mage ? mpDark : 0x321900;
  const botFill = mage ? mpFill : 0xe08a28;
  g.fillStyle(botDark, 1);
  g.fillRect(x + pad, mid + 1, innerW, y + h - mid - pad - 1);
  g.fillStyle(botFill, 1);
  const bw = Math.max(0, innerW * Phaser.Math.Clamp(bot, 0, 1));
  g.fillRect(x + pad, mid + 1, bw, y + h - mid - pad - 1);
  if (bw > 2) {
    g.fillStyle(0xffffff, 0.16);
    g.fillRect(x + pad, mid + 1, bw, 2);
  }
}

export function drawIconFrame(
  g: Phaser.GameObjects.Graphics,
  x: number,
  y: number,
  size: number,
  hovered: boolean,
  selected: boolean,
): void {
  g.fillStyle(0x000000, 0.3);
  g.fillRect(x + 2, y + 2, size, size);
  g.fillStyle(hovered || selected ? slotHover : slotEdge, 1);
  g.fillRect(x, y, size, size);
  g.fillStyle(slotBg, 1);
  g.fillRect(x + 2, y + 2, size - 4, size - 4);
  g.lineStyle(1, selected ? slotSel : 0x1a140c, 1);
  g.strokeRect(x, y, size, size);
}

export function drawIconFace(
  g: Phaser.GameObjects.Graphics,
  x: number,
  y: number,
  source: "item" | "spell",
  id: string,
): void {
  const icon = iconFor(source, id);
  const s = INNER;
  const ix = x + PAD;
  const iy = y + PAD;
  g.fillStyle(icon.fill, 1);
  g.fillRoundedRect(ix + 2, iy + 2, s - 4, s - 4, 3);
  g.fillStyle(0xffffff, 0.22);
  g.fillRect(ix + 3, iy + 3, s - 8, 3);
  const mark = source === "spell" ? 0xf4efe4 : 0x1a1208;
  g.fillStyle(mark, 0.35);
  g.fillRect(ix + s / 2 - 2, iy + 6, 4, s - 12);
}

export function drawCooldownClock(
  g: Phaser.GameObjects.Graphics,
  x: number,
  y: number,
  size: number,
  frac: number,
): void {
  if (frac <= 0) return;
  const cx = x + size / 2;
  const cy = y + size / 2;
  g.fillStyle(0x000000, 0.55);
  g.beginPath();
  g.moveTo(cx, cy);
  g.arc(cx, cy, size / 2, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * Math.min(1, frac), false);
  g.closePath();
  g.fillPath();
}

export function drawTooltipBox(
  g: Phaser.GameObjects.Graphics,
  x: number,
  y: number,
  w: number,
  h: number,
): void {
  drawWindow(g, x, y, w, h, 3);
}
