import Phaser from "phaser";
import type { Grid } from "@/game/world/Grid";
import { mapInner, gy } from "@/game/ui/layout";
import type { Fog } from "@/game/ui/fog";
import { session } from "@/game/systems/session";
import { ZONE_TITLE } from "@/game/ui/zoneTitles";
import { clearText, fillText } from "@/game/ui/chrome";
import { DEPTH, type Layers } from "@/game/ui/layers";
import { uiText, white } from "@/game/ui/theme";

const RADAR = 16;
const TEX_KEY = "minimap-chart";

/** Parchment-chart palette. Unseen is deep ink, not magenta. */
const INK = 0x14100c;
const GRASS = 0xe4d4b0;
const WOOD = 0x8a5a32;
const STONE = 0xa89878;
const BONE = 0xe8dcc8;
const WATER = 0x4a5c6c;
const BUSH = 0x5a6a38;
const WALL = 0x3a2418;
const DOOR = 0xc4a070;
const PIP = 0xf4efe4;
const PIP_EDGE = 0x1a1008;

function hex(n: number): string {
  return `#${n.toString(16).padStart(6, "0")}`;
}

export class MiniMap {
  private readonly scene: Phaser.Scene;
  private readonly overlay: Phaser.GameObjects.Graphics;
  private readonly chart: Phaser.GameObjects.Image;
  private readonly title: Phaser.GameObjects.Text;
  private texW = 0;
  private texH = 0;

  constructor(scene: Phaser.Scene, layers: Layers) {
    this.scene = scene;
    this.chart = layers.addUi(
      scene.add.image(0, 0, "__WHITE").setOrigin(0).setVisible(false).setDepth(DEPTH.guiText),
    );
    this.overlay = layers.addUi(scene.add.graphics().setDepth(DEPTH.guiText));
    this.title = layers.addUi(
      scene.add.text(0, 0, "", { ...uiText, fontSize: `${gy(8)}px`, color: white }).setDepth(DEPTH.guiText),
    );
  }

  paint(grid: Grid, fog: Fog, px: number, py: number): void {
    const pane = mapInner;
    const { cx, cy } = grid.worldToCell(px, py);
    const county = session.zone === "county";
    const cols = county ? RADAR * 2 + 1 : grid.cols;
    const rows = county ? RADAR * 2 + 1 : grid.rows;
    const canvas = this.ensureCanvas(cols, rows);
    const ctx = canvas.getContext();
    ctx.imageSmoothingEnabled = false;
    if (county) this.plotRadar(ctx, grid, cx, cy, cols, rows);
    else this.plotFull(ctx, grid, fog, cols, rows);
    canvas.refresh();

    const scale = Math.min(pane.w / cols, pane.h / rows);
    const dw = cols * scale;
    const dh = rows * scale;
    const ox = pane.x + (pane.w - dw) / 2;
    const oy = pane.y + (pane.h - dh) / 2;

    this.chart.setTexture(TEX_KEY);
    this.chart.setVisible(true);
    this.chart.setPosition(ox, oy);
    this.chart.setDisplaySize(dw, dh);
    this.chart.texture.setFilter(Phaser.Textures.FilterMode.NEAREST);

    this.overlay.clear();
    this.overlay.fillStyle(INK, 0.62);
    this.overlay.fillRect(pane.x + 4, pane.y + 3, Math.min(268, pane.w - 8), gy(12) + 4);
    const pipCx = county ? RADAR : cx;
    const pipCy = county ? RADAR : cy;
    this.pip(ox + (pipCx + 0.5) * scale, oy + (pipCy + 0.5) * scale, scale);
    if (county) this.northTick(ox, oy, dw);

    fillText(this.title, { x: pane.x + 6, y: pane.y + 4, w: Math.min(pane.w - 12, 280), h: gy(12) }, ZONE_TITLE[session.zone]);
  }

  hide(): void {
    this.chart.setVisible(false);
    this.overlay.clear();
    clearText(this.title);
  }

  private ensureCanvas(w: number, h: number): Phaser.Textures.CanvasTexture {
    if (this.scene.textures.exists(TEX_KEY) && this.texW === w && this.texH === h) {
      return this.scene.textures.get(TEX_KEY) as Phaser.Textures.CanvasTexture;
    }
    this.chart.setTexture("__WHITE");
    if (this.scene.textures.exists(TEX_KEY)) this.scene.textures.remove(TEX_KEY);
    const canvas = this.scene.textures.createCanvas(TEX_KEY, w, h);
    if (!canvas) throw new Error("minimap canvas");
    canvas.setFilter(Phaser.Textures.FilterMode.NEAREST);
    this.texW = w;
    this.texH = h;
    return canvas;
  }

  private plotRadar(
    ctx: CanvasRenderingContext2D,
    grid: Grid,
    cx: number,
    cy: number,
    cols: number,
    rows: number,
  ): void {
    const x0 = cx - RADAR;
    const y0 = cy - RADAR;
    for (let j = 0; j < rows; j++) {
      for (let i = 0; i < cols; i++) {
        ctx.fillStyle = hex(this.liveColor(grid, x0 + i, y0 + j));
        ctx.fillRect(i, j, 1, 1);
      }
    }
  }

  private plotFull(
    ctx: CanvasRenderingContext2D,
    grid: Grid,
    fog: Fog,
    cols: number,
    rows: number,
  ): void {
    for (let ty = 0; ty < rows; ty++) {
      for (let tx = 0; tx < cols; tx++) {
        ctx.fillStyle = hex(fog.seen(tx, ty) ? this.tileColor(grid, tx, ty) : INK);
        ctx.fillRect(tx, ty, 1, 1);
      }
    }
  }

  private liveColor(grid: Grid, cx: number, cy: number): number {
    if (!grid.inBounds(cx, cy)) return INK;
    return this.tileColor(grid, cx, cy);
  }

  private tileColor(grid: Grid, cx: number, cy: number): number {
    switch (grid.get(cx, cy)) {
      case "water":
        return WATER;
      case "wood":
        return WOOD;
      case "stone":
        return STONE;
      case "bone":
        return BONE;
      case "bush":
        return BUSH;
      case "house":
      case "wall":
        return WALL;
      case "door":
        return DOOR;
      default:
        return GRASS;
    }
  }

  private pip(x: number, y: number, cell: number): void {
    const s = Math.max(3.5, Math.min(6, cell * 0.55));
    this.overlay.fillStyle(PIP_EDGE, 1);
    this.diamond(x, y, s + 1.6);
    this.overlay.fillStyle(PIP, 1);
    this.diamond(x, y, s);
  }

  private diamond(x: number, y: number, r: number): void {
    this.overlay.fillTriangle(x, y - r, x + r, y, x, y + r);
    this.overlay.fillTriangle(x, y - r, x - r, y, x, y + r);
  }

  private northTick(ox: number, oy: number, dw: number): void {
    const mx = ox + dw / 2;
    this.overlay.fillStyle(PIP, 0.95);
    this.overlay.fillTriangle(mx, oy + 3, mx - 3.5, oy + 11, mx + 3.5, oy + 11);
    this.overlay.fillRect(mx - 0.75, oy + 10, 1.5, 6);
  }
}
