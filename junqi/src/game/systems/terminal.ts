import { TERM_COMMANDS, type TermCommand, type TermCtx } from "@/game/systems/termCommands";
import { formatPath } from "@/game/systems/termFs";
import { paint, printBanner, printError, println } from "@/game/systems/termLog";

export { println, printError, tClear, paint, termLines, onTermChange, lineToHtml, stripColor } from "@/game/systems/termLog";
export type { WorldInspect } from "@/game/systems/session";

const HISTORY_KEY = "junqi.term.history";
const ALIAS_KEY = "junqi.term.aliases";
const HISTORY_CAP = 80;

const byName = new Map<string, TermCommand>();
for (const cmd of TERM_COMMANDS) {
  byName.set(cmd.name, cmd);
  for (const alias of cmd.aliases ?? []) byName.set(alias, cmd);
}

let cwd: string[] = [];
const history: string[] = loadHistory();
const aliases: Record<string, string> = loadAliases();
let closeHook = (): void => { };
let booted = false;

export function setTermClose(fn: () => void): void {
  closeHook = fn;
}

export function termCwd(): string {
  return formatPath(cwd);
}

export function termHistory(): readonly string[] {
  return history;
}

export function bootTerminal(): void {
  if (booted) return;
  booted = true;
  printBanner();
}

export function runLine(line: string): void {
  const trimmed = line.trim();
  if (!trimmed) return;
  println(`${paint("ltred", termCwd() + " ")}${trimmed}`);
  if (trimmed === "!!") {
    const last = history[history.length - 1];
    if (!last) {
      printError("!!: no history");
      return;
    }
    runLine(last);
    return;
  }
  pushHistory(trimmed);
  for (const chunk of trimmed.split(";")) execOne(chunk);
}

export function completeLine(line: string): { insert: string; options: string[] } {
  const leading = line.match(/^\s*/)?.[0] ?? "";
  const body = line.slice(leading.length);
  const parts = tokenize(body);
  const trailingSpace = /\s$/.test(line);
  const ctx = makeCtx();

  if (!parts.length || (parts.length === 1 && !trailingSpace)) {
    const start = parts[0] ?? "";
    const options = unique([
      ...commandNames().filter((n) => n.startsWith(start.toLowerCase())),
      ...Object.keys(aliases).filter((n) => n.startsWith(start.toLowerCase())),
    ]);
    return finishComplete(leading, [], options, start);
  }

  const cmd = lookup(parts[0] ?? "");
  if (!cmd?.complete) return { insert: line, options: [] };
  const args = parts.slice(1);
  const partial = trailingSpace ? "" : (args.pop() ?? "");
  const options = cmd.complete(trailingSpace ? [...args, ""] : [...args, partial], ctx);
  return finishComplete(leading, [parts[0] ?? "", ...args], options, partial);
}

function finishComplete(
  leading: string,
  head: string[],
  options: string[],
  partial: string,
): { insert: string; options: string[] } {
  if (!options.length) return { insert: leading + join(head, partial), options: [] };
  const common = commonPrefix(options);
  const insert = leading + join(head, common || partial);
  return { insert, options };
}

function join(head: string[], tail: string): string {
  if (!head.length) return tail;
  return tail ? `${head.join(" ")} ${tail}` : `${head.join(" ")} `;
}

function execOne(raw: string): void {
  const tokens = tokenize(raw);
  if (!tokens.length) return;
  const expanded = expandAlias(tokens, 0);
  if (!expanded) return;
  const [name, ...args] = expanded;
  const cmd = lookup(name ?? "");
  if (!cmd) {
    printError(`'${name}' command not found`);
    return;
  }
  cmd.run(args, makeCtx());
}

function expandAlias(tokens: string[], depth: number): string[] | null {
  if (depth > 8) {
    printError("alias: loop");
    return null;
  }
  const head = tokens[0]?.toLowerCase() ?? "";
  const alias = aliases[head];
  if (!alias) return tokens;
  const next = [...tokenize(alias), ...tokens.slice(1)];
  if (next[0]?.toLowerCase() === head) return next;
  return expandAlias(next, depth + 1);
}

function tokenize(line: string): string[] {
  const cut = line.split("#")[0] ?? "";
  return cut.trim().split(/\s+/).filter(Boolean);
}

function lookup(name: string): TermCommand | undefined {
  return byName.get(name.toLowerCase());
}

function commandNames(): string[] {
  return unique(TERM_COMMANDS.flatMap((c) => [c.name, ...(c.aliases ?? [])])).sort();
}

function makeCtx(): TermCtx {
  return {
    cwd,
    setCwd: (path) => {
      cwd = path;
    },
    history,
    aliases,
    setAlias: (name, value) => {
      aliases[name] = value;
      localStorage.setItem(ALIAS_KEY, JSON.stringify(aliases));
    },
    close: () => closeHook(),
    names: commandNames,
    lookup,
  };
}

function pushHistory(line: string): void {
  if (history[history.length - 1] === line) return;
  history.push(line);
  if (history.length > HISTORY_CAP) history.splice(0, history.length - HISTORY_CAP);
  localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
}

function loadHistory(): string[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    const parsed = raw ? (JSON.parse(raw) as unknown) : [];
    return Array.isArray(parsed) ? parsed.filter((x): x is string => typeof x === "string") : [];
  } catch {
    return [];
  }
}

function loadAliases(): Record<string, string> {
  try {
    const raw = localStorage.getItem(ALIAS_KEY);
    const parsed = raw ? (JSON.parse(raw) as unknown) : {};
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, string>)
      : {};
  } catch {
    return {};
  }
}

function unique(items: string[]): string[] {
  return [...new Set(items)];
}

function commonPrefix(items: string[]): string {
  if (!items.length) return "";
  let prefix = items[0] ?? "";
  for (const item of items) {
    let i = 0;
    while (i < prefix.length && i < item.length && prefix[i]?.toLowerCase() === item[i]?.toLowerCase()) i += 1;
    prefix = prefix.slice(0, i);
  }
  return prefix;
}
