import { SAVE_KEY } from "@/game/constants";
import {
  beatSpine,
  effectCatalog,
  itemCatalog,
  questCatalog,
} from "@/game/systems/catalog";
import { inventoryAdd } from "@/game/systems/inventory";
import { session } from "@/game/systems/session";
import { applyStatus } from "@/game/systems/status";
import { bannerLines, paint, printError, println, stripColor, tClear, termLines, TERM_VERSION } from "@/game/systems/termLog";
import {
  buildFs,
  completePath,
  formatPath,
  listNames,
  resolvePath,
  walk,
  type VNode,
} from "@/game/systems/termFs";
import type { ZoneId } from "@/game/types";

export type TermCtx = {
  cwd: string[];
  setCwd: (path: string[]) => void;
  history: readonly string[];
  aliases: Record<string, string>;
  setAlias: (name: string, value: string) => void;
  close: () => void;
  names: () => string[];
  lookup: (name: string) => TermCommand | undefined;
};

export type TermCommand = {
  name: string;
  aliases?: string[];
  usage: string;
  summary: string;
  group: "console" | "world" | "shell";
  run: (args: string[], ctx: TermCtx) => void;
  complete?: (args: string[], ctx: TermCtx) => string[];
};

const ZONES: ZoneId[] = [
  "county",
  "house",
  "dungeon",
  "burial",
  "mine",
  "abandoned",
  "museum",
  "graveyard",
  "factory",
  "school",
  "butterfly",
  "pipes",
];

const DUMP_KIT: [string, number][] = [
  ["gold_bar", 4],
  ["rock", 4],
  ["small_water", 16],
  ["hemshade_root", 4],
  ["white_water_cap", 4],
  ["night_lich_moss", 4],
  ["savage_snakeroot", 4],
  ["honeylace_lily", 4],
  ["nasturtium", 4],
  ["pansy", 4],
  ["white_water_rose", 4],
];

export const TERM_COMMANDS: TermCommand[] = [
  {
    name: "help",
    aliases: ["man", "?"],
    usage: "help [command]",
    summary: "list commands, or one usage line",
    group: "console",
    run: (args, ctx) => {
      if (args[0]) {
        const cmd = ctx.lookup(args[0]);
        if (!cmd) {
          printError(`help: no help for '${args[0]}'`);
          return;
        }
        println(`${paint("yellow", cmd.usage)}`);
        println(`  ${cmd.summary}`);
        if (cmd.aliases?.length) println(`  aliases: ${cmd.aliases.join(", ")}`);
        return;
      }
      println(paint("ltcyan", "-- console --"));
      printGroup(ctx, "console");
      println(paint("ltcyan", "-- world --"));
      printGroup(ctx, "world");
      println(paint("ltcyan", "-- shell --"));
      printGroup(ctx, "shell");
      println(`${paint("gray", "tab complete · ↑↓ history · pgup/pgdn scroll · esc close")}`);
    },
    complete: (args, ctx) => prefix(ctx.names(), args[0] ?? ""),
  },
  {
    name: "cmdlist",
    usage: "cmdlist",
    summary: "quake-style name dump",
    group: "console",
    run: (_args, ctx) => {
      println(ctx.names().join("  "));
    },
  },
  {
    name: "ver",
    usage: "ver",
    summary: "print version banner",
    group: "console",
    run: () => {
      for (const line of bannerLines()) println(line);
    },
  },
  {
    name: "clear",
    aliases: ["cls"],
    usage: "clear",
    summary: "wipe the scrollback",
    group: "console",
    run: () => tClear(),
  },
  {
    name: "exit",
    usage: "exit",
    summary: "close the console (leave the shell)",
    group: "console",
    run: (_args, ctx) => ctx.close(),
  },
  {
    name: "quit",
    usage: "quit",
    summary: "return to title (quake quit)",
    group: "console",
    run: (_args, ctx) => {
      if (!session.started) {
        println("already at boot");
        return;
      }
      session.wantQuit = true;
      println("quitting to title");
      ctx.close();
    },
  },
  {
    name: "restart",
    usage: "restart",
    summary: "soft game_restart — back to title, RAM wiped, slot kept",
    group: "console",
    run: (_args, ctx) => {
      if (!session.started) {
        println("already at boot");
        return;
      }
      session.wantRestart = true;
      println("restarting");
      ctx.close();
    },
  },
  {
    name: "clear_config",
    usage: "clear_config",
    summary: "delete slot 0 and return to title",
    group: "console",
    run: (_args, ctx) => {
      localStorage.removeItem(SAVE_KEY);
      session.wantRestart = true;
      println("cleared slot 0");
      ctx.close();
    },
  },
  {
    name: "debug",
    usage: "debug [on|off]",
    summary: "F2 overlay",
    group: "console",
    run: (args) => {
      const op = args[0]?.toLowerCase();
      if (op === "on") session.debug = true;
      else if (op === "off") session.debug = false;
      else if (!op) session.debug = !session.debug;
      else {
        printError(`debug: '${args[0]}' invalid operand`);
        return;
      }
      println(session.debug ? "debug mode ON" : "debug mode OFF");
    },
    complete: (args) => prefix(["on", "off"], args[0] ?? ""),
  },
  {
    name: "inst",
    usage: "inst count|list",
    summary: "live world census",
    group: "console",
    run: (args) => {
      const op = args[0]?.toLowerCase() ?? "";
      const world = session.inspectWorld?.();
      if (!op) {
        printError("inst: missing operand");
        return;
      }
      if (op !== "count" && op !== "list") {
        printError(`inst: '${args[0]}' invalid operand`);
        return;
      }
      if (!world) {
        println("current instance count = 0");
        println("  no world (title)");
        return;
      }
      const units = world.actors.length + 1;
      const total = units + world.props.length + world.bolts;
      println(`current instance count = ${total}`);
      println(
        `  units ${units}  props ${world.props.length}  bolts ${world.bolts}  asleep ${world.asleep}  chunks ${world.chunks}`,
      );
      if (op === "list") {
        println(paint("yellow", "actors"));
        for (const a of world.actors) {
          const sleep = a.asleep ? " sleep" : "";
          const dead = a.alive ? "" : " dead";
          println(`  ${a.id}  ${a.kind}  ${Math.round(a.x)},${Math.round(a.y)}  ${Math.round(a.hp)}/${a.maxhp}${dead}${sleep}`);
        }
        println(paint("yellow", "props"));
        for (const p of world.props) {
          println(`  ${p.id}  ${p.kind}  ${Math.round(p.x)},${Math.round(p.y)}`);
        }
      }
    },
    complete: (args) => prefix(["count", "list"], args[0] ?? ""),
  },
  {
    name: "echo",
    usage: "echo [text]",
    summary: "print a line (color tags ok)",
    group: "console",
    run: (args) => println(args.join(" ")),
  },
  {
    name: "find",
    aliases: ["grep"],
    usage: "find <text>",
    summary: "search the scrollback",
    group: "console",
    run: (args) => {
      const q = args.join(" ").toLowerCase();
      if (!q) {
        printError("find: missing operand");
        return;
      }
      const hits = termLines().filter((line) => stripColor(line).toLowerCase().includes(q));
      if (!hits.length) {
        println("no matches");
        return;
      }
      for (const line of hits.slice(-40)) println(line);
      if (hits.length > 40) println(paint("gray", `… ${hits.length - 40} older hits`));
    },
  },
  {
    name: "condump",
    usage: "condump",
    summary: "copy scrollback to the clipboard",
    group: "console",
    run: () => {
      const text = termLines().map(stripColor).join("\n");
      void navigator.clipboard.writeText(text).then(
        () => println(`copied ${termLines().length} lines`),
        () => printError("condump: clipboard blocked"),
      );
    },
  },
  {
    name: "history",
    usage: "history",
    summary: "typed commands this session",
    group: "console",
    run: (_args, ctx) => {
      ctx.history.forEach((line, i) => println(`  ${i + 1}  ${line}`));
    },
  },
  {
    name: "alias",
    usage: "alias [name=command]",
    summary: "user aliases (saved)",
    group: "console",
    run: (args, ctx) => {
      if (!args[0]) {
        const keys = Object.keys(ctx.aliases);
        if (!keys.length) {
          println("no aliases");
          return;
        }
        for (const key of keys) println(`  ${key}=${ctx.aliases[key]}`);
        return;
      }
      const joined = args.join(" ");
      const eq = joined.indexOf("=");
      if (eq < 1) {
        printError("alias: use name=command");
        return;
      }
      const name = joined.slice(0, eq).trim().toLowerCase();
      const value = joined.slice(eq + 1).trim();
      if (!value) {
        printError("alias: empty command");
        return;
      }
      ctx.setAlias(name, value);
      println(`${name}=${value}`);
    },
  },
  {
    name: "lua",
    usage: "lua",
    summary: "2020 lua state — not linked",
    group: "console",
    run: () => {
      println(`---- ${paint("ltred", "GMD2 ")}${paint("magenta", "Lua")}${paint("white", " ----")}`);
      println("lua runtime not linked. cat, set, and give are the remake.");
    },
  },
  {
    name: "god",
    usage: "god",
    summary: "toggle invuln",
    group: "world",
    run: () => {
      session.god = !session.god;
      println(session.god ? paint("ltgreen", "god on") : paint("ltred", "god off"));
    },
  },
  {
    name: "give",
    usage: "give <item> [qty]",
    summary: "put items in the bag",
    group: "world",
    run: (args) => {
      const id = args[0];
      if (!id) {
        printError("give: missing operand");
        return;
      }
      if (!itemCatalog[id]) {
        printError(`give: '${id}' no such item`);
        return;
      }
      if (!session.started) {
        printError("give: no player");
        return;
      }
      const qty = Number(args[1] ?? 1);
      if (!Number.isFinite(qty) || qty <= 0) {
        printError("give: bad qty");
        return;
      }
      const left = inventoryAdd(session.player, id, qty);
      session.quests.syncAcquire(session.player);
      println(left ? `gave, leftover ${left}` : `gave ${id} x${qty}`);
    },
    complete: (args) => prefix(Object.keys(itemCatalog), args[0] ?? ""),
  },
  {
    name: "dump",
    usage: "dump",
    summary: "town dump-chest kit (designer give-all)",
    group: "world",
    run: () => {
      if (!session.started) {
        printError("dump: no player");
        return;
      }
      let leftover = 0;
      for (const [id, qty] of DUMP_KIT) leftover += inventoryAdd(session.player, id, qty);
      session.quests.syncAcquire(session.player);
      println(leftover ? `dump kit, leftover ${leftover}` : "dump kit given");
    },
  },
  {
    name: "time",
    usage: "time [hour]",
    summary: "read or set the day clock (0–23)",
    group: "world",
    run: (args) => {
      if (!args[0]) {
        println(`time ${session.time.toFixed(2)}`);
        return;
      }
      const hour = Number(args[0]);
      if (!Number.isFinite(hour)) {
        printError("time: bad hour");
        return;
      }
      session.time = ((hour % 24) + 24) % 24;
      println(`time ${session.time}`);
    },
  },
  {
    name: "quest",
    usage: "quest <id>",
    summary: "acquire a quest row",
    group: "world",
    run: (args) => {
      const id = args[0];
      if (!id) {
        printError("quest: missing operand");
        return;
      }
      if (!questCatalog[id]) {
        printError(`quest: '${id}' no such quest`);
        return;
      }
      const ok = session.quests.acquire(id);
      println(ok ? `quest ${id}` : `quest ${id} already held`);
    },
    complete: (args) => prefix(Object.keys(questCatalog), args[0] ?? ""),
  },
  {
    name: "beat",
    usage: "beat <id>",
    summary: "grant a story beat",
    group: "world",
    run: (args) => {
      const id = args[0];
      if (!id) {
        printError("beat: missing operand");
        return;
      }
      if (!beatSpine.some((b) => b.id === id)) {
        printError(`beat: '${id}' no such beat`);
        return;
      }
      session.beats.grant(id);
      println(`beat ${id}`);
    },
    complete: (args) => prefix(beatSpine.map((b) => b.id), args[0] ?? ""),
  },
  {
    name: "tp",
    aliases: ["map"],
    usage: "tp <zone>",
    summary: "travel: county house dungeon burial mine farm museum…",
    group: "world",
    run: (args) => {
      const id = args[0] as ZoneId | undefined;
      if (!id) {
        printError("tp: missing operand");
        return;
      }
      if (!ZONES.includes(id)) {
        printError(`tp: '${args[0]}' no such zone`);
        return;
      }
      if (!session.started) {
        printError("tp: no world");
        return;
      }
      session.pendingZone = id;
      println(`tp ${id}`);
    },
    complete: (args) => prefix(ZONES, args[0] ?? ""),
  },
  {
    name: "save",
    usage: "save",
    summary: "write slot 0",
    group: "world",
    run: () => {
      if (!session.started) {
        printError("save: no world");
        return;
      }
      session.wantSave = true;
      println("saved");
    },
  },
  {
    name: "mp",
    usage: "mp",
    summary: "fill mana",
    group: "world",
    run: () => {
      if (!session.started) {
        printError("mp: no player");
        return;
      }
      session.player.mp = session.player.maxmp;
      println("mp full");
    },
  },
  {
    name: "hp",
    usage: "hp",
    summary: "fill health",
    group: "world",
    run: () => {
      if (!session.started) {
        printError("hp: no player");
        return;
      }
      session.player.hp = session.player.maxhp;
      session.player.alive = true;
      println("hp full");
    },
  },
  {
    name: "effect",
    usage: "effect <id>",
    summary: "apply a status row to the player",
    group: "world",
    run: (args) => {
      const id = args[0];
      if (!id) {
        printError("effect: missing operand");
        return;
      }
      if (!effectCatalog[id]) {
        printError(`effect: '${id}' no such effect`);
        return;
      }
      if (!session.started) {
        printError("effect: no player");
        return;
      }
      applyStatus(session.player, id);
      println(`effect ${id}`);
    },
    complete: (args) => prefix(Object.keys(effectCatalog), args[0] ?? ""),
  },
  {
    name: "set",
    usage: "set [flag] [value]",
    summary: "read or write session flags",
    group: "world",
    run: (args) => {
      if (!args[0]) {
        const keys = Object.keys(session.flags);
        if (!keys.length) {
          println("no flags");
          return;
        }
        for (const key of keys) println(`  ${key}=${String(session.flags[key])}`);
        return;
      }
      if (args[1] === undefined) {
        println(`${args[0]}=${String(session.flags[args[0]] ?? "")}`);
        return;
      }
      const raw = args.slice(1).join(" ");
      const value = raw === "true" ? true : raw === "false" ? false : Number.isFinite(Number(raw)) ? Number(raw) : raw;
      session.setFlag(args[0], value);
      println(`${args[0]}=${String(value)}`);
    },
    complete: (args) => prefix(Object.keys(session.flags), args[0] ?? ""),
  },
  {
    name: "status",
    aliases: ["inv"],
    usage: "status",
    summary: "player vitals and bag",
    group: "world",
    run: () => {
      const p = session.player;
      const world = session.inspectWorld?.();
      println(
        `${p.name}  ${session.zone}  hp ${Math.round(p.hp)}/${p.maxhp}  mp ${Math.round(p.mp)}/${p.maxmp}  en ${Math.round(p.energy)}`,
      );
      if (world) println(`xy ${Math.round(world.player.x)},${Math.round(world.player.y)}`);
      println(session.beats.debugLine());
      for (const slot of p.inventory) {
        if (slot.id) println(`  ${slot.id} x${slot.qty}`);
      }
    },
  },
  {
    name: "pwd",
    usage: "pwd",
    summary: "print working directory",
    group: "shell",
    run: (_args, ctx) => println(formatPath(ctx.cwd)),
  },
  {
    name: "cd",
    usage: "cd [path]",
    summary: "change directory",
    group: "shell",
    run: (args, ctx) => {
      const dest = args[0] ?? "\\";
      const parts = resolvePath(ctx.cwd, dest);
      if (!parts) {
        printError(`cd: '${dest}'`);
        return;
      }
      const node = walk(fs(ctx), parts);
      if (!node) {
        printError(`No directory '${paint("magenta", dest)}'`);
        return;
      }
      if (node.kind !== "dir") {
        printError(`cd: '${dest}' not a directory`);
        return;
      }
      ctx.setCwd(parts);
    },
    complete: (args, ctx) => completePath(ctx.cwd, args[0] ?? "", ctx.names()),
  },
  {
    name: "ls",
    aliases: ["dir"],
    usage: "ls [-l] [path]",
    summary: "list catalog / proc / bin",
    group: "shell",
    run: (args, ctx) => {
      const long = args[0] === "-l";
      const pathArg = long ? args[1] : args[0];
      const parts = pathArg ? resolvePath(ctx.cwd, pathArg) : [...ctx.cwd];
      if (!parts) {
        printError(`ls: '${pathArg}'`);
        return;
      }
      const node = walk(fs(ctx), parts);
      if (!node) {
        printError(`ls: '${pathArg ?? formatPath(ctx.cwd)}' not found`);
        return;
      }
      if (node.kind === "file") {
        println(pathArg ?? formatPath(ctx.cwd));
        return;
      }
      const names = listNames(node);
      if (!names.length) {
        println(paint("gray", "(empty)"));
        return;
      }
      if (long) {
        for (const name of names) {
          const kid = node.kids[name];
          const mark = kid.kind === "dir" ? "d" : "-";
          const size = kid.kind === "file" ? String(kid.text().length).padStart(5) : "    -";
          println(`${mark}  ${size}  ${kid.kind === "dir" ? paint("ltcyan", name + "\\") : name}`);
        }
        return;
      }
      println(
        names
          .map((name) => (node.kids[name].kind === "dir" ? paint("ltcyan", name + "\\") : name))
          .join("  "),
      );
    },
    complete: (args, ctx) => {
      const start = args[0] === "-l" ? args[1] ?? "" : args[0] ?? "";
      return completePath(ctx.cwd, start, ctx.names());
    },
  },
  {
    name: "cat",
    aliases: ["type", "more"],
    usage: "cat <path>",
    summary: "print a virtual file",
    group: "shell",
    run: (args, ctx) => {
      const pathArg = args[0];
      if (!pathArg) {
        printError("cat: missing operand");
        return;
      }
      const parts = resolvePath(ctx.cwd, pathArg);
      if (!parts) {
        printError(`cat: '${pathArg}'`);
        return;
      }
      const node = walk(fs(ctx), parts);
      if (!node) {
        printError(`cat: '${pathArg}' not found`);
        return;
      }
      if (node.kind === "dir") {
        printError(`cat: '${pathArg}' is a directory`);
        return;
      }
      println(node.text());
    },
    complete: (args, ctx) => completePath(ctx.cwd, args[0] ?? "", ctx.names()),
  },
  {
    name: "tree",
    usage: "tree [path]",
    summary: "walk a directory two deep",
    group: "shell",
    run: (args, ctx) => {
      const parts = args[0] ? resolvePath(ctx.cwd, args[0]) : [...ctx.cwd];
      if (!parts) {
        printError(`tree: '${args[0]}'`);
        return;
      }
      const node = walk(fs(ctx), parts);
      if (!node) {
        printError(`tree: not found`);
        return;
      }
      println(formatPath(parts));
      walkTree(node, "", 0, 2);
    },
    complete: (args, ctx) => completePath(ctx.cwd, args[0] ?? "", ctx.names()),
  },
  {
    name: "which",
    usage: "which <command>",
    summary: "resolve a command or alias",
    group: "shell",
    run: (args, ctx) => {
      const name = args[0]?.toLowerCase();
      if (!name) {
        printError("which: missing operand");
        return;
      }
      if (ctx.aliases[name]) {
        println(`${name}: alias ${ctx.aliases[name]}`);
        return;
      }
      const cmd = ctx.lookup(name);
      if (!cmd) {
        printError(`which: '${name}' not found`);
        return;
      }
      println(`\\bin\\${cmd.name}`);
    },
    complete: (args, ctx) => prefix(ctx.names(), args[0] ?? ""),
  },
  {
    name: "whoami",
    usage: "whoami",
    summary: "player @ zone",
    group: "shell",
    run: () => println(`${session.player.name}@${session.zone}`),
  },
  {
    name: "uname",
    usage: "uname",
    summary: "runtime string",
    group: "shell",
    run: () => println(`JUNQI/web ${TERM_VERSION}`),
  },
  {
    name: "date",
    usage: "date",
    summary: "real clock and day clock",
    group: "shell",
    run: () => println(`${new Date().toLocaleString()}  game ${session.time.toFixed(2)}h`),
  },
];

function fs(ctx: TermCtx): VNode {
  return buildFs(ctx.names());
}

function printGroup(ctx: TermCtx, group: TermCommand["group"]): void {
  const seen = new Set<string>();
  for (const name of ctx.names()) {
    const cmd = ctx.lookup(name);
    if (!cmd || cmd.group !== group || seen.has(cmd.name)) continue;
    seen.add(cmd.name);
    println(`  ${paint("yellow", cmd.name.padEnd(14))} ${cmd.summary}`);
  }
}

function prefix(pool: string[], start: string): string[] {
  const q = start.toLowerCase();
  return pool.filter((n) => n.toLowerCase().startsWith(q));
}

function walkTree(node: VNode, indent: string, depth: number, max: number): void {
  if (node.kind !== "dir" || depth >= max) return;
  const names = listNames(node);
  names.forEach((name, i) => {
    const last = i === names.length - 1;
    const kid = node.kids[name];
    const branch = last ? "└─ " : "├─ ";
    const label = kid.kind === "dir" ? paint("ltcyan", name + "\\") : name;
    println(indent + branch + label);
    if (kid.kind === "dir") walkTree(kid, indent + (last ? "   " : "│  "), depth + 1, max);
  });
}
