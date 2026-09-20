import Phaser from "phaser";
import { GCD_SECONDS, SCREEN_H, SCREEN_W } from "@/game/constants";
import type { BeatTracker } from "@/game/systems/beats";
import { session } from "@/game/systems/session";
import { println } from "@/game/systems/termLog";
import { drawEnergy, drawHealthStack, fillText, clearText } from "@/game/ui/chrome";
import {
  clockX,
  clockY,
  energyY,
  gx,
  gy,
  hpH,
  hpW,
  hpX,
  hpY,
  targetX,
  trackW,
  trackX,
  trackY,
  trackH,
} from "@/game/ui/layout";
import {
  cream,
  energyEmpty,
  energyOk,
  fuchsia,
  hungerFill,
  hungerWarn,
  muted,
  uiText,
  warmthFill,
  warmthWarn,
  white,
} from "@/game/ui/theme";
import { DEPTH, type Layers } from "@/game/ui/layers";
import type { Unit } from "@/game/entities/Unit";

export class Hud {
  private readonly stack: { text: string; life: number }[] = [];
  private readonly gfx: Phaser.GameObjects.Graphics;
  private readonly hpText: Phaser.GameObjects.Text;
  private readonly mpText: Phaser.GameObjects.Text;
  private readonly tgtHp: Phaser.GameObjects.Text;
  private readonly tgtMp: Phaser.GameObjects.Text;
  private readonly youName: Phaser.GameObjects.Text;
  private readonly tgtName: Phaser.GameObjects.Text;
  private readonly toastText: Phaser.GameObjects.Text;
  private readonly debugText: Phaser.GameObjects.Text;
  private readonly surviveMarks: Phaser.GameObjects.Text[] = [];
  private readonly clockText: Phaser.GameObjects.Text;
  private readonly trackText: Phaser.GameObjects.Text;
  private readonly usePrompt: Phaser.GameObjects.Text;
  private readonly veil: Phaser.GameObjects.Rectangle;
  private readonly veilTitle: Phaser.GameObjects.Text;
  private readonly veilHint: Phaser.GameObjects.Text;
  private readonly fade: Phaser.GameObjects.Rectangle;
  private readonly layers: Layers;
  private readonly scene: Phaser.Scene;
  private useAt: { x: number; y: number } | null = null;
  private fading = false;

  constructor(scene: Phaser.Scene, layers: Layers) {
    this.scene = scene;
    this.layers = layers;
    this.gfx = layers.addUi(scene.add.graphics().setDepth(DEPTH.hud));
    const tiny = { ...uiText, fontSize: `${gy(8)}px`, color: white };
    this.hpText = layers.addUi(scene.add.text(hpX + hpW - gx(4), hpY + hpH * 0.25, "", tiny).setOrigin(1, 0.5).setDepth(DEPTH.hudText));
    this.mpText = layers.addUi(scene.add.text(hpX + hpW - gx(4), hpY + hpH * 0.75, "", tiny).setOrigin(1, 0.5).setDepth(DEPTH.hudText));
    this.tgtHp = layers.addUi(scene.add.text(targetX + hpW - gx(4), hpY + hpH * 0.25, "", tiny).setOrigin(1, 0.5).setDepth(DEPTH.hudText));
    this.tgtMp = layers.addUi(scene.add.text(targetX + hpW - gx(4), hpY + hpH * 0.75, "", tiny).setOrigin(1, 0.5).setDepth(DEPTH.hudText));
    this.youName = layers.addUi(
      scene.add.text(hpX, energyY + gy(16), session.player.name, { ...uiText, fontSize: `${gy(8)}px`, color: muted }).setDepth(DEPTH.hudText),
    );
    this.tgtName = layers.addUi(
      scene.add.text(targetX, energyY + gy(6), "", { ...uiText, fontSize: `${gy(8)}px`, color: muted }).setDepth(DEPTH.hudText),
    );
    const mark = { ...uiText, fontSize: `${gy(6)}px`, color: muted };
    this.surviveMarks.push(
      layers.addUi(scene.add.text(hpX + gx(98), energyY - 1, "E", mark).setDepth(DEPTH.hudText)),
      layers.addUi(scene.add.text(hpX + gx(98), energyY + gy(5), "H", mark).setDepth(DEPTH.hudText)),
      layers.addUi(scene.add.text(hpX + gx(98), energyY + gy(9), "W", mark).setDepth(DEPTH.hudText)),
    );
    this.clockText = layers.addUi(
      scene.add.text(clockX, clockY, "", { ...uiText, fontSize: `${gy(8)}px`, color: cream }).setOrigin(1, 0).setDepth(DEPTH.hudText),
    );
    this.trackText = layers.addUi(
      scene.add
        .text(0, 0, "", {
          ...uiText,
          fontSize: `${gy(8)}px`,
          color: muted,
          align: "right",
        })
        .setDepth(DEPTH.hudText),
    );
    this.usePrompt = layers.addUi(
      scene.add
        .text(0, 0, "E", { ...uiText, fontSize: `${gy(9)}px`, color: cream })
        .setOrigin(0.5, 1)
        .setDepth(DEPTH.hudText)
        .setVisible(false),
    );
    this.toastText = layers.addUi(
      scene.add
        .text(SCREEN_W / 2, SCREEN_H - gy(48), "", {
          ...uiText,
          fontSize: `${gy(10)}px`,
          color: fuchsia,
          align: "center",
        })
        .setOrigin(0.5, 1)
        .setDepth(DEPTH.toast),
    );
    this.debugText = layers.addUi(
      scene.add.text(hpX, gy(42), "", { ...uiText, fontSize: "12px", color: "#8a9a70" }).setDepth(DEPTH.hudText),
    );
    this.veil = layers.addUi(
      scene.add.rectangle(0, 0, SCREEN_W, SCREEN_H, 0x0a0806, 0.48).setOrigin(0).setDepth(DEPTH.pause).setVisible(false),
    );
    this.veilTitle = layers.addUi(
      scene.add
        .text(SCREEN_W / 2, SCREEN_H / 2 - gy(10), "", { ...uiText, fontSize: `${gy(16)}px`, color: cream })
        .setOrigin(0.5)
        .setDepth(DEPTH.pauseText)
        .setVisible(false),
    );
    this.veilHint = layers.addUi(
      scene.add
        .text(SCREEN_W / 2, SCREEN_H / 2 + gy(10), "", {
          ...uiText,
          fontSize: `${gy(9)}px`,
          color: muted,
          align: "center",
        })
        .setOrigin(0.5)
        .setDepth(DEPTH.pauseText)
        .setVisible(false),
    );
    this.fade = layers.addUi(scene.add.rectangle(0, 0, SCREEN_W, SCREEN_H, 0x000000, 1).setOrigin(0).setDepth(DEPTH.fade));
  }

  toast(message: string): void {
    this.stack.unshift({ text: message, life: 3 });
    if (this.stack.length > 3) this.stack.length = 3;
    println(message);
  }

  float(worldX: number, worldY: number, text: string, crit: boolean): void {
    const pos = this.layers.worldToScreen(worldX, worldY);
    const label = this.layers.addUi(
      this.scene.add
        .text(pos.x, pos.y - 18, text, {
          ...uiText,
          fontSize: crit ? "18px" : "14px",
          color: crit ? "#fff4a8" : "#ffe07a",
        })
        .setOrigin(0.5)
        .setDepth(DEPTH.float),
    );
    this.scene.tweens.add({
      targets: label,
      y: pos.y - 48,
      alpha: 0,
      duration: 700,
      onComplete: () => label.destroy(),
    });
  }

  setUse(at: { x: number; y: number } | null): void {
    this.useAt = at;
  }

  fadeArrive(): void {
    this.scene.tweens.killTweensOf(this.fade);
    this.fade.setAlpha(1);
    this.scene.tweens.add({ targets: this.fade, alpha: 0, duration: 180, ease: "Quad.easeOut" });
  }

  fadeDepart(done: () => void): boolean {
    if (this.fading) return false;
    this.fading = true;
    this.scene.tweens.killTweensOf(this.fade);
    this.scene.tweens.add({
      targets: this.fade,
      alpha: 1,
      duration: 120,
      ease: "Quad.easeIn",
      onComplete: done,
    });
    return true;
  }

  update(dt: number, player: Unit, beats: BeatTracker, seed: number, target: Unit | null, extra = ""): void {
    if (session.toast) {
      this.toast(session.toast);
      session.toast = "";
    }
    for (const row of this.stack) row.life -= dt;
    while (this.stack.length && this.stack[this.stack.length - 1].life <= 0) this.stack.pop();

    this.gfx.clear();
    drawHealthStack(this.gfx, hpX, hpY, hpW, hpH, player.hp / player.maxhp, player.mp / player.maxmp, true);
    this.hpText.setText(`${Math.round(player.hp)}/${player.maxhp}`);
    this.mpText.setText(`${Math.round(player.mp)}/${player.maxmp}`);
    drawEnergy(this.gfx, hpX, energyY, gx(96), gy(4), player.energy / player.maxenergy, player.energyEmpty, energyOk, energyEmpty);
    drawEnergy(this.gfx, hpX, energyY + gy(6), gx(96), gy(3), player.hunger / 100, player.hunger < 18, hungerFill, hungerWarn);
    drawEnergy(this.gfx, hpX, energyY + gy(10), gx(96), gy(3), player.warmth / 100, player.warmth < 22, warmthFill, warmthWarn);
    this.hpText.setVisible(true);
    this.mpText.setVisible(true);
    this.youName.setVisible(true);
    for (const mark of this.surviveMarks) mark.setVisible(true);

    if (target?.alive) {
      const bot = target.class === "mage" ? target.mp / target.maxmp : target.energy / target.maxenergy;
      drawHealthStack(this.gfx, targetX, hpY, hpW, hpH, target.hp / target.maxhp, bot, target.class === "mage");
      this.tgtHp.setText(`${Math.round(target.hp)}/${target.maxhp}`);
      this.tgtMp.setText(
        target.class === "mage"
          ? `${Math.round(target.mp)}/${target.maxmp}`
          : `${Math.round(target.energy)}/${target.maxenergy}`,
      );
      this.tgtName.setVisible(true).setText(target.name);
    } else {
      this.tgtHp.setText("");
      this.tgtMp.setText("");
      this.tgtName.setVisible(false);
    }

    this.clockText.setText(formatClock(session.time));
    this.clockText.setVisible(!session.guiOpen);
    if (!session.guiOpen) this.drawClockPip();
    const track = session.guiOpen ? "" : trackerLine();
    if (track) fillText(this.trackText, { x: trackX - trackW, y: trackY, w: trackW, h: trackH }, track);
    else clearText(this.trackText);

    const showUse = Boolean(this.useAt) && !session.guiOpen && !session.paused && player.alive;
    this.usePrompt.setVisible(showUse);
    if (showUse && this.useAt) this.usePrompt.setPosition(this.useAt.x, this.useAt.y);

    this.paintVeil(player.alive);
    this.toastText.setText(this.stack.map((s) => s.text).join("\n"));
    this.debugText.setVisible(session.debug);
    this.debugText.setText(
      session.debug
        ? `${extra}\n${beats.debugLine()}  seed ${seed.toString(16)}  gcd ${player.gcd.toFixed(1)}/${GCD_SECONDS}`
        : "",
    );
  }

  private drawClockPip(): void {
    const day = session.time >= 6 && session.time < 18;
    const cx = clockX - this.clockText.width - gx(8);
    const cy = clockY + gy(5);
    this.gfx.fillStyle(day ? 0xe8c868 : 0x7a9ad0, 1);
    this.gfx.fillCircle(cx, cy, gy(3));
  }

  private paintVeil(alive: boolean): void {
    const death = !alive;
    const paused = session.paused && !session.guiOpen && alive;
    const on = death || paused;
    this.veil.setVisible(on);
    this.veilTitle.setVisible(on);
    this.veilHint.setVisible(on);
    if (!on) return;
    if (death) {
      this.veilTitle.setText("You died");
      this.veilHint.setText("E stand up\nF5 was save");
    } else {
      this.veilTitle.setText("PAUSED");
      this.veilHint.setText("Esc");
    }
  }
}

function formatClock(time: number): string {
  const h24 = Math.floor(((time % 24) + 24) % 24);
  const m = Math.floor((time % 1) * 60);
  const ap = h24 >= 12 ? "pm" : "am";
  const h = h24 % 12 || 12;
  return `${h}:${String(m).padStart(2, "0")} ${ap}`;
}

function trackerLine(): string {
  const log = session.quests;
  if (log.active.length === 0) return "";
  const run = log.active[log.selection] ?? log.active[0];
  if (!run) return "";
  const def = log.def(run.id);
  if (log.met(run.id)) return `${def.name}  (ready)`;
  const i = def.requirements.findIndex((req, n) => run.progress[n] < req.qty);
  const req = def.requirements[i] ?? def.requirements[0];
  if (!req) return def.name;
  const prog = run.progress[i >= 0 ? i : 0] ?? 0;
  return `${def.name}\n${req.text}  ${prog}/${req.qty}`;
}
