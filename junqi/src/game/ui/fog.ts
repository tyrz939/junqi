import type { ZoneId } from "@/game/types";
import type { Grid } from "@/game/world/Grid";
import { session } from "@/game/systems/session";

const REVEAL_R = 8;

export class Fog {
  private bits = new Uint8Array(0);
  private cols = 0;
  private rows = 0;
  zone: ZoneId | null = null;

  ensure(zone: ZoneId, cols: number, rows: number): void {
    if (this.zone === zone && this.cols === cols && this.rows === rows) return;
    this.zone = zone;
    this.cols = cols;
    this.rows = rows;
    if (zone === "county") {
      this.bits = new Uint8Array(0);
      return;
    }
    this.bits = new Uint8Array(cols * rows);
    const packed = session.readZone(zone)?.fog;
    if (packed) unpack(packed, this.bits);
  }

  tick(grid: Grid, cx: number, cy: number): void {
    this.ensure(session.zone, grid.cols, grid.rows);
    if (session.zone === "county") return;
    const r = REVEAL_R;
    const x0 = Math.max(0, cx - r);
    const y0 = Math.max(0, cy - r);
    const x1 = Math.min(this.cols - 1, cx + r);
    const y1 = Math.min(this.rows - 1, cy + r);
    const r2 = r * r;
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) {
        const dx = x - cx;
        const dy = y - cy;
        if (dx * dx + dy * dy <= r2) this.bits[y * this.cols + x] = 1;
      }
    }
  }

  seen(cx: number, cy: number): boolean {
    if (cx < 0 || cy < 0 || cx >= this.cols || cy >= this.rows) return false;
    return this.bits[cy * this.cols + cx] === 1;
  }

  pack(): string | undefined {
    if (!this.zone || this.zone === "county" || this.bits.length === 0) return undefined;
    return pack(this.bits);
  }
}

function pack(bits: Uint8Array): string {
  const bytes = new Uint8Array(Math.ceil(bits.length / 8));
  for (let i = 0; i < bits.length; i++) {
    if (bits[i]) bytes[i >> 3] |= 1 << (i & 7);
  }
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin);
}

function unpack(b64: string, bits: Uint8Array): void {
  try {
    const bin = atob(b64);
    for (let i = 0; i < bits.length; i++) {
      const byte = bin.charCodeAt(i >> 3);
      bits[i] = byte & (1 << (i & 7)) ? 1 : 0;
    }
  } catch {
    bits.fill(0);
  }
}
