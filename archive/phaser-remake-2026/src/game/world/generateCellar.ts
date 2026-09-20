import { createRng } from "@/game/systems/rng";
import { stampTrigger } from "@/game/systems/triggers";
import { Grid } from "@/game/world/Grid";
import { barrelPile, roomCenter, stoneTunnel, type RoomRect } from "@/game/world/motifs";
import { cellWorld, getPocket, stampPocket } from "@/game/world/stamp";
import type { ZoneBlue } from "@/game/world/zoneTypes";

export function generateCellar(seed: number): ZoneBlue {
  const rng = createRng(seed ^ 0x9e3779b9);
  const cols = 40;
  const rows = 28;
  const grid = new Grid(cols, rows, "wall");

  const hall: RoomRect = { x: 4, y: 18, w: 14, h: 7 };
  const mid: RoomRect = { x: 16 + Math.floor(rng() * 4), y: 8 + Math.floor(rng() * 3), w: 10, h: 8 };
  const key: RoomRect = { x: 26 + Math.floor(rng() * 3), y: 4 + Math.floor(rng() * 3), w: 10, h: 8 };
  const rooms = [hall, mid, key];
  for (const room of rooms) grid.fillRect(room.x, room.y, room.w, room.h, "stone");
  stoneTunnel(grid, roomCenter(hall), roomCenter(mid));
  stoneTunnel(grid, roomCenter(mid), roomCenter(key));

  const stairs = roomCenter(hall);
  const spawn = cellWorld(stairs.cx, stairs.cy + 2);
  const keyStamp = stampPocket(grid, getPocket("cellar_key"), key.x + 1, key.y + 1, rng, 1);
  grid.fillRect(key.x + 6, key.y + 5, 2, 2, "bush");

  return {
    grid,
    spawn,
    required: ["cellar_gate"],
    triggers: [stampTrigger("under_the_house", spawn.x, spawn.y)],
    lights: [
      { ...cellWorld(stairs.cx, stairs.cy), radius: 28 },
      { ...cellWorld(roomCenter(key).cx, roomCenter(key).cy), radius: 20 },
    ],
    enemies: rng() < 0.6 ? [{ id: "cellar_rat", kind: "rat", ...cellWorld(roomCenter(mid).cx, roomCenter(mid).cy) }] : [],
    props: [
      {
        id: "cellar_up",
        kind: "hatch",
        ...cellWorld(stairs.cx, stairs.cy + 3),
        texture: "hatch",
        toZone: "house",
      },
      ...keyStamp.props.map((p) =>
        p.id === "cellar_gate" ? { ...p, ...cellWorld(key.x, roomCenter(key).cy) } : p,
      ),
      ...barrelPile({ cx: mid.x + 1, cy: mid.y + 1 }, "cellar_barrel", 3),
    ],
  };
}

