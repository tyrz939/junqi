import Phaser from "phaser";
import { SCREEN_H, SCREEN_W } from "@/game/constants";
import type { ActionWorld } from "@/game/systems/actions";
import { runActions, type DialogueNode } from "@/game/systems/dialogue";
import { drawWindow, fillText, padRect, type Rect } from "@/game/ui/chrome";
import { gx, gy } from "@/game/ui/layout";
import { ink, uiText } from "@/game/ui/theme";
import { DEPTH, type Layers } from "@/game/ui/layers";

export class DialogueBox {
  open = false;
  private node: DialogueNode | null = null;
  private line = 0;
  private readonly gfx: Phaser.GameObjects.Graphics;
  private readonly body: Phaser.GameObjects.Text;
  private readonly optA: Phaser.GameObjects.Text;
  private readonly optB: Phaser.GameObjects.Text;
  private notes: string[] = [];
  private readonly box: Rect = { x: 0, y: 0, w: 0, h: 0 };

  constructor(scene: Phaser.Scene, layers: Layers) {
    this.box.w = gx(220);
    this.box.h = gy(56);
    this.box.x = Math.round((SCREEN_W - this.box.w) / 2);
    this.box.y = SCREEN_H - gy(96);
    this.gfx = layers.addUi(scene.add.graphics().setDepth(DEPTH.talk));
    this.body = layers.addUi(
      scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(8)}px`, color: ink }).setDepth(DEPTH.talkText),
    );
    this.optA = layers.addUi(
      scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(8)}px`, color: ink }).setDepth(DEPTH.talkText),
    );
    this.optB = layers.addUi(
      scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(8)}px`, color: ink }).setDepth(DEPTH.talkText),
    );
    this.hide();
  }

  start(node: DialogueNode | null): boolean {
    if (!node) return false;
    this.node = node;
    this.line = 0;
    this.open = true;
    this.paint();
    return true;
  }

  advance(): string[] {
    if (!this.node) return [];
    if (this.line < this.node.lines.length - 1) {
      this.line += 1;
      this.paint();
      return [];
    }
    if (!this.node.options?.length) return this.close();
    return [];
  }

  choose(index: number, world?: ActionWorld): string[] {
    const opt = this.node?.options?.[index];
    if (!opt) return [];
    this.notes = runActions(opt.actions, world);
    return this.close();
  }

  click(x: number, y: number, world?: ActionWorld): string[] {
    if (!this.open || !this.node) return [];
    const inner = padRect(this.box, gx(8), gy(6));
    const optH = gy(10);
    const opts = this.line >= this.node.lines.length - 1 ? this.node.options ?? [] : [];
    const aY = inner.y + inner.h - optH * 2;
    const bY = inner.y + inner.h - optH;
    if (opts[0] && inside(x, y, inner.x, aY, inner.w, optH)) return this.choose(0, world);
    if (opts[1] && inside(x, y, inner.x, bY, inner.w, optH)) return this.choose(1, world);
    if (inside(x, y, this.box.x, this.box.y, this.box.w, this.box.h)) return this.advance();
    return [];
  }

  private close(): string[] {
    this.open = false;
    this.node = null;
    this.hide();
    const out = this.notes;
    this.notes = [];
    return out;
  }

  private hide(): void {
    this.gfx.clear();
    this.body.setVisible(false);
    this.optA.setVisible(false);
    this.optB.setVisible(false);
  }

  private paint(): void {
    if (!this.node) return;
    this.gfx.clear();
    drawWindow(this.gfx, this.box.x, this.box.y, this.box.w, this.box.h, 1);
    const inner = padRect(this.box, gx(8), gy(6));
    const optH = gy(10);
    const opts = this.line >= this.node.lines.length - 1 ? this.node.options ?? [] : [];
    const strip = opts.length ? optH * 2 : optH;
    fillText(this.body, { x: inner.x, y: inner.y, w: inner.w, h: Math.max(1, inner.h - strip) }, this.node.lines[this.line] ?? "");
    const aY = inner.y + inner.h - optH * 2;
    const bY = inner.y + inner.h - optH;
    if (opts[0]) fillText(this.optA, { x: inner.x, y: aY, w: inner.w, h: optH }, `1  ${opts[0].label}`);
    else if (this.line < this.node.lines.length - 1) fillText(this.optA, { x: inner.x, y: bY, w: inner.w, h: optH }, "E");
    else this.optA.setVisible(false);
    if (opts[1]) fillText(this.optB, { x: inner.x, y: bY, w: inner.w, h: optH }, `2  ${opts[1].label}`);
    else this.optB.setVisible(false);
  }
}

function inside(px: number, py: number, x: number, y: number, w: number, h: number): boolean {
  return px >= x && px <= x + w && py >= y && py <= y + h;
}
