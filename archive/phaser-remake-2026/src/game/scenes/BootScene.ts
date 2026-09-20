import Phaser from "phaser";
import { PATH_CELL } from "@/game/constants";

const DIRS = ["e", "s", "w", "n"] as const;

export class BootScene extends Phaser.Scene {
  constructor() {
    super("boot");
  }

  create(): void {
    const g = this.add.graphics();
    grass(g, "grass", false);
    grass(g, "grass_alt", true);
    bricks(g, "wall", 0x3d2c1e, 0x5c4432, 0x24180e);
    bricks(g, "house", 0x6e4e34, 0x8d6a48, 0x4a321e);
    planks(g, "wood", 0x7a5a38, 0x9a764c, 0x5a3e24);
    planks(g, "door", 0x2b1c12, 0x4a3220, 0x160e08);
    bushTile(g);
    stoneTile(g);
    boneTile(g);
    waterTile(g);

    playerSheet(g);
    dogSheet(g);
    skeletonSheet(g);
    ratSheet(g);
    snakeSheet(g);
    banditSheet(g);
    plantSheet(g);
    spiderSheet(g);
    blobSheet(g, "bat", 0x3a2a4a, 0x1a1218, 0xc04040);
    blobSheet(g, "wolf", 0x6a6a70, 0x3a3a40, 0xe8e8f0);
    blobSheet(g, "crow", 0x1a1a1c, 0x3a3a40, 0xc0a020);
    blobSheet(g, "slime", 0x44aa66, 0x226633, 0xa0e8b0);

    chestProp(g);
    crateProp(g);
    doorProp(g);
    benchProp(g);
    hatchProp(g);
    orbProp(g);
    iceOrbProp(g);
    shrineProp(g);
    signProp(g);
    leverProp(g);
    campfireProp(g);
    npcSheet(g);
    torchProp(g);
    brokenProp(g);
    dropProp(g);
    reticleProp(g);

    bolt(g, "bolt_frost", 0x9ad4ff, 0x3a6aaa);
    bolt(g, "bolt_fire", 0xff8844, 0xaa3310);
    bolt(g, "bolt_nature", 0x88cc44, 0x3a6a18);
    bolt(g, "bolt_heal", 0xa8e8a8, 0x2a6a3a);
    slash(g);
    spark(g);

    g.destroy();
    this.scene.start("title");
  }
}

function p(g: Phaser.GameObjects.Graphics, x: number, y: number, c: number, a = 1): void {
  g.fillStyle(c, a);
  g.fillRect(x, y, 1, 1);
}

function grass(g: Phaser.GameObjects.Graphics, key: string, alt: boolean): void {
  g.clear();
  const mid = alt ? 0x3f5c28 : 0x48682e;
  const hi = alt ? 0x6a8a40 : 0x74a04c;
  const lo = alt ? 0x2a4018 : 0x324818;
  g.fillStyle(mid, 1);
  g.fillRect(0, 0, 8, 8);
  p(g, 1, 2, hi);
  p(g, 5, 1, hi);
  p(g, 3, 5, lo);
  p(g, 6, 4, hi);
  p(g, 2, 6, lo);
  p(g, 7, 6, alt ? 0x8a6a28 : 0x2f4a1c);
  g.generateTexture(key, PATH_CELL, PATH_CELL);
}

function bricks(g: Phaser.GameObjects.Graphics, key: string, a: number, b: number, mortar: number): void {
  g.clear();
  g.fillStyle(mortar, 1);
  g.fillRect(0, 0, 8, 8);
  g.fillStyle(a, 1);
  g.fillRect(0, 0, 3, 3);
  g.fillRect(4, 0, 4, 3);
  g.fillStyle(b, 1);
  g.fillRect(0, 4, 5, 3);
  g.fillRect(6, 4, 2, 3);
  p(g, 1, 1, b);
  p(g, 5, 5, a);
  g.generateTexture(key, PATH_CELL, PATH_CELL);
}

function planks(g: Phaser.GameObjects.Graphics, key: string, a: number, b: number, line: number): void {
  g.clear();
  g.fillStyle(a, 1);
  g.fillRect(0, 0, 8, 8);
  g.fillStyle(line, 1);
  g.fillRect(0, 2, 8, 1);
  g.fillRect(0, 5, 8, 1);
  p(g, 2, 0, b);
  p(g, 6, 3, b);
  p(g, 1, 6, b);
  g.generateTexture(key, PATH_CELL, PATH_CELL);
}

function bushTile(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  g.fillStyle(0x2a441c, 1);
  g.fillRect(0, 0, 8, 8);
  g.fillStyle(0x3a6a28, 1);
  g.fillRect(1, 1, 6, 6);
  p(g, 2, 2, 0x6a9a40);
  p(g, 5, 3, 0x1e3014);
  p(g, 3, 5, 0x6a9a40);
  g.generateTexture("bush", PATH_CELL, PATH_CELL);
}

function stoneTile(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  g.fillStyle(0x3a3a3c, 1);
  g.fillRect(0, 0, 8, 8);
  g.fillStyle(0x555558, 1);
  g.fillRect(1, 1, 3, 3);
  g.fillRect(4, 4, 3, 3);
  p(g, 6, 1, 0x2a2a2c);
  p(g, 2, 6, 0x6a6a6c);
  g.generateTexture("stone", PATH_CELL, PATH_CELL);
}

function boneTile(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  g.fillStyle(0x5a5448, 1);
  g.fillRect(0, 0, 8, 8);
  g.fillStyle(0x8a8274, 1);
  g.fillRect(0, 3, 8, 2);
  p(g, 2, 1, 0xa09888);
  p(g, 6, 6, 0x3a342c);
  g.generateTexture("bone", PATH_CELL, PATH_CELL);
}

function waterTile(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  g.fillStyle(0x1e3e58, 1);
  g.fillRect(0, 0, 8, 8);
  g.fillStyle(0x3a6a88, 1);
  g.fillRect(0, 2, 5, 1);
  g.fillRect(3, 5, 5, 1);
  p(g, 6, 1, 0x7ab0c8);
  g.generateTexture("water", PATH_CELL, PATH_CELL);
}

function shadow(g: Phaser.GameObjects.Graphics, x: number, y: number, w: number): void {
  g.fillStyle(0x000000, 0.32);
  g.fillRect(x, y, w, 2);
}

function playerSheet(g: Phaser.GameObjects.Graphics): void {
  for (const dir of DIRS) {
    g.clear();
    shadow(g, 3, 13, 10);
    const hx = dir === "e" ? 7 : dir === "w" ? 4 : 6;
    g.fillStyle(0x2a322c, 1);
    g.fillRect(5, 7, 6, 5);
    g.fillStyle(0x4a5a52, 1);
    g.fillRect(6, 7, 4, 4);
    g.fillStyle(0x3a2a1c, 1);
    g.fillRect(dir === "e" ? 8 : 5, 11, 2, 3);
    g.fillRect(dir === "w" ? 6 : 9, 11, 2, 3);
    g.fillStyle(0xe0c8a0, 1);
    g.fillRect(hx, 2, 4, 5);
    g.fillStyle(0x3a2414, 1);
    g.fillRect(hx, 2, 4, 2);
    if (dir === "n") g.fillRect(hx, 2, 4, 4);
    g.fillStyle(0x1a1208, 1);
    if (dir === "s") {
      p(g, hx, 5, 0x1a1208);
      p(g, hx + 3, 5, 0x1a1208);
    } else if (dir === "e") p(g, hx + 3, 5, 0x1a1208);
    else if (dir === "w") p(g, hx, 5, 0x1a1208);
    g.generateTexture(`player_${dir}`, 16, 16);
  }
  g.generateTexture("player", 16, 16);
}

function dogSheet(g: Phaser.GameObjects.Graphics): void {
  for (const dir of DIRS) {
    g.clear();
    shadow(g, 3, 13, 10);
    g.fillStyle(0xb88848, 1);
    if (dir === "e" || dir === "w") {
      g.fillRect(4, 8, 9, 4);
      g.fillRect(dir === "e" ? 11 : 2, 6, 4, 4);
      p(g, dir === "e" ? 14 : 2, 7, 0x1a1208);
      g.fillRect(dir === "e" ? 3 : 12, 9, 2, 2);
    } else {
      g.fillRect(5, 7, 6, 6);
      g.fillRect(6, 4, 4, 4);
      if (dir === "s") {
        p(g, 6, 6, 0x1a1208);
        p(g, 9, 6, 0x1a1208);
      }
    }
    g.fillStyle(0x5a3a18, 1);
    g.fillRect(5, 12, 2, 2);
    g.fillRect(9, 12, 2, 2);
    g.generateTexture(`dog_${dir}`, 16, 16);
  }
  g.generateTexture("dog", 16, 16);
}

function skeletonSheet(g: Phaser.GameObjects.Graphics): void {
  for (const dir of DIRS) {
    g.clear();
    shadow(g, 3, 13, 10);
    g.fillStyle(0xe8e0d0, 1);
    g.fillRect(6, 2, 4, 4);
    g.fillRect(6, 7, 4, 5);
    p(g, 6, 8, 0x4a4438);
    p(g, 9, 8, 0x4a4438);
    p(g, 7, 10, 0x4a4438);
    g.fillRect(5, 12, 2, 2);
    g.fillRect(9, 12, 2, 2);
    g.fillStyle(0x2a2418, 1);
    if (dir === "s") {
      p(g, 6, 3, 0x2a2418);
      p(g, 9, 3, 0x2a2418);
    }
    g.generateTexture(`skeleton_${dir}`, 16, 16);
  }
  g.generateTexture("skeleton", 16, 16);
}

function ratSheet(g: Phaser.GameObjects.Graphics): void {
  for (const dir of DIRS) {
    g.clear();
    shadow(g, 4, 13, 8);
    g.fillStyle(0x6a4a3a, 1);
    g.fillRect(4, 9, 8, 4);
    g.fillStyle(0xc49090, 1);
    p(g, 4, 8, 0xc49090);
    p(g, 11, 8, 0xc49090);
    g.fillStyle(0x4a3024, 1);
    g.fillRect(dir === "w" ? 2 : 12, 10, 2, 1);
    g.generateTexture(`rat_${dir}`, 16, 16);
  }
  g.generateTexture("rat", 16, 16);
}

function snakeSheet(g: Phaser.GameObjects.Graphics): void {
  for (const dir of DIRS) {
    g.clear();
    shadow(g, 2, 13, 12);
    g.fillStyle(0x3a6a2a, 1);
    g.fillRect(3, 8, 10, 3);
    g.fillStyle(0x88cc44, 1);
    p(g, 4, 8, 0x88cc44);
    p(g, 8, 9, 0x88cc44);
    g.fillStyle(0x1a3a12, 1);
    g.fillRect(dir === "e" ? 12 : 2, 7, 3, 3);
    p(g, dir === "e" ? 14 : 2, 7, 0xe04040);
    g.generateTexture(`snake_${dir}`, 16, 16);
  }
  g.generateTexture("snake", 16, 16);
}

function banditSheet(g: Phaser.GameObjects.Graphics): void {
  for (const dir of DIRS) {
    g.clear();
    shadow(g, 3, 13, 10);
    g.fillStyle(0x5a2a2a, 1);
    g.fillRect(5, 7, 6, 5);
    g.fillStyle(0xc4a080, 1);
    g.fillRect(6, 3, 4, 4);
    g.fillStyle(0x8a2020, 1);
    g.fillRect(5, 9, 6, 2);
    g.fillStyle(0x2a1a10, 1);
    g.fillRect(5, 2, 6, 2);
    g.fillRect(5, 12, 2, 2);
    g.fillRect(9, 12, 2, 2);
    g.generateTexture(`bandit_${dir}`, 16, 16);
  }
  g.generateTexture("bandit", 16, 16);
}

function plantSheet(g: Phaser.GameObjects.Graphics): void {
  for (const dir of DIRS) {
    g.clear();
    shadow(g, 4, 13, 8);
    g.fillStyle(0x6a4a28, 1);
    g.fillRect(5, 10, 6, 4);
    g.fillStyle(0x3a6a22, 1);
    g.fillRect(7, 5, 2, 6);
    g.fillStyle(0x88aa44, 1);
    g.fillRect(4, 3, 8, 4);
    p(g, 7, 2, 0xc04060);
    g.generateTexture(`plant_${dir}`, 16, 16);
  }
  g.generateTexture("plant", 16, 16);
}

function blobSheet(
  g: Phaser.GameObjects.Graphics,
  name: string,
  body: number,
  shade: number,
  eye: number,
): void {
  for (const dir of DIRS) {
    g.clear();
    shadow(g, 3, 13, 10);
    g.fillStyle(body, 1);
    g.fillRect(4, 7, 8, 6);
    g.fillStyle(shade, 1);
    g.fillRect(5, 8, 6, 3);
    g.fillStyle(eye, 1);
    if (dir === "s" || dir === "e") p(g, 6, 8, eye);
    if (dir === "s" || dir === "w") p(g, 9, 8, eye);
    g.generateTexture(`${name}_${dir}`, 16, 16);
  }
  g.generateTexture(name, 16, 16);
}

function spiderSheet(g: Phaser.GameObjects.Graphics): void {
  for (const dir of DIRS) {
    g.clear();
    shadow(g, 3, 13, 10);
    g.fillStyle(0x3a2a24, 1);
    g.fillRect(5, 7, 6, 5);
    g.fillStyle(0x1a1210, 1);
    g.fillRect(2, 8, 3, 1);
    g.fillRect(11, 8, 3, 1);
    g.fillRect(2, 11, 3, 1);
    g.fillRect(11, 11, 3, 1);
    p(g, 6, 8, 0xc04040);
    p(g, 9, 8, 0xc04040);
    g.generateTexture(`spider_${dir}`, 16, 16);
  }
  g.generateTexture("spider", 16, 16);
}

function chestProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  shadow(g, 2, 13, 12);
  g.fillStyle(0x5a3a10, 1);
  g.fillRect(3, 7, 10, 6);
  g.fillStyle(0xd4a020, 1);
  g.fillRect(3, 5, 10, 4);
  g.fillStyle(0xe8d070, 1);
  g.fillRect(3, 5, 10, 1);
  p(g, 7, 8, 0xffe080);
  g.generateTexture("chest", 16, 16);
}

function crateProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  shadow(g, 2, 13, 12);
  g.fillStyle(0x6a4a28, 1);
  g.fillRect(3, 4, 10, 10);
  g.fillStyle(0x3a2a14, 1);
  g.fillRect(3, 8, 10, 1);
  g.fillRect(7, 4, 1, 10);
  g.fillStyle(0xb08a50, 1);
  g.fillRect(3, 4, 10, 1);
  g.generateTexture("crate", 16, 16);
}

function doorProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  g.fillStyle(0x2a1a10, 1);
  g.fillRect(4, 2, 8, 13);
  g.fillStyle(0x5a3e28, 1);
  g.fillRect(5, 3, 6, 11);
  p(g, 9, 9, 0xc4a024);
  g.generateTexture("doorway", 16, 16);
}

function benchProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  shadow(g, 2, 13, 12);
  g.fillStyle(0x4a3424, 1);
  g.fillRect(2, 8, 12, 5);
  g.fillStyle(0xc4a060, 1);
  g.fillRect(3, 6, 10, 3);
  g.generateTexture("bench", 16, 16);
}

function hatchProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  g.fillStyle(0x1a1a1a, 1);
  g.fillRect(3, 3, 10, 10);
  g.fillStyle(0x6a3a18, 1);
  g.fillRect(4, 4, 8, 8);
  g.fillStyle(0x2a1a10, 1);
  g.fillRect(4, 7, 8, 1);
  p(g, 10, 6, 0xc4a024);
  g.generateTexture("hatch", 16, 16);
}

function orbProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  shadow(g, 4, 13, 8);
  g.fillStyle(0x3a0e0e, 1);
  g.fillRect(5, 10, 6, 3);
  g.fillStyle(0xff5520, 1);
  g.fillRect(5, 4, 6, 6);
  g.fillStyle(0xffaa66, 1);
  g.fillRect(6, 5, 3, 3);
  p(g, 7, 6, 0xfff0c0);
  g.generateTexture("orb", 16, 16);
}

function iceOrbProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  shadow(g, 4, 13, 8);
  g.fillStyle(0x0e1a3a, 1);
  g.fillRect(5, 10, 6, 3);
  g.fillStyle(0x66aaff, 1);
  g.fillRect(5, 4, 6, 6);
  g.fillStyle(0xc0e8ff, 1);
  g.fillRect(6, 5, 3, 3);
  p(g, 7, 6, 0xffffff);
  g.generateTexture("orb_ice", 16, 16);
}

function signProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  shadow(g, 4, 13, 8);
  g.fillStyle(0x4a2e14, 1);
  g.fillRect(7, 8, 2, 6);
  g.fillStyle(0x8a6a38, 1);
  g.fillRect(3, 3, 10, 7);
  g.fillStyle(0xd4c090, 1);
  g.fillRect(4, 4, 8, 5);
  p(g, 5, 5, 0x3a2a14);
  p(g, 7, 6, 0x3a2a14);
  p(g, 9, 5, 0x3a2a14);
  g.generateTexture("sign", 16, 16);
}

function leverProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  shadow(g, 4, 13, 8);
  g.fillStyle(0x4a4a52, 1);
  g.fillRect(5, 10, 6, 4);
  g.fillStyle(0x8a8a94, 1);
  g.fillRect(7, 4, 2, 8);
  g.fillStyle(0xc04040, 1);
  g.fillRect(6, 2, 4, 3);
  p(g, 7, 3, 0xff8080);
  g.generateTexture("lever", 16, 16);
}

function campfireProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  shadow(g, 3, 13, 10);
  g.fillStyle(0x3a2414, 1);
  g.fillRect(4, 10, 8, 3);
  g.fillStyle(0xff6622, 1);
  g.fillRect(6, 4, 4, 7);
  p(g, 7, 3, 0xffe080);
  p(g, 8, 6, 0xffaa33);
  g.generateTexture("campfire", 16, 16);
}

function npcSheet(g: Phaser.GameObjects.Graphics): void {
  for (const dir of DIRS) {
    g.clear();
    shadow(g, 3, 13, 10);
    g.fillStyle(0x3a5a28, 1);
    g.fillRect(5, 7, 6, 5);
    g.fillStyle(0xc4a080, 1);
    g.fillRect(6, 3, 4, 4);
    g.fillStyle(0x2a6a24, 1);
    g.fillRect(5, 2, 6, 2);
    g.fillRect(5, 12, 2, 2);
    g.fillRect(9, 12, 2, 2);
    if (dir === "s") {
      p(g, 6, 4, 0x1a1208);
      p(g, 9, 4, 0x1a1208);
    }
    g.generateTexture(`npc_${dir}`, 16, 16);
  }
  g.generateTexture("npc", 16, 16);
}

function shrineProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  shadow(g, 3, 13, 10);
  g.fillStyle(0x8a8a94, 1);
  g.fillRect(5, 6, 6, 8);
  g.fillStyle(0xc8c8d0, 1);
  g.fillRect(4, 3, 8, 4);
  p(g, 7, 4, 0xaa66cc);
  p(g, 8, 5, 0xddaaee);
  g.generateTexture("shrine", 16, 16);
}

function torchProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  g.fillStyle(0x4a2a10, 1);
  g.fillRect(7, 7, 2, 7);
  g.fillStyle(0xffaa33, 1);
  g.fillRect(6, 3, 4, 5);
  p(g, 7, 2, 0xfff0a0);
  p(g, 8, 4, 0xff6622);
  g.generateTexture("torch", 16, 16);
}

function brokenProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  shadow(g, 2, 13, 12);
  g.fillStyle(0x4a2e18, 1);
  g.fillRect(2, 8, 12, 5);
  g.fillStyle(0x2a1a10, 1);
  g.fillRect(4, 6, 3, 3);
  g.fillRect(9, 5, 4, 4);
  g.generateTexture("broken", 16, 16);
}

function dropProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  shadow(g, 4, 13, 8);
  g.fillStyle(0x8a6a30, 1);
  g.fillRect(5, 7, 6, 6);
  g.fillStyle(0xe0c050, 1);
  g.fillRect(6, 5, 4, 3);
  p(g, 7, 6, 0xfff0a0);
  g.generateTexture("drop", 16, 16);
}

function reticleProp(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  const c = 0xf0d070;
  g.fillStyle(c, 1);
  g.fillRect(1, 1, 4, 1);
  g.fillRect(1, 1, 1, 4);
  g.fillRect(11, 1, 4, 1);
  g.fillRect(14, 1, 1, 4);
  g.fillRect(1, 14, 4, 1);
  g.fillRect(1, 11, 1, 4);
  g.fillRect(11, 14, 4, 1);
  g.fillRect(14, 11, 1, 4);
  g.generateTexture("reticle", 16, 16);
}

function bolt(g: Phaser.GameObjects.Graphics, key: string, hi: number, lo: number): void {
  g.clear();
  g.fillStyle(lo, 1);
  g.fillRect(2, 1, 4, 6);
  g.fillStyle(hi, 1);
  g.fillRect(3, 2, 2, 4);
  p(g, 3, 3, 0xffffff);
  g.generateTexture(key, 8, 8);
}

function slash(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  g.fillStyle(0xf4e7c8, 0.95);
  g.fillRect(1, 7, 14, 2);
  g.fillStyle(0xffffff, 0.7);
  g.fillRect(2, 6, 11, 1);
  g.generateTexture("slash", 16, 16);
}

function spark(g: Phaser.GameObjects.Graphics): void {
  g.clear();
  g.fillStyle(0xf0d060, 1);
  g.fillRect(3, 0, 2, 8);
  g.fillRect(0, 3, 8, 2);
  p(g, 3, 3, 0xffffff);
  g.generateTexture("spark", 8, 8);
}
