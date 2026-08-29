import { stampTrigger } from "@/game/systems/triggers";
import { Grid } from "@/game/world/Grid";
import { furnitureWall, paintBorder } from "@/game/world/motifs";
import { cellWorld, getPocket, stampPocket } from "@/game/world/stamp";
import type { ZoneBlue } from "@/game/world/zoneTypes";

export function generateHouse(): ZoneBlue {
  const cols = 20;
  const rows = 18;
  const grid = new Grid(cols, rows, "wood");
  paintBorder(grid);
  grid.fillRect(8, rows - 1, 2, 1, "door");

  const dummy = () => 0.5;
  const kitchen = stampPocket(grid, getPocket("kitchen"), 2, 2, dummy, 0);
  const furniture = furnitureWall({ cx: 12, cy: 3 }, 3, "kitchen_bench");

  return {
    grid,
    spawn: cellWorld(10, rows - 3),
    required: ["bench", "hatch_a"],
    triggers: [stampTrigger("see_the_kitchen", cellWorld(10, rows - 3).x, cellWorld(10, rows - 3).y)],
    lights: [
      { ...cellWorld(4, 5), radius: 32 },
      { ...cellWorld(14, 5), radius: 26 },
    ],
    enemies: [],
    props: [...kitchen.props, ...furniture],
  };
}
