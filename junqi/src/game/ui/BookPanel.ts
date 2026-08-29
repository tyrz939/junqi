import Phaser from "phaser";
import { SCREEN_W } from "@/game/constants";
import { getSpell } from "@/game/systems/catalog";
import { session } from "@/game/systems/session";
import { clearText, drawIconFace, drawIconFrame, drawWindow, fillText, paintTooltip, padRect } from "@/game/ui/chrome";
import { BOOK_CARDS, ICON, bookCard, gy, inRect, pager, pagerNext, pagerPrev } from "@/game/ui/layout";
import { fuchsia, ink, uiText, white } from "@/game/ui/theme";
import { DEPTH, type Layers } from "@/game/ui/layers";

export class BookPanel {
  private readonly rows: Phaser.GameObjects.Text[] = [];
  private readonly pageText: Phaser.GameObjects.Text;
  private readonly prevText: Phaser.GameObjects.Text;
  private readonly nextText: Phaser.GameObjects.Text;
  private readonly tipName: Phaser.GameObjects.Text;
  private readonly tipDesc: Phaser.GameObjects.Text;

  constructor(
    scene: Phaser.Scene,
    layers: Layers,
    private readonly gfx: Phaser.GameObjects.Graphics,
    private readonly tipGfx: Phaser.GameObjects.Graphics,
  ) {
    const tiny = { ...uiText, fontSize: `${gy(8)}px`, color: ink, lineSpacing: 3 };
    for (let i = 0; i < BOOK_CARDS; i++) {
      this.rows.push(layers.addUi(scene.add.text(0, 0, "", tiny).setDepth(DEPTH.guiText)));
    }
    this.pageText = layers.addUi(scene.add.text(0, 0, "", tiny).setDepth(DEPTH.guiText));
    this.prevText = layers.addUi(scene.add.text(0, 0, "", tiny).setDepth(DEPTH.guiText));
    this.nextText = layers.addUi(scene.add.text(0, 0, "", tiny).setDepth(DEPTH.guiText));
    this.tipName = layers.addUi(scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(8)}px` }).setDepth(DEPTH.tipText));
    this.tipDesc = layers.addUi(scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(8)}px`, color: "#e8d080" }).setDepth(DEPTH.tipText));
  }

  hide(): void {
    for (const row of this.rows) clearText(row);
    clearText(this.pageText);
    clearText(this.prevText);
    clearText(this.nextText);
    clearText(this.tipName);
    clearText(this.tipDesc);
  }

  draw(px: number, py: number): void {
    const page = session.spellbookPage;
    const book = session.player.spellbook;
    for (let i = 0; i < BOOK_CARDS; i++) {
      const card = bookCard(i);
      const id = book[i + page * BOOK_CARDS];
      drawWindow(this.gfx, card.x, card.y, card.w, card.h, 2);
      if (!id) {
        clearText(this.rows[i]);
        continue;
      }
      const s = getSpell(id);
      drawIconFrame(this.gfx, card.x + 4, card.y + 4, ICON, inRect(px, py, card), false);
      drawIconFace(this.gfx, card.x + 4, card.y + 4, "spell", id);
      const cd = session.player.cooldowns[id] ?? 0;
      const cost = s.mpCost > 0 ? `${s.mpCost} MP` : "NO COST";
      fillText(
        this.rows[i],
        padRect({ x: card.x + ICON + 8, y: card.y, w: card.w - ICON - 12, h: card.h }, 4, 4),
        `${s.name}\n${cd > 0 ? `${cd.toFixed(1)} SEC` : `${s.cooldown} SEC`}   ${cost}   ${Math.max(s.range, 1)} M\n${s.description}`,
      );
      this.rows[i].setColor(ink);
      if (inRect(px, py, card) && !session.held) {
        paintTooltip(this.tipGfx, this.tipName, this.tipDesc, px, py, s.name, s.description, fuchsia, SCREEN_W);
      }
    }
    drawWindow(this.gfx, pagerPrev.x, pagerPrev.y, pagerPrev.w, pagerPrev.h, 2);
    drawWindow(this.gfx, pagerNext.x, pagerNext.y, pagerNext.w, pagerNext.h, 2);
    fillText(this.prevText, padRect(pagerPrev, 14, 4), "<");
    this.prevText.setColor(white);
    fillText(this.nextText, padRect(pagerNext, 14, 4), ">");
    this.nextText.setColor(white);
    fillText(this.pageText, { x: pagerPrev.x + pagerPrev.w, y: pager.y, w: pagerNext.x - pagerPrev.x - pagerPrev.w, h: pager.h }, String(page + 1));
    this.pageText.setColor(white);
  }

  press(x: number, y: number): { page?: number; spell?: number } | null {
    if (inRect(x, y, pagerPrev) || (inRect(x, y, pager) && x < pager.x + pager.w / 2)) return { page: -1 };
    if (inRect(x, y, pagerNext) || inRect(x, y, pager)) return { page: 1 };
    for (let i = 0; i < BOOK_CARDS; i++) {
      if (inRect(x, y, bookCard(i))) return { spell: i + session.spellbookPage * BOOK_CARDS };
    }
    return null;
  }

  turn(dir: number): void {
    const max = Math.max(0, Math.floor((session.player.spellbook.length - 1) / BOOK_CARDS));
    session.spellbookPage = Phaser.Math.Clamp(session.spellbookPage + dir, 0, max);
  }
}
