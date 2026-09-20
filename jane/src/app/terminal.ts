// The in-game console. "Keep the terminal. println into an in-game console is how
// you debug an RPG." Commands are rows in one table: add a row, do not grow a
// second language (the 2026 Phaser build spent 1,500 lines on a fake shell).
// Anything that changes the game goes through sim.command({ t: "dev" }), so a
// session that used the console still replays exactly.

import { PARTY_DEALT, PARTY_TAKEN } from "@/sim/constants";
import { hashState } from "@/sim/save";
import type { DevOp } from "@/sim/sim";
import { ZONE_IDS, type ZoneId } from "@/sim/state";
import { maxHp, maxMp } from "@/sim/units";
import type { App } from "@/app/app";

type Row = { usage: string; help: string; run: (app: App, args: string[]) => string[] | Promise<string[]> };

const need = (app: App): NonNullable<ReturnType<App["sim"]>> | null => app.sim();
const NO_GAME = ["No game running."];

const ROWS: Record<string, Row> = {
  help: {
    usage: "help",
    help: "List commands",
    run: () => Object.values(ROWS).map((r) => `${r.usage.padEnd(28)} ${r.help}`),
  },
  give: {
    usage: "give <item> [qty]",
    help: "Add items to the bag",
    run: (app, [item, qty]) => {
      const sim = need(app);
      if (!sim) return NO_GAME;
      if (!sim.catalog.items[item]) return [`No item "${item}". Try: ${Object.keys(sim.catalog.items).slice(0, 8).join(" ")} ...`];
      app.command({ t: "dev", dev: { op: "give", item, qty: Math.max(1, Number(qty) || 1) } });
      return [`+${Math.max(1, Number(qty) || 1)} ${sim.catalog.items[item].name}`];
    },
  },
  god: {
    usage: "god [on|off]",
    help: "No damage, double speed, enemies ignore you",
    run: (app, [v]) => {
      const sim = need(app);
      if (!sim) return NO_GAME;
      const on = v ? v === "on" : !sim.me.god;
      app.command({ t: "dev", dev: { op: "god", on } });
      return [`god ${on ? "on" : "off"}`];
    },
  },
  tp: {
    usage: "tp <zone> [mark]",
    help: `Travel. Zones: ${ZONE_IDS.join(" ")}`,
    run: (app, [zone, mark]) => {
      const sim = need(app);
      if (!sim) return NO_GAME;
      if (!ZONE_IDS.includes(zone as ZoneId)) return [`Zones: ${ZONE_IDS.join(" ")}`];
      app.command({ t: "dev", dev: { op: "tp", zone: zone as ZoneId, mark: mark ?? "start" } });
      return [`-> ${sim.rt.bp.name}. Marks: ${Object.keys(sim.rt.bp.marks).join(" ")}`];
    },
  },
  time: {
    usage: "time <hour>",
    help: "Set the clock, 0-24",
    run: (app, [h]) => dev(app, { op: "time", hour: Number(h) || 0 }, `time ${Number(h) || 0}:00`),
  },
  hp: { usage: "hp [value]", help: "Set health (default: full)", run: (app, [v]) => dev(app, { op: "hp", value: Number(v) || 1e9 }, "hp set") },
  mp: { usage: "mp [value]", help: "Set mana (default: full)", run: (app, [v]) => dev(app, { op: "mp", value: Number(v) || 1e9 }, "mp set") },
  learn: {
    usage: "learn <spell>",
    help: "Add a spell to the book",
    run: (app, [spell]) => {
      const sim = need(app);
      if (!sim) return NO_GAME;
      if (!sim.catalog.spells[spell]) return [`Spells: ${Object.keys(sim.catalog.spells).join(" ")}`];
      return dev(app, { op: "learn", spell }, `learned ${spell}`);
    },
  },
  quest: {
    usage: "quest [id]",
    help: "Give a quest, or list the log",
    run: (app, [id]) => {
      const sim = need(app);
      if (!sim) return NO_GAME;
      if (!id) return [`active: ${sim.state.quests.active.map((q) => q.quest).join(" ") || "-"}`, `done: ${sim.state.quests.done.join(" ") || "-"}`];
      if (!sim.catalog.quests[id]) return [`Quests: ${Object.keys(sim.catalog.quests).join(" ")}`];
      return dev(app, { op: "quest", quest: id }, `quest ${id}`);
    },
  },
  flag: {
    usage: "flag [name] [value]",
    help: "Set a flag, or list flags",
    run: (app, [name, value]) => {
      const sim = need(app);
      if (!sim) return NO_GAME;
      if (!name) return Object.entries(sim.state.flags).map(([k, v]) => `${k} = ${v}`);
      return dev(app, { op: "flag", flag: name, value: value === undefined ? 1 : Number(value) }, `${name} set`);
    },
  },
  kill: { usage: "kill", help: "Kill every hostile within 25 m", run: (app) => dev(app, { op: "kill" }, "done") },
  spawn: {
    usage: "spawn <unit>",
    help: "Spawn a unit row next to you",
    run: (app, [def]) => {
      const sim = need(app);
      if (!sim) return NO_GAME;
      if (!sim.catalog.units[def]) return [`Units: ${Object.keys(sim.catalog.units).join(" ")}`];
      return dev(app, { op: "spawn", def }, `spawned ${def}`);
    },
  },
  // Storage is asynchronous: these two answer when the slot has really been written or read.
  save: { usage: "save [slot 1-3]", help: "Write a save slot", run: async (app, [s]) => [(await app.save(slotArg(s))) ? "saved" : "save failed"] },
  load: { usage: "load [slot 1-3]", help: "Read a save slot", run: async (app, [s]) => [(await app.load(slotArg(s))) ? "loaded" : "nothing to load"] },
  seed: {
    usage: "seed [n]",
    help: "Show the run seed, or start a new run on seed n",
    run: (app, [n]) => {
      if (n !== undefined) {
        app.newGame(Number(n) >>> 0);
        return [`new run, seed ${Number(n) >>> 0}`];
      }
      const sim = need(app);
      return sim ? [`seed ${sim.state.seed}`] : NO_GAME;
    },
  },
  hash: {
    usage: "hash",
    help: "State hash: equal seeds + equal inputs must give equal hashes",
    run: (app) => {
      const sim = need(app);
      return sim ? [`tick ${sim.state.tick}  hash ${hashState(sim.state)}`] : NO_GAME;
    },
  },
  pos: {
    usage: "pos",
    help: "Where am I",
    run: (app) => {
      const sim = need(app);
      if (!sim) return NO_GAME;
      const p = sim.player;
      return [`${sim.me.zone} px${p.x.toFixed(1)},${p.y.toFixed(1)} cell ${Math.floor(p.x / 8)},${Math.floor(p.y / 8)} hp ${Math.round(p.hp)}/${maxHp(p)} mp ${Math.round(p.mp)}/${maxMp(p)}`];
    },
  },
  inst: {
    usage: "inst count",
    help: "Instance counts (2020's `inst count`)",
    run: (app) => {
      const sim = need(app);
      if (!sim) return NO_GAME;
      const z = sim.zone;
      const awake = z.units.filter((u) => u.awake).length;
      return [`units ${z.units.length} (awake ${awake})  props ${z.props.length}  drops ${z.drops.length}  bolts ${z.projectiles.length}  grounds ${z.grounds.length}`];
    },
  },
  speed: {
    usage: "speed <x>",
    help: "Time scale, 0.1-8 (2020's F2 fast-forward)",
    run: (app, [x]) => {
      app.setTimeScale(Math.max(0.1, Math.min(8, Number(x) || 1)));
      return [`speed ${Math.max(0.1, Math.min(8, Number(x) || 1))}`];
    },
  },
  replay: {
    usage: "replay [verify|export]",
    help: "Re-simulate this session and compare hashes, or download it",
    run: (app, [mode]) => app.replay(mode ?? "verify"),
  },
  party: {
    usage: "party",
    help: "Who is sitting at the table, where, and what the head count costs",
    run: (app) => {
      const sim = need(app);
      if (!sim) return NO_GAME;
      const n = sim.party.size();
      const coats = ["plum", "teal", "moss", "ochre"];
      const rows = sim.state.players.map((p) => `seat ${p.index + 1}  ${coats[p.index].padEnd(6)} ${p.who.padEnd(12)} ${p.connected ? p.zone : "(away)"}`);
      return [...rows, `the world is ${sim.state.open ? "open" : "closed"}`, `${n} connected: each deals ${Math.round(PARTY_DEALT[n - 1] * 100)}%, takes ${Math.round(PARTY_TAKEN[n - 1] * 100)}%`];
    },
  },
  open: {
    usage: "open | close",
    help: "Let others sit down in this world, or stop letting them. A save always loads closed",
    run: (app) => (app.sim() ? (app.seatCommand(0, { t: "open", on: true }), ["The world is open. join <who> sits someone down."]) : NO_GAME),
  },
  close: {
    usage: "close",
    help: "Stop letting anyone new sit down. Nobody is sent home",
    run: (app) => (app.sim() ? (app.seatCommand(0, { t: "open", on: false }), ["The world is closed."]) : NO_GAME),
  },
  join: {
    usage: "join [who]",
    help: "Sit another body down at the party's fire (it just stands there). For feeling the co-op penalty before there is a network. The same [who] gets her bags back",
    run: (app, [who]) => {
      const sim = need(app);
      if (!sim) return NO_GAME;
      if (!sim.state.open) return ["The world is closed. Type open first."];
      const seat = app.seatCommand(-1, { t: "join", who: who ?? `guest${sim.state.players.length + 1}` });
      return [seat < 0 ? "All four seats are taken." : `seat ${seat + 1} taken`];
    },
  },
  leave: {
    usage: "leave <seat>",
    help: "Stand a guest up again (seats 2-4)",
    run: (app, [s]) => {
      const seat = Number(s) - 1;
      if (!app.sim() || !(seat >= 1 && seat <= 3)) return ["leave 2, 3 or 4"];
      return [app.seatCommand(seat, { t: "leave" }) < 0 ? "Nobody is sitting there." : `seat ${seat + 1} is empty`];
    },
  },
  ver: { usage: "ver", help: "Build version", run: (app) => [app.version] },
  title: {
    usage: "title",
    help: "Quit to the title screen",
    run: (app) => {
      app.toTitle();
      return ["bye"];
    },
  },
};

function dev(app: App, op: DevOp, ok: string): string[] {
  if (!app.sim()) return NO_GAME;
  app.command({ t: "dev", dev: op });
  return [ok];
}

function slotArg(s: string | undefined): number {
  const n = Number(s);
  return Number.isInteger(n) && n >= 1 && n <= 3 ? n - 1 : 0;
}

export function runTerminal(app: App, line: string): string[] | Promise<string[]> {
  const parts = line.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return [];
  const row = ROWS[parts[0].toLowerCase()];
  if (!row) return [`Unknown command "${parts[0]}". Type help.`];
  try {
    return row.run(app, parts.slice(1));
  } catch (err) {
    return [`error: ${err instanceof Error ? err.message : String(err)}`];
  }
}
