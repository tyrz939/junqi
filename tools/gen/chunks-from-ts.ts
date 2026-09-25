// Build every chunk of jane/src/world/chunks.ts on a fresh Kit at a fixed origin and dump, in box
// cells, its tiles, props (with the footprint each claimed), units, marks, rects, gates, slots and
// claim mask, as JSON for tools/gen/chunks-from-ts.py (which wrote data/chunks/*.chunk from it and
// checks them against it). Run from jane/ (it needs the @ alias and vite-node):
//
//   npx vite-node --config vite.config.ts ../tools/gen/chunks-from-ts.ts /tmp/chunks.json
//   python3 ../tools/gen/chunks-from-ts.py /tmp/chunks.json check
import { writeFileSync } from "node:fs";
import { CHUNKS } from "@/world/chunks";
import { Kit } from "@/world/kit";
import { Tile } from "@/sim/grid";

const W = 600;
const H = 600;
const out: Record<string, unknown> = {};
for (const [id, build] of Object.entries(CHUNKS)) {
  const k = new Kit("county" as never, W, H, 12345, 0, Tile.Void);
  const foot: { w: number; h: number }[] = [];
  const origProp = k.prop.bind(k);
  (k as unknown as { prop: typeof k.prop }).prop = (s, w, h) => {
    foot.push({ w, h });
    return origProp(s, w, h);
  };
  const c = k.within(`chunk:${id}`, () => build(k, 300, 300));
  const b = c.box;
  const tiles: number[][] = [];
  for (let y = 0; y < b.h; y++) {
    const row: number[] = [];
    for (let x = 0; x < b.w; x++) row.push(k.get(b.cx + x, b.cy + y));
    tiles.push(row);
  }
  let outsideTiles = 0;
  for (let y = 0; y < H; y++)
    for (let x = 0; x < W; x++) {
      const inside = x >= b.cx && y >= b.cy && x < b.cx + b.w && y < b.cy + b.h;
      if (!inside && k.get(x, y) !== Tile.Void) outsideTiles++;
    }
  const claims: string[] = [];
  let outsideClaims = 0;
  for (let y = 0; y < b.h; y++) {
    let s = "";
    for (let x = 0; x < b.w; x++) s += k.isClaimed(b.cx + x, b.cy + y) ? "x" : ".";
    claims.push(s);
  }
  for (let y = b.cy - 4; y < b.cy + b.h + 4; y++)
    for (let x = b.cx - 4; x < b.cx + b.w + 4; x++) {
      const inside = x >= b.cx && y >= b.cy && x < b.cx + b.w && y < b.cy + b.h;
      if (!inside && k.inside(x, y) && k.isClaimed(x, y)) outsideClaims++;
    }
  out[id] = {
    box: b,
    tiles,
    outsideTiles,
    claims,
    outsideClaims,
    props: k.props.map((p, i) => ({ ...p, anon: p.key.startsWith("county_"), fw: foot[i].w, fh: foot[i].h, x: p.cx - b.cx, y: p.cy - b.cy })),
    units: k.units.map((u) => ({ ...u, anon: u.key.startsWith("county_"), x: u.cx - b.cx, y: u.cy - b.cy, route: u.patrol?.map((q) => (q.length === 3 ? [q[0] - b.cx, q[1] - b.cy, q[2]] : [q[0] - b.cx, q[1] - b.cy])) })),
    marks: Object.entries(k.marks).map(([name, m]) => ({ name, x: m.cx - b.cx, y: m.cy - b.cy, facing: m.facing })),
    rects: Object.entries(k.rects).map(([name, r]) => ({ name, x: r.cx - b.cx, y: r.cy - b.cy, w: r.w, h: r.h })),
    gates: c.gates.map(([x, y]) => [x - b.cx, y - b.cy]),
    slots: Object.entries(c.slots ?? {}).map(([name, [x, y]]) => ({ name, x: x - b.cx, y: y - b.cy })),
    tileNames: Object.fromEntries(Object.entries(Tile).filter(([, v]) => typeof v === "number").map(([n, v]) => [v, n])),
  };
}
writeFileSync(process.argv[2] ?? "/dev/stdout", JSON.stringify(out, null, 1));
