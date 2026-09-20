import Phaser from "phaser";
import { SCREEN_W, SCREEN_H } from "@/game/constants";
import { getItem } from "@/game/systems/catalog";
import { inventorySwap, tryUseItem } from "@/game/systems/inventory";
import { session } from "@/game/systems/session";
import { BagPanel } from "@/game/ui/BagPanel";
import { BookPanel } from "@/game/ui/BookPanel";
import { QuestLogView } from "@/game/ui/QuestLog";
import { MiniMap } from "@/game/ui/MiniMap";
import { Fog } from "@/game/ui/fog";
import { DRAG2, destroyBagSlot, unbindBar, type HeldFrom } from "@/game/ui/drag";
import { clearText, drawWindow, fillText, padRect } from "@/game/ui/chrome";
import {
  TAB_IDS,
  TAB_LABELS,
  barTray,
  gy,
  inRect,
  panel,
  panelTabs,
} from "@/game/ui/layout";
import { ink, muted, uiText } from "@/game/ui/theme";
import { DEPTH, type Layers } from "@/game/ui/layers";
import type { Grid } from "@/game/world/Grid";
import type { GuiWindow } from "@/game/types";

export class Gui {
  readonly fog = new Fog();
  private readonly dimmer: Phaser.GameObjects.Rectangle;
  private readonly gfx: Phaser.GameObjects.Graphics;
  private readonly tipGfx: Phaser.GameObjects.Graphics;
  private readonly tabs: Phaser.GameObjects.Text[] = [];
  private readonly bag: BagPanel;
  private readonly book: BookPanel;
  private readonly quests: QuestLogView;
  private readonly map: MiniMap;
  private tabHit = panelTabs;
  private pressBag: { index: number; x: number; y: number } | null = null;
  private pressBook: { index: number; x: number; y: number } | null = null;
  private dragging = false;
  private heldFrom: HeldFrom | null = null;
  private lastWindow: GuiWindow | null = null;
  private grid: Grid | null = null;
  private playerX = 0;
  private playerY = 0;

  constructor(scene: Phaser.Scene, layers: Layers) {
    this.dimmer = layers.addUi(
      scene.add.rectangle(0, 0, SCREEN_W, SCREEN_H, 0x0a0806, 0.32).setOrigin(0).setDepth(DEPTH.guiDim),
    );
    this.gfx = layers.addUi(scene.add.graphics().setDepth(DEPTH.gui));
    this.tipGfx = layers.addUi(scene.add.graphics().setDepth(DEPTH.tip));
    this.bag = new BagPanel(scene, layers, this.gfx, this.tipGfx);
    this.book = new BookPanel(scene, layers, this.gfx, this.tipGfx);
    this.quests = new QuestLogView(scene, layers, this.gfx);
    this.map = new MiniMap(scene, layers);
    for (let i = 0; i < TAB_IDS.length; i++) {
      this.tabs.push(
        layers.addUi(scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(7)}px`, color: ink }).setDepth(DEPTH.guiTab)),
      );
    }
  }

  update(px = 0, py = 0, grid?: Grid, playerX = 0, playerY = 0): void {
    if (grid) {
      this.grid = grid;
      this.playerX = playerX;
      this.playerY = playerY;
      const cell = grid.worldToCell(playerX, playerY);
      this.fog.tick(grid, cell.cx, cell.cy);
    }
    const open = session.guiOpen;
    this.dimmer.setVisible(open);
    this.gfx.setVisible(open);
    this.tipGfx.setVisible(open);
    if (!open) {
      this.gfx.clear();
      this.tipGfx.clear();
      this.tabHit = [];
      this.cancelHeld();
      this.hidePanels();
      return;
    }
    if (session.window !== this.lastWindow) {
      this.quests.listScroll = 0;
      this.quests.bodyScroll = 0;
      this.lastWindow = session.window;
    }
    this.tickDrag(px, py);
    this.gfx.clear();
    this.tipGfx.clear();
    this.hidePanels();
    drawWindow(this.gfx, panel.x, panel.y, panel.w, panel.h, 1);
    if (session.window === "inventory") this.bag.draw(px, py);
    else if (session.window === "spellbook") this.book.draw(px, py);
    else if (session.window === "quest") this.quests.draw();
    else if (this.grid) this.map.paint(this.grid, this.fog, this.playerX, this.playerY);
    this.drawTabs();
  }

  press(x: number, y: number, right: boolean): string | null {
    if (!session.guiOpen) return null;
    for (let i = 0; i < this.tabHit.length; i++) {
      if (inRect(x, y, this.tabHit[i])) {
        session.window = TAB_IDS[i];
        return null;
      }
    }
    if (session.window === "inventory") return this.pressBagSlot(x, y, right);
    if (session.window === "spellbook") return this.pressSpell(x, y);
    if (session.window === "quest") {
      this.quests.click(x, y);
      return null;
    }
    return null;
  }

  release(x: number, y: number): string | null {
    const press = this.pressBag;
    const book = this.pressBook;
    const dragged = this.dragging;
    this.pressBag = null;
    this.pressBook = null;
    this.dragging = false;

    if (press && !dragged) {
      const slot = session.player.inventory[press.index];
      if (!slot?.id) return null;
      if (this.bag.slotAt(x, y) !== press.index) return null;
      return tryUseItem(session.player, slot.id);
    }
    if (book && !dragged) return null;
    if (!session.held) {
      this.heldFrom = null;
      return null;
    }

    const dest = this.bag.slotAt(x, y);
    if (dest >= 0) {
      if (this.heldFrom?.kind === "bag") inventorySwap(session.player, this.heldFrom.index, dest);
      this.clearHeld();
      return null;
    }
    const craft = this.bag.craftAt(x, y);
    if (craft >= 0 && session.held.source === "item") {
      session.craft[craft] = session.held.id;
      this.clearHeld();
      return "Set craft input";
    }
    if (this.onChrome(x, y)) {
      this.clearHeld();
      return null;
    }
    return this.dropOutside();
  }

  cancelPress(): void {
    this.pressBag = null;
    this.pressBook = null;
    this.dragging = false;
  }

  cancelHeld(): void {
    this.cancelPress();
    this.heldFrom = null;
    session.held = null;
  }

  isPressing(): boolean {
    return Boolean(this.pressBag || this.pressBook || this.dragging);
  }

  holdFrom(kind: HeldFrom["kind"], index: number): void {
    this.heldFrom = { kind, index };
  }

  droppedOnBar(): void {
    this.cancelPress();
    this.heldFrom = null;
  }

  wheel(delta: number, px: number, py: number): void {
    if (!session.guiOpen) return;
    if (session.window === "quest") {
      this.quests.wheel(delta, px, py);
      return;
    }
    if (session.window === "spellbook" && inRect(px, py, panel)) {
      this.book.turn(delta > 0 ? 1 : -1);
    }
  }

  private hidePanels(): void {
    this.bag.hide();
    this.book.hide();
    this.quests.hide();
    this.map.hide();
    for (const tab of this.tabs) clearText(tab);
  }

  private drawTabs(): void {
    this.tabHit = panelTabs;
    this.tabHit.forEach((tab, i) => {
      const on = session.window === TAB_IDS[i];
      drawWindow(this.gfx, tab.x, tab.y, tab.w, tab.h, on ? 1 : 2);
      fillText(this.tabs[i], padRect(tab, 8, 6), TAB_LABELS[i]);
      this.tabs[i].setColor(on ? ink : muted);
    });
  }

  private tickDrag(px: number, py: number): void {
    if (this.pressBag && !this.dragging) {
      const dx = px - this.pressBag.x;
      const dy = py - this.pressBag.y;
      if (dx * dx + dy * dy > DRAG2) {
        this.dragging = true;
        const slot = session.player.inventory[this.pressBag.index];
        if (slot.id) {
          session.held = { source: "item", id: slot.id };
          this.heldFrom = { kind: "bag", index: this.pressBag.index };
        }
      }
    }
    if (this.pressBook && !this.dragging) {
      const dx = px - this.pressBook.x;
      const dy = py - this.pressBook.y;
      if (dx * dx + dy * dy > DRAG2) {
        this.dragging = true;
        const id = session.player.spellbook[this.pressBook.index];
        if (id) {
          session.held = { source: "spell", id };
          this.heldFrom = { kind: "book", index: this.pressBook.index };
        }
      }
    }
  }

  private pressBagSlot(x: number, y: number, right: boolean): string | null {
    if (session.held) return null;
    const i = this.bag.slotAt(x, y);
    if (i >= 0) {
      const slot = session.player.inventory[i];
      if (!slot.id) return null;
      if (right) {
        session.held = { source: "item", id: slot.id };
        this.heldFrom = { kind: "bag", index: i };
        return `Holding ${getItem(slot.id).name}`;
      }
      this.pressBag = { index: i, x, y };
      this.dragging = false;
      return null;
    }
    if (this.bag.craftAt(x, y) >= 0) return null;
    if (this.bag.onOutput(x, y)) return this.bag.takeCraft();
    return null;
  }

  private pressSpell(x: number, y: number): string | null {
    const hit = this.book.press(x, y);
    if (!hit) return null;
    if (hit.page) {
      this.book.turn(hit.page);
      return null;
    }
    if (session.held || hit.spell == null) return null;
    const id = session.player.spellbook[hit.spell];
    if (!id) return null;
    this.pressBook = { index: hit.spell, x, y };
    this.dragging = false;
    return null;
  }

  private onChrome(x: number, y: number): boolean {
    return inRect(x, y, panel) || inRect(x, y, barTray);
  }

  private dropOutside(): string | null {
    const from = this.heldFrom;
    if (!session.held || !from) {
      this.clearHeld();
      return null;
    }
    if (from.kind === "book") {
      this.clearHeld();
      return null;
    }
    if (from.kind === "bar") {
      const msg = unbindBar(from.index);
      this.clearHeld();
      return msg;
    }
    const msg = destroyBagSlot(from.index);
    this.clearHeld();
    return msg;
  }

  private clearHeld(): void {
    this.heldFrom = null;
    session.held = null;
  }
}
