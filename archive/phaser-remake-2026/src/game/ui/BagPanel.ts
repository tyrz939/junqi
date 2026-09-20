import Phaser from "phaser";
import { SCREEN_W } from "@/game/constants";
import { getItem, matchRecipe } from "@/game/systems/catalog";
import { iconFor } from "@/game/systems/icons";
import { inventoryAdd, inventoryCanFit, inventoryQuantity, inventoryRemove } from "@/game/systems/inventory";
import { session } from "@/game/systems/session";
import { clearText, drawIconFace, drawIconFrame, fillText, paintTooltip, padRect } from "@/game/ui/chrome";
import { bagSlot, craftSlot, gx, gy, inRect, stats, ICON } from "@/game/ui/layout";
import { lime, uiText, white } from "@/game/ui/theme";
import { DEPTH, type Layers } from "@/game/ui/layers";

export class BagPanel {
  private readonly labels: Phaser.GameObjects.Text[] = [];
  private readonly statsText: Phaser.GameObjects.Text;
  private readonly tipName: Phaser.GameObjects.Text;
  private readonly tipDesc: Phaser.GameObjects.Text;

  constructor(
    scene: Phaser.Scene,
    layers: Layers,
    private readonly gfx: Phaser.GameObjects.Graphics,
    private readonly tipGfx: Phaser.GameObjects.Graphics,
  ) {
    const tiny = { ...uiText, fontSize: `${gy(8)}px`, color: lime };
    this.statsText = layers.addUi(scene.add.text(0, 0, "", tiny).setDepth(DEPTH.guiText));
    for (let i = 0; i < session.player.inventorySize + 4; i++) {
      this.labels.push(
        layers.addUi(
          scene.add
            .text(0, 0, "", { ...uiText, fontSize: `${gy(7)}px`, color: "#1a1208" })
            .setOrigin(0.5)
            .setDepth(DEPTH.guiText),
        ),
      );
    }
    this.tipName = layers.addUi(scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(8)}px` }).setDepth(DEPTH.tipText));
    this.tipDesc = layers.addUi(scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(8)}px`, color: "#e8d080" }).setDepth(DEPTH.tipText));
  }

  hide(): void {
    clearText(this.statsText);
    clearText(this.tipName);
    clearText(this.tipDesc);
    for (const label of this.labels) label.setVisible(false);
  }

  draw(px: number, py: number): void {
    fillText(
      this.statsText,
      padRect(stats, gx(2), gy(6)),
      `Strength  ${session.player.strength}   ·   Spirit  ${session.player.spirit}`,
    );
    this.statsText.setColor(lime);

    session.player.inventory.forEach((slot, i) => {
      const r = bagSlot(i);
      drawIconFrame(this.gfx, r.x, r.y, ICON, inRect(px, py, r), false);
      const label = this.labels[i];
      label.setVisible(true).setPosition(r.x + ICON / 2, r.y + ICON / 2);
      if (slot.id) {
        drawIconFace(this.gfx, r.x, r.y, "item", slot.id);
        label.setText(slot.qty > 1 ? String(slot.qty) : iconFor("item", slot.id).glyph);
        if (inRect(px, py, r) && !session.held) {
          paintTooltip(this.tipGfx, this.tipName, this.tipDesc, px, py, getItem(slot.id).name, getItem(slot.id).description, white, SCREEN_W);
        }
      } else label.setText("");
    });

    if (!session.flag("near_bench")) return;
    for (let i = 0; i < 3; i++) {
      const r = craftSlot(i);
      const id = session.craft[i];
      drawIconFrame(this.gfx, r.x, r.y, ICON, inRect(px, py, r), false);
      const label = this.labels[session.player.inventorySize + i];
      label.setVisible(true).setPosition(r.x + ICON / 2, r.y + ICON / 2);
      if (id) {
        drawIconFace(this.gfx, r.x, r.y, "item", id);
        label.setText(iconFor("item", id).glyph);
        if (inRect(px, py, r) && !session.held) {
          paintTooltip(this.tipGfx, this.tipName, this.tipDesc, px, py, getItem(id).name, getItem(id).description, white, SCREEN_W);
        }
      } else label.setText("");
    }
    const recipe = matchRecipe(session.craft.filter((x): x is string => Boolean(x)));
    const out = craftSlot(3);
    drawIconFrame(this.gfx, out.x, out.y, ICON, inRect(px, py, out), false);
    const outLabel = this.labels[session.player.inventorySize + 3];
    outLabel.setVisible(true).setPosition(out.x + ICON / 2, out.y + ICON / 2);
    if (recipe) {
      drawIconFace(this.gfx, out.x, out.y, "item", recipe.output);
      outLabel.setText(iconFor("item", recipe.output).glyph);
    } else outLabel.setText("");
  }

  slotAt(x: number, y: number): number {
    for (let i = 0; i < session.player.inventorySize; i++) {
      if (inRect(x, y, bagSlot(i))) return i;
    }
    return -1;
  }

  craftAt(x: number, y: number): number {
    if (!session.flag("near_bench")) return -1;
    for (let i = 0; i < 3; i++) {
      if (inRect(x, y, craftSlot(i))) return i;
    }
    return -1;
  }

  onOutput(x: number, y: number): boolean {
    return session.flag("near_bench") && inRect(x, y, craftSlot(3));
  }

  takeCraft(): string {
    if (!session.flag("near_bench")) return "Need a bench";
    const recipe = matchRecipe(session.craft.filter((x): x is string => Boolean(x)));
    if (!recipe) return "No recipe";
    for (const id of recipe.inputs) {
      if (inventoryQuantity(session.player, id) < 1) return "Missing ingredients";
    }
    if (!inventoryCanFit(session.player, recipe.output, recipe.qty)) return "Inventory full";
    for (const id of recipe.inputs) inventoryRemove(session.player, id, 1);
    inventoryAdd(session.player, recipe.output, recipe.qty);
    session.craft = [null, null, null];
    session.quests.syncAcquire(session.player);
    return `Crafted ${getItem(recipe.output).name}`;
  }
}
