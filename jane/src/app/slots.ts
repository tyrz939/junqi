// The slot store, over two tiny interfaces so the rules can be tested without a browser:
//
//   SlotDb      async, structured values. IndexedDB in the browser, a Map in tests.
//   TextStore   sync strings. localStorage: where saves lived first, and the fallback.
//
// The rules:
//   - The menus list slots synchronously, so the three summaries live in memory.
//     `init()` fills them; every write and clear keeps them true.
//   - Everything that touches one slot runs in a queue for that slot. Two saves in a
//     row land in order and the second is what remains; a load asked for right after
//     a save reads that save; nothing interleaves.
//   - A slot lives in the database, gzipped. If the database is missing or throws, the
//     same text goes to the text store uncompressed. If there is no CompressionStream,
//     the database holds plain text. None of this is ever the player's problem.
//   - A slot that cannot be read is an empty slot, never a throw.

import { canGzip, gunzipText, gzipText } from "@/app/gzip";
import { decodeSave, type SaveFile } from "@/sim/save";
import type { SlotInfo } from "@/ui/host";

export const SLOT_COUNT = 3;

/** The summary kept beside the data, so listing slots never decompresses one. */
export type SlotMeta = SlotInfo & { version: number; bytes: number };
/** `data` is gzip bytes, or the JSON itself when the platform cannot compress. */
export type SlotRecord = { meta: SlotMeta; data: ArrayBuffer | string };

export interface SlotDb {
  get(slot: number): Promise<unknown>;
  put(slot: number, record: SlotRecord): Promise<void>;
  delete(slot: number): Promise<void>;
}

export interface TextStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

const key = (slot: number): string => `jane.save.${slot}`;
/** Where slots lived before the project was renamed. */
const oldKey = (slot: number): string => `junqi.save.${slot}`;

type Held = { meta: SlotMeta; in: "db" | "text" };

export class SlotStore {
  private readonly held: (Held | null)[] = Array.from({ length: SLOT_COUNT }, () => null);
  private readonly tails: Promise<unknown>[] = Array.from({ length: SLOT_COUNT }, () => Promise.resolve());
  private db: SlotDb | null;

  constructor(
    db: SlotDb | null,
    private readonly text: TextStore | null,
    private readonly gzip: boolean = canGzip(),
  ) {
    this.db = db;
  }

  /**
   * Fill the summaries. Cheap on purpose (three small reads), because boot waits for it.
   * Slots still sitting in the text store are listed at once and moved into the database
   * behind the scenes, through the same queue as any other write.
   */
  async init(): Promise<void> {
    for (let slot = 0; slot < SLOT_COUNT; slot++) {
      if (!this.db) break;
      try {
        const meta = metaOf(await this.db.get(slot), slot);
        if (meta) this.held[slot] = { meta, in: "db" };
      } catch {
        // Private windows and blocked site data fail here. The text store is all there is.
        this.db = null;
        this.held.fill(null);
      }
    }
    for (let slot = 0; slot < SLOT_COUNT; slot++) {
      const json = this.legacyText(slot);
      const file = json ? decoded(json) : null;
      if (!json || !file) continue; // not ours, or not readable: leave it alone
      const meta: SlotMeta = { slot, savedAt: file.savedAt, ...file.summary, version: file.version, bytes: json.length };
      // A text copy beside a newer database copy is a leftover. Anything else is the slot.
      const inDb = this.held[slot];
      if (inDb && inDb.meta.savedAt >= meta.savedAt) continue;
      this.held[slot] = { meta, in: "text" };
      if (this.db) void this.enqueue(slot, () => this.commit(slot, json, meta));
    }
  }

  /** Always length 3; null = empty slot. */
  infos(): (SlotInfo | null)[] {
    return this.held.map((h) => (h ? { slot: h.meta.slot, savedAt: h.meta.savedAt, zone: h.meta.zone, day: h.meta.day, hour: h.meta.hour, hp: h.meta.hp, maxhp: h.meta.maxhp } : null));
  }

  /** `json` is an encoded SaveFile, already taken off the live state. Resolves once it is really stored. */
  write(slot: number, json: string, summary: SlotInfo, version: number): Promise<boolean> {
    return this.enqueue(slot, () => this.commit(slot, json, { ...summary, slot, version, bytes: json.length }));
  }

  read(slot: number): Promise<SaveFile | null> {
    return this.enqueue(slot, async () => {
      const at = this.held[slot];
      if (!at) return null;
      let file: SaveFile | null = null;
      try {
        const json = at.in === "db" ? await this.dbText(slot) : this.legacyText(slot);
        file = json ? decoded(json) : null;
      } catch {
        file = null;
      }
      // Unreadable is empty. The bytes stay where they are; only the listing forgets them.
      if (!file) this.held[slot] = null;
      return file;
    });
  }

  clear(slot: number): Promise<void> {
    return this.enqueue(slot, async () => {
      this.held[slot] = null;
      this.dropText(slot);
      try {
        await this.db?.delete(slot);
      } catch {
        /* nothing to clear */
      }
    });
  }

  /** Resolves when nothing is queued. For tests, and for anything that must not outrun a write. */
  async idle(): Promise<void> {
    await Promise.all(this.tails);
  }

  // --- internals -------------------------------------------------------------------

  private enqueue<T>(slot: number, job: () => Promise<T>): Promise<T> {
    const run = this.tails[slot].then(job);
    // The queue must survive a job that throws; the caller still sees the rejection.
    this.tails[slot] = run.catch(() => undefined);
    return run;
  }

  private async commit(slot: number, json: string, meta: SlotMeta): Promise<boolean> {
    if (this.db) {
      try {
        const data = await this.pack(json);
        const bytes = typeof data === "string" ? data.length : data.byteLength;
        const stored = { ...meta, bytes };
        await this.db.put(slot, { meta: stored, data });
        // One home per slot: a stale text copy would come back as a migration next boot.
        this.dropText(slot);
        this.held[slot] = { meta: stored, in: "db" };
        return true;
      } catch {
        /* quota, a closed connection, a revoked permission: try the text store */
      }
    }
    try {
      if (!this.text) return false;
      this.text.setItem(key(slot), json);
      this.held[slot] = { meta, in: "text" };
      return true;
    } catch {
      return false;
    }
  }

  private async pack(json: string): Promise<ArrayBuffer | string> {
    if (!this.gzip) return json;
    try {
      return (await gzipText(json)).buffer as ArrayBuffer;
    } catch {
      return json;
    }
  }

  private async dbText(slot: number): Promise<string | null> {
    const rec = await this.db?.get(slot);
    if (!isRecord(rec)) return null;
    const data = rec.data;
    if (typeof data === "string") return data;
    if (data instanceof ArrayBuffer) return gunzipText(new Uint8Array(data));
    if (ArrayBuffer.isView(data)) return gunzipText(new Uint8Array(data.buffer as ArrayBuffer, data.byteOffset, data.byteLength));
    if (typeof Blob !== "undefined" && data instanceof Blob) return gunzipText(new Uint8Array(await data.arrayBuffer()));
    return null;
  }

  private legacyText(slot: number): string | null {
    try {
      return this.text?.getItem(key(slot)) || this.text?.getItem(oldKey(slot)) || null;
    } catch {
      return null;
    }
  }

  private dropText(slot: number): void {
    try {
      this.text?.removeItem(key(slot));
      this.text?.removeItem(oldKey(slot));
    } catch {
      /* nothing to drop */
    }
  }
}

function decoded(json: string): SaveFile | null {
  const result = decodeSave(json);
  return result.ok ? result.file : null;
}

/** A database row is somebody else's data until proven otherwise. */
function metaOf(rec: unknown, slot: number): SlotMeta | null {
  if (!isRecord(rec) || !isRecord(rec.meta) || rec.data == null) return null;
  const m = rec.meta;
  if (typeof m.savedAt !== "string" || typeof m.zone !== "string") return null;
  const n = (v: unknown): number => (typeof v === "number" && Number.isFinite(v) ? v : 0);
  return { slot, savedAt: m.savedAt, zone: m.zone, day: n(m.day), hour: n(m.hour), hp: n(m.hp), maxhp: n(m.maxhp), version: n(m.version), bytes: n(m.bytes) };
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}
