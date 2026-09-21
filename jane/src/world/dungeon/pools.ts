// Every .room file there is, parsed once, by pool. A dungeon's rooms live in a folder of
// their own (rooms/mine/*.room); a pool is a name templates share (`mine.plate`), and the
// mission only ever names the pool.

import { parseRoom } from "@/world/dungeon/room";
import type { RoomTemplate } from "@/world/dungeon/types";

const FILES = import.meta.glob("./rooms/*/*.room", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

/** In file path order, which is what makes "the first template of the pool" mean something. */
export const TEMPLATES: readonly RoomTemplate[] = Object.keys(FILES)
  .sort()
  .map((path) => parseRoom(FILES[path], path));

{
  const seen = new Set<string>();
  for (const t of TEMPLATES) {
    if (seen.has(t.id)) throw new Error(`Room template "${t.id}" is defined twice`);
    seen.add(t.id);
  }
}

export function poolOf(pool: string): RoomTemplate[] {
  return TEMPLATES.filter((t) => t.pool === pool);
}

export function templateById(id: string): RoomTemplate | undefined {
  return TEMPLATES.find((t) => t.id === id);
}
