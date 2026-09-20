import Phaser from "phaser";
import { session } from "@/game/systems/session";
import { ScrollPane, clearText, drawScrollThumb, drawWindow, fillText, padRect } from "@/game/ui/chrome";
import { gy, inRect, panel, questDetail, questList, questRowH } from "@/game/ui/layout";
import { ink, uiText, white } from "@/game/ui/theme";
import { DEPTH, type Layers } from "@/game/ui/layers";

export class QuestLogView {
  private readonly rows: Phaser.GameObjects.Text[] = [];
  private readonly empty: Phaser.GameObjects.Text;
  private readonly detail: ScrollPane;
  listScroll = 0;
  bodyScroll = 0;

  constructor(
    scene: Phaser.Scene,
    layers: Layers,
    private readonly gfx: Phaser.GameObjects.Graphics,
  ) {
    const tiny = { ...uiText, fontSize: `${gy(8)}px`, color: ink };
    this.empty = layers.addUi(scene.add.text(0, 0, "", tiny).setDepth(DEPTH.guiText));
    for (let i = 0; i < 18; i++) {
      this.rows.push(layers.addUi(scene.add.text(0, 0, "", tiny).setDepth(DEPTH.guiText)));
    }
    this.detail = new ScrollPane(scene, layers);
  }

  hide(): void {
    clearText(this.empty);
    for (const row of this.rows) clearText(row);
    this.detail.hide();
  }

  draw(): void {
    this.gfx.fillStyle(0x000000, 0.08);
    this.gfx.fillRect(questList.x, questList.y, questList.w, questList.h);

    if (session.quests.active.length === 0) {
      fillText(this.empty, questDetail, "No active quests");
      this.empty.setColor(white);
      this.detail.hide();
      return;
    }
    clearText(this.empty);

    const rowH = questRowH;
    const visible = Math.min(this.rows.length, Math.max(1, Math.floor(questList.h / rowH)));
    const maxStart = Math.max(0, session.quests.active.length - visible);
    const start = Phaser.Math.Clamp(Math.floor(this.listScroll / rowH), 0, maxStart);
    this.listScroll = start * rowH;
    for (let i = 0; i < this.rows.length; i++) {
      const qi = start + i;
      if (i >= visible || qi >= session.quests.active.length) {
        clearText(this.rows[i]);
        continue;
      }
      const run = session.quests.active[qi];
      const def = session.quests.def(run.id);
      const y = questList.y + i * rowH;
      const rowBox = { x: questList.x, y, w: questList.w - 6, h: rowH - 2 };
      drawWindow(this.gfx, rowBox.x, rowBox.y, rowBox.w, rowBox.h, qi === session.quests.selection ? 1 : 2);
      fillText(this.rows[i], padRect(rowBox, 8, 4), def.name + (session.quests.met(run.id) ? "  (ready)" : ""));
      this.rows[i].setColor(session.quests.met(run.id) ? white : ink);
    }
    drawScrollThumb(this.gfx, questList, session.quests.active.length * rowH, this.listScroll);

    const run = session.quests.active[session.quests.selection];
    if (!run) {
      this.detail.hide();
      return;
    }
    const def = session.quests.def(run.id);
    const reqs = def.requirements.map((req, i) => `${req.text}  ${run.progress[i]}/${req.qty}`).join("\n");
    const body = `${def.name}\n\n${def.description}\n\n${reqs}${def.completion ? `\n\n${def.completion}` : ""}`;
    const scrolled = this.detail.show(questDetail, body, this.bodyScroll);
    this.bodyScroll = scrolled.scrollY;
    this.detail.text.setColor(ink);
    drawScrollThumb(this.gfx, questDetail, scrolled.contentH, this.bodyScroll);
  }

  click(x: number, y: number): void {
    if (!inRect(x, y, questList)) return;
    const rowH = questRowH;
    const visible = Math.min(this.rows.length, Math.max(1, Math.floor(questList.h / rowH)));
    const maxStart = Math.max(0, session.quests.active.length - visible);
    const start = Phaser.Math.Clamp(Math.floor(this.listScroll / rowH), 0, maxStart);
    const i = start + Math.floor((y - questList.y) / rowH);
    if (i >= 0 && i < session.quests.active.length) session.quests.selection = i;
  }

  wheel(delta: number, px: number, py: number): void {
    const step = delta > 0 ? gy(12) : -gy(12);
    if (inRect(px, py, questList)) this.listScroll = Math.max(0, this.listScroll + step);
    else if (inRect(px, py, questDetail) || inRect(px, py, panel)) this.bodyScroll = Math.max(0, this.bodyScroll + step);
  }
}
