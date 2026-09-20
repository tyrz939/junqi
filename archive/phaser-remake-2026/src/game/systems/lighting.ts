import Phaser from "phaser";
import { SCREEN_H, SCREEN_W } from "@/game/constants";
import type { Layers } from "@/game/ui/layers";

export function nightAlpha(time: number): number {
  if (time >= 7 && time < 17) return 0.06;
  if (time >= 17 && time < 20) return 0.2 + ((time - 17) / 3) * 0.22;
  if (time >= 5 && time < 7) return 0.44 - ((time - 5) / 2) * 0.38;
  return 0.52;
}

export class Lighting {
  private readonly wash: Phaser.GameObjects.Rectangle;
  private readonly glows: Phaser.GameObjects.Arc[] = [];
  private readonly layers: Layers;
  private readonly scene: Phaser.Scene;

  constructor(scene: Phaser.Scene, layers: Layers) {
    this.scene = scene;
    this.layers = layers;
    const vig = layers.addUi(scene.add.graphics().setDepth(74));
    for (let i = 0; i < 14; i++) {
      const a = 0.03;
      vig.fillStyle(0x06040a, a);
      vig.fillRect(0, 0, SCREEN_W, 10 + i * 7);
      vig.fillRect(0, SCREEN_H - 10 - i * 7, SCREEN_W, 10 + i * 7);
      vig.fillRect(0, 0, 12 + i * 9, SCREEN_H);
      vig.fillRect(SCREEN_W - 12 - i * 9, 0, 12 + i * 9, SCREEN_H);
    }
    this.wash = layers.addUi(
      scene.add.rectangle(0, 0, SCREEN_W, SCREEN_H, 0x120c1a, 0.42).setOrigin(0).setDepth(80),
    );
  }

  draw(lights: { x: number; y: number; radius: number }[], time: number): void {
    this.wash.setAlpha(nightAlpha(time));
    const pulse = 0.22 + Math.sin(this.scene.time.now / 380) * 0.08;
    while (this.glows.length < lights.length) {
      this.glows.push(
        this.layers.addWorld(
          this.scene.add.circle(0, 0, 32, 0xffc86a, 0.28).setBlendMode(Phaser.BlendModes.ADD).setDepth(1),
        ),
      );
    }
    this.glows.forEach((glow, i) => {
      const light = lights[i];
      glow.setVisible(Boolean(light));
      if (!light) return;
      glow.setPosition(light.x, light.y);
      glow.setRadius(light.radius * (0.94 + pulse * 0.35));
      glow.setAlpha(pulse);
    });
  }
}
