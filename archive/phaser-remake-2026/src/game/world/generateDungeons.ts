import { stampTrigger, makeTrigger } from "@/game/systems/triggers";
import type { ZoneId } from "@/game/types";
import { Grid, type Tile } from "@/game/world/Grid";
import { barrelPile, paintBorder, roomCenter, stoneTunnel, torchGrid, type RoomRect } from "@/game/world/motifs";
import { cellWorld } from "@/game/world/stamp";
import type { EnemySpec, FloodSpec, PropSpec, ZoneBlue } from "@/game/world/zoneTypes";

function room(grid: Grid, x: number, y: number, w: number, h: number, tile: Tile): RoomRect {
  grid.fillRect(x, y, w, h, tile);
  return { x, y, w, h };
}

function link(grid: Grid, a: RoomRect, b: RoomRect): void {
  stoneTunnel(grid, roomCenter(a), roomCenter(b));
}

function at(r: RoomRect, dx: number, dy: number): { x: number; y: number } {
  return cellWorld(r.x + dx, r.y + dy);
}

function door(
  id: string,
  pos: { x: number; y: number },
  extra: Partial<PropSpec> = {},
): PropSpec {
  return { id, kind: "door", x: pos.x, y: pos.y, texture: "doorway", ...extra };
}

function hatch(id: string, pos: { x: number; y: number }, extra: Partial<PropSpec> = {}): PropSpec {
  return { id, kind: "hatch", x: pos.x, y: pos.y, texture: "hatch", ...extra };
}

function lever(id: string, pos: { x: number; y: number }, actions: string[]): PropSpec {
  return { id, kind: "toggle", x: pos.x, y: pos.y, texture: "lever", actions };
}

function sign(id: string, pos: { x: number; y: number }, talk: string): PropSpec {
  return { id, kind: "sign", x: pos.x, y: pos.y, texture: "sign", talk };
}

function trash(id: string, kind: string, pos: { x: number; y: number }, tag?: string): EnemySpec {
  return { id, kind, x: pos.x, y: pos.y, respawn: 0, tag };
}

function exitTo(id: string, zone: ZoneId, pos: { x: number; y: number }, kind: "door" | "hatch" = "door"): PropSpec {
  return {
    id,
    kind,
    x: pos.x,
    y: pos.y,
    texture: kind === "hatch" ? "hatch" : "doorway",
    toZone: zone,
  };
}

function lightsFor(rooms: RoomRect[], prefix: string, step = 6): { props: PropSpec[]; lights: { x: number; y: number; radius: number }[] } {
  const props: PropSpec[] = [];
  const lights: { x: number; y: number; radius: number }[] = [];
  rooms.forEach((r, i) => {
    const t = torchGrid(r, `${prefix}_${i}`, step);
    props.push(...t.props);
    lights.push(...t.lights);
  });
  return { props, lights };
}

/** Loading bay you can leave. Locked line you can see. Lever in the crates. Clear the floor. Repair the grate. */
export function generateFactory(): ZoneBlue {
  const cols = 72;
  const rows = 42;
  const grid = new Grid(cols, rows, "wall");
  paintBorder(grid);
  const bay = room(grid, 4, 24, 16, 14, "stone");
  const crates = room(grid, 4, 6, 16, 14, "stone");
  const hall = room(grid, 22, 24, 12, 14, "stone");
  const line = room(grid, 36, 20, 16, 18, "stone");
  const office = room(grid, 36, 4, 16, 12, "stone");
  const shop = room(grid, 22, 6, 12, 14, "stone");
  const pit = room(grid, 54, 20, 14, 16, "stone");
  link(grid, bay, crates);
  link(grid, bay, hall);
  link(grid, hall, line);
  link(grid, line, office);
  link(grid, hall, shop);
  link(grid, line, pit);

  const spawn = at(bay, 8, 11);
  const torch = lightsFor([bay, crates, hall, line, office, shop, pit], "factory_torch", 5);

  const props: PropSpec[] = [
    exitTo("factory_up", "county", at(bay, 8, 13)),
    sign("factory_dock", at(bay, 3, 3), "factory_dock"),
    door("factory_gate", at(hall, 0, 7), {
      locked: true,
      lockHint: "The line is dead. Something in the crates still has a say.",
    }),
    lever("factory_lever", at(crates, 2, 2), [
      "unlock:factory_gate",
      "note:The line lurches. The bay remembers boots.",
    ]),
    lever("factory_records", at(office, 12, 6), [
      "unlock:factory_pit_door",
      "note:A second door on the line. You walked past the idea of it.",
    ]),
    door("factory_pit_door", at(pit, 0, 8), {
      locked: true,
      lockHint: "Records said the pit waits on an office lever.",
    }),
    {
      id: "factory_pit_chest",
      kind: "chest",
      ...at(pit, 8, 4),
      texture: "chest",
      loot: [
        { id: "fire_stone", qty: 1 },
        { id: "iron", qty: 2 },
      ],
    },
    ...barrelPile({ cx: crates.x + 4, cy: crates.y + 3 }, "factory_crate", 5),
    door("factory_line_door", at(line, 0, 9), { lockHint: "The floor wants a body count." }),
    door("factory_office_door", at(office, 8, 11), {
      locked: true,
      lockHint: "The office waits until the floor is quiet.",
    }),
    sign("factory_office_note", at(office, 4, 4), "factory_office"),
    { id: "factory_ember", kind: "orb", ...at(office, 2, 6), texture: "orb", learn: "ember" },
    {
      id: "factory_office_chest",
      kind: "chest",
      ...at(office, 10, 4),
      texture: "chest",
      loot: [
        { id: "iron", qty: 3 },
        { id: "coal", qty: 2 },
      ],
    },
    {
      id: "factory_wreck",
      kind: "broken",
      ...at(shop, 3, 4),
      texture: "broken",
      actions: [
        "unlock:pipes_hatch",
        "set:factory_repaired",
        "note:The grate remembers how to open. The pipes are a second street.",
      ],
    },
    hatch("pipes_hatch", at(shop, 8, 8), {
      locked: true,
      toZone: "pipes",
      lockHint: "Dead grate. Repair the wreck.",
    }),
    sign("factory_shop_sign", at(shop, 2, 2), "factory_shop"),
    ...torch.props,
  ];

  const lineCrew = [
    trash("factory_line_a", "bandit", at(line, 4, 6), "factory_line"),
    trash("factory_line_b", "bandit", at(line, 10, 8), "factory_line"),
    trash("factory_line_c", "rat", at(line, 7, 12), "factory_line"),
    trash("factory_pit_a", "bandit", at(pit, 6, 8)),
    trash("factory_pit_b", "skeleton", at(pit, 10, 10)),
  ];

  return {
    grid,
    spawn,
    required: ["factory_up"],
    floods: [],
    triggers: [
      stampTrigger("into_factory", spawn.x, spawn.y),
      makeTrigger("factory_slam", "enter", ["lock:factory_line_door", "note:The door behind you takes a shift."], {
        x: at(line, 8, 8).x,
        y: at(line, 8, 8).y,
        w: 80,
        h: 80,
      }),
      makeTrigger(
        "factory_clear",
        "clear",
        ["unlock:factory_line_door", "unlock:factory_office_door", "note:The line goes quiet. The office unclenches."],
        { watch: lineCrew.map((e) => e.id) },
      ),
    ],
    lights: [{ ...spawn, radius: 24 }, ...torch.lights],
    enemies: lineCrew,
    props,
  };
}

/** Hall you walk twice. West classroom is desks. East is a fight. Office is the key. Trophy you saw first. */
export function generateSchool(): ZoneBlue {
  const cols = 46;
  const rows = 44;
  const grid = new Grid(cols, rows, "wall");
  paintBorder(grid);
  const hall = room(grid, 17, 8, 12, 20, "wood");
  const west = room(grid, 3, 12, 12, 12, "wood");
  const east = room(grid, 31, 12, 12, 12, "wood");
  const office = room(grid, 15, 2, 16, 8, "wood");
  const trophy = room(grid, 31, 24, 10, 8, "wood");
  const south = room(grid, 16, 30, 14, 10, "wood");
  link(grid, hall, west);
  link(grid, hall, east);
  link(grid, hall, office);
  link(grid, hall, trophy);
  link(grid, hall, south);

  const spawn = at(hall, 6, 18);
  const torch = lightsFor([hall, west, east, office, trophy, south], "school_torch", 6);

  const props: PropSpec[] = [
    exitTo("school_up", "county", at(hall, 6, 19)),
    sign("school_hall_sign", at(hall, 2, 10), "school_hall"),
    door("school_office_door", at(office, 8, 7), {
      locked: true,
      opens: "school_office",
      lockHint: "Principal's office. A classroom still has the key.",
    }),
    door("school_trophy_gate", at(trophy, 0, 4), {
      locked: true,
      lockHint: "A case you walked past. Offices open cases.",
    }),
    {
      id: "school_key_chest",
      kind: "chest",
      ...at(west, 9, 3),
      texture: "chest",
      loot: [{ id: "key_school", qty: 1 }],
    },
    ...barrelPile({ cx: west.x + 2, cy: west.y + 3 }, "school_desk", 6),
    sign("school_west_sign", at(west, 2, 2), "school_desks"),
    sign("school_chalk", at(east, 6, 3), "school_chalk"),
    sign("school_office_note", at(office, 4, 3), "school_office"),
    lever("school_bell", at(office, 12, 3), [
      "unlock:school_trophy_gate",
      "note:The bell does not know the year. The case in the hall does.",
    ]),
    {
      id: "school_trophy",
      kind: "chest",
      ...at(trophy, 6, 4),
      texture: "chest",
      loot: [
        { id: "gold_dust", qty: 2 },
        { id: "light_stone", qty: 1 },
      ],
    },
    {
      id: "school_south_chest",
      kind: "chest",
      ...at(south, 10, 4),
      texture: "chest",
      loot: [
        { id: "apple", qty: 3 },
        { id: "small_water", qty: 2 },
      ],
    },
    sign("school_south_sign", at(south, 2, 2), "school_hall"),
    ...torch.props,
  ];

  const classEast = [
    trash("school_skel_a", "skeleton", at(east, 4, 6), "school_east"),
    trash("school_skel_b", "skeleton", at(east, 8, 8), "school_east"),
    trash("school_south_a", "rat", at(south, 5, 5)),
    trash("school_south_b", "rat", at(south, 8, 7)),
  ];

  return {
    grid,
    spawn,
    required: ["school_up"],
    triggers: [
      stampTrigger("into_school", spawn.x, spawn.y),
      makeTrigger("school_class_slam", "enter", ["lock:school_east_door", "note:Detention."], {
        x: at(east, 6, 6).x,
        y: at(east, 6, 6).y,
        w: 72,
        h: 72,
      }),
      makeTrigger("school_class_clear", "clear", ["unlock:school_east_door", "note:The chalk still says JANE."], {
        watch: classEast.map((e) => e.id),
      }),
    ],
    lights: [{ ...spawn, radius: 22 }, ...torch.lights],
    enemies: classEast,
    props: [
      ...props,
      door("school_east_door", at(east, 0, 6), { lockHint: "The classroom wants you to finish the lesson." }),
    ],
  };
}

/** See the chest across water. Learn Grow in the east. Bridge. North spiders keep the amulet. */
export function generateButterfly(): ZoneBlue {
  const cols = 52;
  const rows = 40;
  const grid = new Grid(cols, rows, "grass");
  paintBorder(grid, "bush");
  const south = room(grid, 20, 28, 12, 10, "grass");
  const hub = room(grid, 20, 16, 12, 12, "grass");
  const east = room(grid, 34, 14, 14, 14, "grass");
  const west = room(grid, 2, 14, 16, 16, "water");
  const north = room(grid, 18, 2, 16, 12, "grass");
  grid.fillRect(west.x + 5, west.y + 5, 5, 5, "grass");
  link(grid, south, hub);
  link(grid, hub, east);
  link(grid, hub, north);
  grid.fillRect(18, 20, 4, 3, "grass");
  grid.fillRect(36, 2, 12, 10, "water");
  grid.fillRect(40, 6, 3, 3, "grass");
  grid.fillRect(38, 12, 4, 3, "grass");

  const spawn = at(south, 6, 8);
  const props: PropSpec[] = [
    exitTo("butterfly_up", "county", at(south, 6, 9)),
    sign("forest_south", at(south, 2, 2), "forest_south"),
    {
      id: "grow_orb",
      kind: "orb",
      ...at(east, 7, 6),
      texture: "orb_ice",
      learn: "grow",
      actions: ["note:Green where there was water. She would have called it gardening."],
    },
    sign("forest_east", at(east, 2, 2), "forest_east"),
    {
      id: "forest_island",
      kind: "chest",
      ...cellWorld(west.x + 7, west.y + 7),
      texture: "chest",
      loot: [
        { id: "honeylace_lily", qty: 2 },
        { id: "small_water", qty: 2 },
      ],
    },
    sign("forest_west", at(hub, 1, 4), "forest_west"),
    {
      id: "forest_amulet",
      kind: "chest",
      ...at(north, 8, 3),
      texture: "chest",
      loot: [{ id: "butterfly_amulet", qty: 1 }],
      actions: ["note:Warm even in shade. The wings are not insects."],
    },
    { id: "forest_torch", kind: "torch", ...at(east, 7, 4), texture: "torch" },
    {
      id: "forest_pool2",
      kind: "chest",
      ...cellWorld(41, 7),
      texture: "chest",
      loot: [
        { id: "apple", qty: 2 },
        { id: "light_stone", qty: 1 },
      ],
    },
  ];

  const spiders = [
    trash("forest_spider_a", "spider", at(north, 4, 6), "forest_north"),
    trash("forest_spider_b", "spider", at(north, 11, 7), "forest_north"),
    trash("forest_plant", "plant", at(north, 7, 9), "forest_north"),
  ];

  return {
    grid,
    spawn,
    required: ["butterfly_up"],
    triggers: [
      stampTrigger("into_butterfly", spawn.x, spawn.y),
      makeTrigger("forest_slam", "enter", ["lock:forest_north_door", "note:The wings close."], {
        x: at(north, 8, 6).x,
        y: at(north, 8, 6).y,
        w: 80,
        h: 72,
      }),
      makeTrigger("forest_clear", "clear", ["unlock:forest_north_door", "note:The north is still."], {
        watch: spiders.map((e) => e.id),
      }),
    ],
    lights: [
      { ...spawn, radius: 22 },
      { ...at(east, 7, 6), radius: 28 },
      { ...at(north, 8, 4), radius: 20 },
    ],
    enemies: spiders,
    props: [
      ...props,
      door("forest_north_door", at(north, 8, 11), { lockHint: "The north wants a fight first." }),
    ],
  };
}

/** See a chest in the wet. Lever in the dry west. Drain. Walk. Rats on the way out. */
export function generatePipes(): ZoneBlue {
  const cols = 50;
  const rows = 34;
  const grid = new Grid(cols, rows, "wall");
  paintBorder(grid);
  const entry = room(grid, 4, 20, 16, 12, "stone");
  const west = room(grid, 4, 4, 16, 12, "stone");
  const flood: FloodSpec = { id: "pipes_east", x: 28, y: 6, w: 18, h: 16 };
  const east = room(grid, flood.x, flood.y, flood.w, flood.h, "stone");
  const rats = room(grid, 22, 20, 14, 10, "stone");
  const flood2: FloodSpec = { id: "pipes_deep", x: 28, y: 24, w: 16, h: 8 };
  const deep = room(grid, flood2.x, flood2.y, flood2.w, flood2.h, "stone");
  link(grid, entry, west);
  link(grid, entry, rats);
  link(grid, rats, east);
  link(grid, rats, deep);
  grid.fillRect(flood.x, flood.y, flood.w, flood.h, "water");
  grid.fillRect(east.x + 7, east.y + 6, 4, 4, "stone");
  grid.fillRect(flood2.x, flood2.y, flood2.w, flood2.h, "water");
  grid.fillRect(deep.x + 6, deep.y + 3, 3, 3, "stone");

  const spawn = at(entry, 8, 9);
  const torch = lightsFor([entry, west, rats], "pipe_torch", 5);

  const props: PropSpec[] = [
    exitTo("pipes_up", "factory", at(entry, 2, 9), "hatch"),
    sign("pipes_entry", at(entry, 4, 2), "pipes_entry"),
    lever("pipes_drain", at(west, 4, 4), [
      "drain:pipes_east",
      "note:The east is a floor. The south is still a lake. The island has a second vote.",
    ]),
    lever("pipes_drain_2", cellWorld(east.x + 6, east.y + 8), [
      "drain:pipes_deep",
      "note:Deeper empties. Castle has more than one wet.",
    ]),
    sign("pipes_west", at(west, 2, 2), "pipes_west"),
    {
      id: "pipes_island",
      kind: "chest",
      ...cellWorld(east.x + 8, east.y + 7),
      texture: "chest",
      loot: [
        { id: "night_lich_moss", qty: 2 },
        { id: "iron", qty: 2 },
      ],
    },
    {
      id: "pipes_deep_chest",
      kind: "chest",
      ...cellWorld(deep.x + 7, deep.y + 4),
      texture: "chest",
      loot: [
        { id: "fire_stone", qty: 1 },
        { id: "coal", qty: 2 },
      ],
    },
    ...barrelPile({ cx: rats.x + 2, cy: rats.y + 2 }, "pipe_crate", 3),
    ...torch.props,
  ];

  const vermin = [
    trash("pipe_rat_a", "rat", at(rats, 4, 5)),
    trash("pipe_rat_b", "rat", at(rats, 8, 6)),
    trash("pipe_rat_c", "rat", at(rats, 10, 3)),
  ];

  return {
    grid,
    spawn,
    required: ["pipes_up"],
    floods: [flood, flood2],
    triggers: [stampTrigger("into_pipes", spawn.x, spawn.y)],
    lights: [{ ...spawn, radius: 18 }, ...torch.lights, { ...cellWorld(east.x + 8, east.y + 7), radius: 16 }],
    enemies: vermin,
    props,
  };
}
