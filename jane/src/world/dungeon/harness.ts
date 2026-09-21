// The template harness (DUNGEONS.md 2.4). A template is an interface, and this proves it
// keeps its word: stamp the room ALONE, holding what its mission node holds, with a dead-end
// stub on every door; then run the lock-and-key solver from each door in turn.
//
//   grants   every socket it names is reached, and every other door can be walked out of
//   blocks   with the `until` socket's lists suppressed, the `what` socket stays shut
//
// A variant that cannot prove itself cannot ship. The generator never calls this; the tests
// (and the viewer, one day) do.

import type { Catalog } from "@/sim/catalog";
import { ZONE_ATTEMPTS, type Blueprint } from "@/world/blueprint";
import { buildDungeon, localName } from "@/world/dungeon/generate";
import { shapesOf, type Shape } from "@/world/dungeon/room";
import type { DungeonDef, MissionNode, RoomTemplate } from "@/world/dungeon/types";
import { validateBlueprint, type SolveTrace } from "@/world/validate";

/**
 * One room and nothing else, as a whole (small) zone. The mission node rides along so its
 * sockets are filled. The zone has a name of its own (`mine_room`), so the story's trigger
 * rows for the real zone do not come looking for rects that are not here.
 */
export function buildRoomAlone(def: DungeonDef, node: MissionNode, shape: Shape): { bp: Blueprint; def: DungeonDef } {
  const alone: DungeonDef = {
    ...def,
    id: `${def.id}_room`,
    lattice: { cols: shape.bays[0] + 2, rows: shape.bays[1] + 2 },
    nodes: [{ ...node, critical: true }],
    edges: [],
    budget: { ...def.budget, sideRooms: [0, 0] },
    fallback: [{ node: node.id, template: shape.template.id, bay: [1, 1], turn: shape.turn, mirror: shape.mirror }],
  };
  // The last attempt is the hand-placed one: exactly this template, exactly here.
  return { bp: buildDungeon(alone, 1, ZONE_ATTEMPTS - 1, { stubs: true }), def: alone };
}

function reached(bp: Blueprint, catalog: Catalog, trace: SolveTrace, key: string): boolean {
  const p = bp.props.find((x) => x.key === key);
  if (!p) {
    const u = bp.units.find((x) => x.key === key);
    return u ? trace.firstSeen[u.cy * bp.w + u.cx] >= 0 : false;
  }
  // Something locked is reached when it has been opened; anything else, when she can stand beside it.
  if (p.locked) return trace.firedAt[key] !== undefined;
  const d = catalog.props[p.def];
  for (let y = p.cy - 1; y <= p.cy + d.h; y++) for (let x = p.cx - 1; x <= p.cx + d.w; x++) if (trace.firstSeen[y * bp.w + x] >= 0) return true;
  return false;
}

/** Every promise the template makes, tried in every transform it allows, from every door. Empty means it holds. */
export function proveTemplate(def: DungeonDef, node: MissionNode, template: RoomTemplate, catalog: Catalog): string[] {
  const errors: string[] = [];
  const contract = { units: [], props: [], marks: [], rects: [] };
  for (const shape of shapesOf(template)) {
    if (!shape.fits) continue;
    const at = `${template.id} turn ${shape.turn}${shape.mirror ? " mirrored" : ""}`;
    const { bp, def: alone } = buildRoomAlone(def, node, shape);
    const doorMark = (id: string): string => `${alone.id}_${node.id}_door_${id}`;
    for (const door of shape.doors) {
      const solve = validateBlueprint(bp, catalog, contract, def.givenKeys, { entry: doorMark(door.id), trace: true });
      if (!solve.trace) {
        errors.push(`${at}: ${solve.errors.join("; ")}`);
        break;
      }
      for (const other of shape.doors) {
        const m = bp.marks[doorMark(other.id)];
        if (solve.trace.firstSeen[m.cy * bp.w + m.cx] < 0) errors.push(`${at}: in by ${door.id}, she cannot leave by ${other.id}`);
      }
      for (const g of template.grants) {
        if (!node.holds.some((h) => h.socket === g)) continue;
        if (!reached(bp, catalog, solve.trace, localName(alone, node, g))) errors.push(`${at}: in by ${door.id}, ${g} is never reached`);
      }
      for (const b of template.blocks) {
        const what = localName(alone, node, b.what);
        const until = localName(alone, node, b.until);
        const without = validateBlueprint(bp, catalog, contract, def.givenKeys, { entry: doorMark(door.id), trace: true, withhold: { props: [until] } });
        if (without.trace && reached(bp, catalog, without.trace, what)) errors.push(`${at}: ${b.what} opens without ${b.until}`);
      }
    }
  }
  return errors;
}
