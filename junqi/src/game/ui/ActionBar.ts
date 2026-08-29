import Phaser from "phaser";
import { ACTION_BAR_SIZE, GCD_SECONDS, SCREEN_W } from "@/game/constants";
import { getItem, getSpell } from "@/game/systems/catalog";
import { iconFor } from "@/game/systems/icons";
import { inventoryQuantity, tryUseItem } from "@/game/systems/inventory";
import { session } from "@/game/systems/session";
import { drawBarTray, drawCooldownClock, drawIconFace, drawIconFrame, paintTooltip, clearText } from "@/game/ui/chrome";
import { INNER, ICON, PAD, SPACING, barSlot, barTray, barX, barY, gx, gy, inRect } from "@/game/ui/layout";
import { cream, fuchsia, muted, uiText, white, yellow } from "@/game/ui/theme";
import { DEPTH, type Layers } from "@/game/ui/layers";
import type { Unit } from "@/game/entities/Unit";
import { DRAG2 } from "@/game/ui/drag";

export class ActionBar {
  readonly x = barX;
  readonly y = barY;
  readonly width = SPACING * (ACTION_BAR_SIZE - 1) + ICON;
  readonly height = ICON;
  private readonly gfx: Phaser.GameObjects.Graphics;
  private readonly tipGfx: Phaser.GameObjects.Graphics;
  private readonly heldGfx: Phaser.GameObjects.Graphics;
  private readonly glyphs: Phaser.GameObjects.Text[] = [];
  private readonly qtys: Phaser.GameObjects.Text[] = [];
  private readonly nums: Phaser.GameObjects.Text[] = [];
  private readonly heldGlyph: Phaser.GameObjects.Text;
  private readonly tipName: Phaser.GameObjects.Text;
  private readonly tipDesc: Phaser.GameObjects.Text;
  hover = -1;
  private tipX = 0;
  private tipY = 0;
  private pressBar: { index: number; x: number; y: number } | null = null;
  private dragging = false;

  constructor(scene: Phaser.Scene, layers: Layers) {
    this.gfx = layers.addUi(scene.add.graphics().setDepth(DEPTH.bar));
    this.tipGfx = layers.addUi(scene.add.graphics().setDepth(DEPTH.tip));
    this.heldGfx = layers.addUi(scene.add.graphics().setDepth(DEPTH.held));
    for (let i = 0; i < ACTION_BAR_SIZE; i++) {
      const sx = barX + i * SPACING;
      this.glyphs.push(
        layers.addUi(
          scene.add
            .text(sx + ICON / 2, barY + ICON / 2 - 2, "", { ...uiText, fontSize: `${gy(8)}px`, color: "#1a1208" })
            .setOrigin(0.5)
            .setDepth(DEPTH.barGlyph),
        ),
      );
      this.qtys.push(
        layers.addUi(
          scene.add
            .text(sx + ICON - 2, barY + ICON - 1, "", { ...uiText, fontSize: `${gy(7)}px`, color: white })
            .setOrigin(1, 1)
            .setDepth(DEPTH.barGlyph),
        ),
      );
      this.nums.push(
        layers.addUi(
          scene.add
            .text(sx + 3, barY + ICON - 2, String(i + 1), { ...uiText, fontSize: `${gy(6)}px`, color: muted })
            .setOrigin(0, 1)
            .setDepth(DEPTH.barGlyph),
        ),
      );
    }
    this.heldGlyph = layers.addUi(
      scene.add
        .text(0, 0, "", { ...uiText, fontSize: `${gy(7)}px`, color: cream })
        .setOrigin(0.5)
        .setDepth(DEPTH.heldGlyph)
        .setVisible(false),
    );
    this.tipName = layers.addUi(scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(8)}px` }).setDepth(DEPTH.tipText));
    this.tipDesc = layers.addUi(scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(8)}px`, color: yellow }).setDepth(DEPTH.tipText));
  }

  hit(x: number, y: number): boolean {
    return inRect(x, y, barTray);
  }

  slotAt(x: number, y: number): number {
    for (let i = 0; i < ACTION_BAR_SIZE; i++) {
      if (inRect(x, y, barSlot(i))) return i;
    }
    return -1;
  }

  click(x: number, y: number, right: boolean): string | null {
    const i = this.slotAt(x, y);
    if (i < 0) return null;
    if (!right && session.held) {
      session.actionBar[i] = { source: session.held.source, id: session.held.id };
      session.held = null;
      return "Bound";
    }
    return this.use(i);
  }

  press(x: number, y: number, right: boolean): { msg: string | null; grab?: number } {
    const i = this.slotAt(x, y);
    if (i < 0) return { msg: null };
    if (session.held) return { msg: null };
    if (right) {
      if (!session.actionBar[i]) return { msg: null };
      session.actionBar[i] = null;
      return { msg: "Unbound" };
    }
    this.pressBar = { index: i, x, y };
    this.dragging = false;
    return { msg: null, grab: i };
  }

  release(x: number, y: number): string | null {
    const i = this.slotAt(x, y);
    const press = this.pressBar;
    const dragged = this.dragging;
    this.pressBar = null;
    this.dragging = false;
    if (session.held && i >= 0) {
      if (press && dragged) {
        if (i !== press.index) {
          const moving = session.actionBar[press.index];
          session.actionBar[press.index] = session.actionBar[i];
          session.actionBar[i] = moving;
        }
        session.held = null;
        return null;
      }
      session.actionBar[i] = { source: session.held.source, id: session.held.id };
      session.held = null;
      return "Bound";
    }
    if (session.held) {
      session.held = null;
      return null;
    }
    if (press && !dragged) return this.use(press.index);
    return null;
  }

  cancelPress(): void {
    this.pressBar = null;
    this.dragging = false;
  }

  use(i: number): string | null {
    const slot = session.actionBar[i];
    if (!slot) return null;
    if (slot.source === "item") return tryUseItem(session.player, slot.id);
    return `cast:${slot.id}`;
  }

  update(pointerX: number, pointerY: number, player: Unit, targetMetres: number | null): void {
    this.hover = this.slotAt(pointerX, pointerY);
    this.tipX = pointerX;
    this.tipY = pointerY;
    if (this.pressBar && !this.dragging) {
      const dx = pointerX - this.pressBar.x;
      const dy = pointerY - this.pressBar.y;
      if (dx * dx + dy * dy > DRAG2) {
        this.dragging = true;
        const slot = session.actionBar[this.pressBar.index];
        if (slot) session.held = { source: slot.source, id: slot.id };
      }
    }
    this.draw(player, targetMetres);
  }

  private draw(player: Unit, targetMetres: number | null): void {
    this.gfx.clear();
    this.tipGfx.clear();
    this.heldGfx.clear();
    drawBarTray(this.gfx, barX, barY, this.width, this.height);
    let tipName = "";
    let tipDesc = "";
    let tipSpell = false;
    for (let i = 0; i < ACTION_BAR_SIZE; i++) {
      const r = barSlot(i);
      const sx = r.x;
      const slot = session.actionBar[i];
      drawIconFrame(this.gfx, sx, barY, ICON, this.hover === i, Boolean(session.held));
      if (!slot) {
        this.glyphs[i].setText("");
        this.qtys[i].setText("");
        continue;
      }
      drawIconFace(this.gfx, sx, barY, slot.source, slot.id);
      this.glyphs[i].setText(iconFor(slot.source, slot.id).glyph);
      this.glyphs[i].setPosition(sx + ICON / 2, barY + ICON / 2 - 1);

      const frac = cdFrac(player, slot.source, slot.id);
      drawCooldownClock(this.gfx, sx + PAD, barY + PAD, INNER, frac);

      if (slot.source === "item") {
        const qty = inventoryQuantity(player, slot.id);
        this.qtys[i].setText(String(qty));
        if (qty <= 0) this.gfx.fillStyle(0x000000, 0.45).fillRect(sx + PAD, barY + PAD, INNER, INNER);
      } else {
        const spell = getSpell(slot.id);
        this.qtys[i].setText("");
        if (player.mp < spell.mpCost || player.energy < spell.energyCost) {
          this.gfx.fillStyle(0x000000, 0.5);
          this.gfx.fillRect(sx + PAD, barY + PAD, INNER, INNER);
        }
        const pip = targetMetres != null && targetMetres > spell.range ? 0xff0000 : 0xffffff;
        this.gfx.fillStyle(pip, 1);
        this.gfx.fillRect(sx + PAD, barY + PAD, gx(2), gy(2));
      }

      if (this.hover === i) {
        tipSpell = slot.source === "spell";
        tipName = tipSpell ? getSpell(slot.id).name : getItem(slot.id).name;
        tipDesc = tipSpell ? getSpell(slot.id).description : getItem(slot.id).description;
      }
    }

    if (session.held) {
      tipName = session.held.source === "spell" ? getSpell(session.held.id).name : getItem(session.held.id).name;
      tipSpell = session.held.source === "spell";
      if (this.pressBar || this.dragging) {
        tipDesc = "Drop on a slot · drop outside to unbind · Esc cancel";
      } else if (tipSpell) {
        tipDesc = "Drop on bar to bind · drop outside to cancel";
      } else {
        tipDesc = "Drop on bar to bind · drop outside to destroy · Esc cancel";
      }
    }

    const show = Boolean(tipName);
    if (show) {
      paintTooltip(this.tipGfx, this.tipName, this.tipDesc, this.tipX, this.tipY, tipName, tipDesc, tipSpell ? fuchsia : white, SCREEN_W);
    } else {
      clearText(this.tipName);
      clearText(this.tipDesc);
    }

    if (session.held) {
      const icon = iconFor(session.held.source, session.held.id);
      const hx = this.tipX + 12;
      const hy = this.tipY + 12;
      this.heldGfx.fillStyle(0x000000, 0.35);
      this.heldGfx.fillRect(hx + 2, hy + 2, 18, 18);
      this.heldGfx.fillStyle(icon.fill, 1);
      this.heldGfx.fillRoundedRect(hx, hy, 18, 18, 3);
      this.heldGlyph.setVisible(true).setText(icon.glyph).setPosition(hx + 9, hy + 9);
    } else {
      this.heldGlyph.setVisible(false);
    }
  }
}

function cdFrac(player: Unit, source: "item" | "spell", id: string): number {
  let frac = 0;
  let actual = 0;
  let checkGcd = false;
  if (source === "spell") {
    const spell = getSpell(id);
    checkGcd = !spell.gcdImmune;
    actual = player.cooldowns[id] ?? 0;
    frac = spell.cooldown > 0 ? actual / spell.cooldown : 0;
  } else {
    const item = getItem(id);
    checkGcd = item.usable;
    const row = player.itemCooldowns.find((c) => c.id === id);
    actual = row?.t ?? 0;
    frac = item.cooldown > 0 ? actual / item.cooldown : 0;
  }
  if (checkGcd && player.gcd > 0 && player.gcd / GCD_SECONDS > frac) {
    frac = player.gcd / GCD_SECONDS;
  }
  return Phaser.Math.Clamp(frac, 0, 1);
}
