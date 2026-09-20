// Save codec. There is no "save schema": a save is GameState, versioned.
// 2020's save_room wrote maxhp where it meant hp and reloaded units at their
// start position. Those bugs cannot happen here because nothing is copied
// field-by-field; what the sim runs on is what gets written.

import { SAVE_VERSION } from "@/sim/constants";
import { hashString } from "@/sim/rng";
import type { GameState } from "@/sim/state";

export type SaveFile = {
  format: "jane-save";
  version: number;
  savedAt: string; // ISO; presentation only, never read by the sim
  summary: { zone: string; day: number; hour: number; hp: number; maxhp: number };
  state: GameState;
};

type Migration = (state: Record<string, unknown>) => void;

/** `MIGRATIONS[n]` upgrades a version-n state to n+1 in place. */
const MIGRATIONS: Record<number, Migration> = {
  // v1 -> v2: the player got a name and a rest point; units got `hidden` and `pathGoal`.
  1: (state) => {
    state.playerName ??= "Jane";
    state.rest ??= null;
    const zones = (state.zones ?? {}) as Record<string, { units?: Record<string, unknown>[] }>;
    for (const zone of Object.values(zones)) {
      for (const u of zone.units ?? []) {
        u.hidden ??= false;
        u.pathGoal ??= -1;
      }
    }
  },
  // v2 -> v3: co-op-ready. Everything that was "the player's" moves into players[0].
  2: (state) => {
    const s = state as Record<string, never> & Record<string, unknown>;
    state.players = [
      {
        index: 0,
        name: (s.playerName as string) ?? "Jane",
        unitId: s.playerId ?? 0,
        zone: s.zone ?? "county",
        lastMark: s.lastMark ?? "start",
        respawnIn: s.respawnIn ?? 0,
        rest: s.rest ?? null,
        bar: s.bar ?? [],
        craft: s.craft ?? [null, null, null],
        dialogue: s.dialogue ?? null,
        god: s.god ?? false,
        stats: s.stats ?? { kills: 0, deaths: 0, casts: 0 },
        travel: null,
        connected: true,
        parked: null,
      },
    ];
    for (const k of ["playerName", "playerId", "zone", "lastMark", "respawnIn", "rest", "bar", "craft", "dialogue", "god", "stats"]) delete state[k];
  },
  // v3 -> v4: one heroine, one fire, one book. The name, the rest point and what has been
  // learned move from the person to the world; a seat is kept by a token, not by a name.
  3: (state) => {
    type OldPlayer = Record<string, unknown> & { index: number; unitId: number; zone: string; parked: { book?: string[] } | null };
    const players = (state.players ?? []) as OldPlayer[];
    const zones = (state.zones ?? {}) as Record<string, { units?: { id: number; book?: string[] }[] }>;
    const spells: string[] = [];
    state.name = (players[0]?.name as string) ?? "Jane";
    state.rest = players.map((p) => p.rest).find((r) => r) ?? null;
    state.open = false;
    for (const p of players) {
      const body = p.parked ?? zones[p.zone]?.units?.find((u) => u.id === p.unitId);
      for (const spell of body?.book ?? []) if (!spells.includes(spell)) spells.push(spell);
      p.who = p.index === 0 ? "host" : String(p.name ?? `guest${p.index}`);
      delete p.name;
      delete p.rest;
    }
    state.growth = { spells };
  },
};

export function encodeSave(state: GameState, summary: SaveFile["summary"], now: Date): string {
  const file: SaveFile = {
    format: "jane-save",
    version: SAVE_VERSION,
    savedAt: now.toISOString(),
    summary,
    state,
  };
  return JSON.stringify(file);
}

export type DecodeResult = { ok: true; file: SaveFile } | { ok: false; error: string };

export function decodeSave(text: string): DecodeResult {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return { ok: false, error: "Save is not valid JSON" };
  }
  // "junqi-save" is what the format was called before the project was renamed. Same file.
  const known = isRecord(raw) && (raw.format === "jane-save" || raw.format === "junqi-save");
  if (!isRecord(raw) || !known || !isRecord(raw.state)) {
    return { ok: false, error: "Not a Jane save" };
  }
  let version = Number(raw.version);
  if (!Number.isInteger(version) || version < 1) return { ok: false, error: "Save has no version" };
  if (version > SAVE_VERSION) return { ok: false, error: `Save is from a newer build (v${version})` };
  while (version < SAVE_VERSION) {
    const migrate = MIGRATIONS[version];
    if (!migrate) return { ok: false, error: `No migration from save v${version}` };
    migrate(raw.state);
    version++;
  }
  raw.version = SAVE_VERSION;
  (raw.state as Record<string, unknown>).version = SAVE_VERSION;
  return { ok: true, file: raw as unknown as SaveFile };
}

/**
 * Order-stable hash of the whole state. Two runs with the same seed and the
 * same inputs must agree on this at every tick; the determinism test and the
 * F2 overlay both use it.
 */
export function hashState(state: GameState): string {
  return hashString(stableStringify(state)).toString(16).padStart(8, "0");
}

function stableStringify(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value) ?? "null";
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  const rec = value as Record<string, unknown>;
  const keys = Object.keys(rec).sort();
  const parts: string[] = [];
  for (const k of keys) {
    if (rec[k] === undefined) continue;
    parts.push(`${JSON.stringify(k)}:${stableStringify(rec[k])}`);
  }
  return `{${parts.join(",")}}`;
}

/** Deep copy through the same path a save takes. Used by tests and by slot writes. */
export function cloneState(state: GameState): GameState {
  return JSON.parse(JSON.stringify(state)) as GameState;
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}
