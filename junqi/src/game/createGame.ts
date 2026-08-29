import Phaser from "phaser";
import { SCREEN_H, SCREEN_W } from "@/game/constants";
import { BootScene } from "@/game/scenes/BootScene";
import { PlayScene } from "@/game/scenes/PlayScene";
import { TitleScene } from "@/game/scenes/TitleScene";

export function createGame(parent: string | HTMLElement): Phaser.Game {
  return new Phaser.Game({
    type: Phaser.AUTO,
    parent,
    width: SCREEN_W,
    height: SCREEN_H,
    backgroundColor: "#0e100c",
    pixelArt: true,
    antialias: false,
    roundPixels: true,
    scene: [BootScene, TitleScene, PlayScene],
    scale: {
      mode: Phaser.Scale.FIT,
      autoCenter: Phaser.Scale.CENTER_BOTH,
    },
    input: { mouse: { preventDefaultWheel: false } },
  });
}
