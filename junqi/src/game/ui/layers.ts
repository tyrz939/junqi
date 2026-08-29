import Phaser from "phaser";
import { SCREEN_H, SCREEN_W, WORLD_ZOOM } from "@/game/constants";

/** UI camera draw order. Tooltips and the held ghost sit above the bag so hover copy is never under glyphs or HP. */
export const DEPTH = {
  toast: 210,
  bar: 220,
  barGlyph: 223,
  hud: 230,
  hudText: 231,
  float: 232,
  guiDim: 300,
  gui: 302,
  guiText: 304,
  guiTab: 305,
  tip: 360,
  tipText: 361,
  held: 370,
  heldGlyph: 371,
  pause: 380,
  pauseText: 381,
  talk: 400,
  talkText: 401,
  fade: 500,
} as const;

export class Layers {
  readonly ui: Phaser.GameObjects.Container;
  readonly worldCam: Phaser.Cameras.Scene2D.Camera;
  readonly uiCam: Phaser.Cameras.Scene2D.Camera;

  constructor(scene: Phaser.Scene) {
    const leftover = scene.cameras.getCamera("ui");
    if (leftover) scene.cameras.remove(leftover, true);

    this.ui = scene.add.container(0, 0).setDepth(1000);

    this.worldCam = scene.cameras.main;
    this.worldCam.setZoom(WORLD_ZOOM);
    this.worldCam.setRoundPixels(true);
    this.worldCam.setBackgroundColor("#141810");

    this.uiCam = scene.cameras.add(0, 0, SCREEN_W, SCREEN_H, false, "ui");
    this.uiCam.setScroll(0, 0);
    this.uiCam.setZoom(1);
    this.uiCam.setRoundPixels(false);
    this.uiCam.setBackgroundColor("rgba(0,0,0,0)");
    this.uiCam.transparent = true;

    this.worldCam.ignore(this.ui);
  }

  addWorld<T extends Phaser.GameObjects.GameObject>(obj: T): T {
    this.uiCam.ignore(obj);
    return obj;
  }

  addUi<T extends Phaser.GameObjects.GameObject>(obj: T): T {
    this.ui.add(obj);
    this.worldCam.ignore(obj);
    return obj;
  }

  worldToScreen(x: number, y: number): { x: number; y: number } {
    const view = this.worldCam.worldView;
    return {
      x: (x - view.x) * this.worldCam.zoom,
      y: (y - view.y) * this.worldCam.zoom,
    };
  }
}
