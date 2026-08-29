import Phaser from "phaser";
import { PATH_CELL, SCREEN_H, SCREEN_W } from "@/game/constants";
import { session } from "@/game/systems/session";
import { cream, dim, frame, muted, paper, uiText } from "@/game/ui/theme";

export class TitleScene extends Phaser.Scene {
  private enter?: Phaser.Input.Keyboard.Key;
  private launching = false;

  constructor() {
    super("title");
  }

  create(): void {
    this.paintCounty();
    this.add.rectangle(0, 0, SCREEN_W, SCREEN_H, 0x0a0810, 0.42).setOrigin(0).setDepth(2);
    this.add.rectangle(0, 0, SCREEN_W, 90, 0x060408, 0.55).setOrigin(0).setDepth(3);
    this.add.rectangle(0, SCREEN_H - 140, SCREEN_W, 140, 0x060408, 0.62).setOrigin(0).setDepth(3);

    this.add.text(88, 56, "JUNQI", {
      fontFamily: "Georgia, 'Times New Roman', serif",
      fontSize: "72px",
      color: cream,
      resolution: 2,
    }).setDepth(10);
    this.add.rectangle(92, 138, 220, 3, 0xc4a24a, 0.9).setOrigin(0).setDepth(10);
    this.add.text(92, 152, "Jane  ·  Sunday train  ·  she does not know yet", {
      ...uiText,
      fontSize: "18px",
      color: muted,
    }).setDepth(10);
    this.add.text(92, 180, "The letter said next week. Castle does not end at the snake.", {
      ...uiText,
      fontSize: "14px",
      color: dim,
    }).setDepth(10);

    this.button(92, 268, "New game", () => this.launchNew());
    this.button(92, 336, session.hasSave() ? "Continue  ·  slot 0" : "Continue  ·  empty", () => this.launchContinue());

    this.add
      .text(92, 430, "WASD move    Shift sprint    RMB walk    LMB target    E use\n1–8 bar    Tab bags    Esc pause    F5 save    ` console", {
        ...uiText,
        fontSize: "15px",
        color: dim,
        lineSpacing: 8,
      })
      .setDepth(10);

    const kb = this.input.keyboard;
    this.enter = kb?.addKey(Phaser.Input.Keyboard.KeyCodes.ENTER, false);
    this.events.once(Phaser.Scenes.Events.SHUTDOWN, () => {
      if (this.enter) kb?.removeKey(Phaser.Input.Keyboard.KeyCodes.ENTER);
    });
    this.fireflies();
  }

  update(): void {
    if (session.wantRestart || session.wantQuit) {
      session.wantRestart = false;
      session.wantQuit = false;
    }
    if (this.enter && Phaser.Input.Keyboard.JustDown(this.enter)) {
      if (session.hasSave()) this.launchContinue();
      else this.launchNew();
    }
  }

  private launchNew(): void {
    if (this.launching) return;
    this.launching = true;
    session.newRun();
    this.scene.start("play");
  }

  private launchContinue(): void {
    if (this.launching) return;
    if (!session.load()) return;
    this.launching = true;
    this.scene.start("play");
  }

  private paintCounty(): void {
    const cols = Math.ceil(SCREEN_W / (PATH_CELL * 8));
    const rows = Math.ceil(SCREEN_H / (PATH_CELL * 8));
    const rt = this.add.renderTexture(0, 0, cols * PATH_CELL, rows * PATH_CELL).setOrigin(0).setScale(8).setDepth(0);
    for (let cy = 0; cy < rows; cy++) {
      for (let cx = 0; cx < cols; cx++) {
        const pond = cx > 12 && cx < 17 && cy > 6 && cy < 10;
        const wall = cx >= 4 && cx <= 8 && cy >= 3 && cy <= 6;
        const key = pond ? "water" : wall ? "house" : (cx + cy) % 2 === 0 ? "grass" : "grass_alt";
        rt.draw(key, cx * PATH_CELL, cy * PATH_CELL);
      }
    }
    rt.draw("door", 6 * PATH_CELL, 6 * PATH_CELL);
    rt.draw("bush", 10 * PATH_CELL, 5 * PATH_CELL);
    rt.draw("bush", 3 * PATH_CELL, 8 * PATH_CELL);
    this.add.image(430, 390, "player_s").setScale(6).setDepth(1);
    this.add.image(510, 400, "dog_e").setScale(6).setDepth(1);
    this.add.image(360, 300, "torch").setScale(5).setDepth(1);
    this.add.image(360, 300, "torch").setScale(10).setAlpha(0.18).setBlendMode(Phaser.BlendModes.ADD).setDepth(1);
  }

  private fireflies(): void {
    for (let i = 0; i < 14; i++) {
      const x = 180 + (i * 79) % 980;
      const y = 90 + (i * 53) % 480;
      const dot = this.add.circle(x, y, 2, 0xffe08a, 0.85).setBlendMode(Phaser.BlendModes.ADD).setDepth(4);
      this.tweens.add({
        targets: dot,
        x: x + 24 - (i % 5) * 10,
        y: y - 18,
        alpha: 0.15,
        duration: 1400 + i * 90,
        yoyo: true,
        repeat: -1,
        ease: "Sine.easeInOut",
      });
    }
  }

  private button(x: number, y: number, label: string, fn: () => void): void {
    const hit = this.add
      .rectangle(x, y, 300, 50, paper, 0.92)
      .setOrigin(0)
      .setStrokeStyle(2, frame)
      .setInteractive({ useHandCursor: true })
      .setDepth(10);
    const text = this.add.text(x + 18, y + 13, label, { ...uiText, fontSize: "20px", color: cream }).setDepth(11);
    hit.on("pointerover", () => {
      hit.setFillStyle(0x2a2214, 0.96);
      hit.setStrokeStyle(2, 0xe8d080);
      text.setColor("#fff4d0");
    });
    hit.on("pointerout", () => {
      hit.setFillStyle(paper, 0.92);
      hit.setStrokeStyle(2, frame);
      text.setColor(cream);
    });
    hit.on("pointerdown", fn);
  }
}
