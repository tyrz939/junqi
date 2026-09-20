// Save storage. The gzip codec runs for real (Node has CompressionStream). IndexedDB
// does not exist here, so the slot rules are proven over an in-memory stand-in: what
// is left unproven is only the thin IndexedDB wrapper in app/storage.ts.

import { describe, expect, it } from "vitest";
import { gunzipText, gzipText } from "@/app/gzip";
import { SlotStore, type SlotDb, type SlotRecord, type TextStore } from "@/app/slots";
import { buildCatalog } from "@/sim/catalog";
import { SAVE_VERSION } from "@/sim/constants";
import { cloneState, decodeSave, encodeSave, hashState } from "@/sim/save";
import { Sim } from "@/sim/sim";
import type { SlotInfo } from "@/ui/host";

const catalog = buildCatalog();
const SUMMARY = { zone: "County", day: 0, hour: 17, hp: 1, maxhp: 1 };

function saveText(seed: number, at = new Date(0)): string {
  return encodeSave(cloneState(Sim.newGame(catalog, seed).state), SUMMARY, at);
}

const info = (slot: number, at = new Date(0)): SlotInfo => ({ slot, savedAt: at.toISOString(), ...SUMMARY });

class FakeDb implements SlotDb {
  readonly rows = new Map<number, SlotRecord>();
  readonly log: string[] = [];
  broken = false;
  /** Earlier puts take longer, so anything that does not queue would land out of order. */
  private delay = 20;

  async get(slot: number): Promise<unknown> {
    if (this.broken) throw new Error("blocked");
    return this.rows.get(slot);
  }

  async put(slot: number, record: SlotRecord): Promise<void> {
    if (this.broken) throw new Error("blocked");
    this.log.push(`begin ${record.meta.savedAt}`);
    await new Promise((r) => setTimeout(r, (this.delay = Math.max(0, this.delay - 10))));
    this.rows.set(slot, record);
    this.log.push(`end ${record.meta.savedAt}`);
  }

  async delete(slot: number): Promise<void> {
    if (this.broken) throw new Error("blocked");
    this.rows.delete(slot);
  }
}

class FakeText implements TextStore {
  readonly items = new Map<string, string>();
  getItem(key: string): string | null {
    return this.items.get(key) ?? null;
  }
  setItem(key: string, value: string): void {
    this.items.set(key, value);
  }
  removeItem(key: string): void {
    this.items.delete(key);
  }
}

describe("gzip codec", () => {
  it("round-trips a real save and makes it much smaller", async () => {
    const text = saveText(4321);
    const packed = await gzipText(text);
    expect(packed.byteLength).toBeLessThan(text.length / 4);
    const back = await gunzipText(packed);
    expect(back).toBe(text);
    const decoded = decodeSave(back);
    expect(decoded.ok).toBe(true);
    if (decoded.ok) expect(hashState(decoded.file.state)).toBe(hashState(Sim.newGame(catalog, 4321).state));
  });

  it("rejects bytes that are not gzip", async () => {
    await expect(gunzipText(new TextEncoder().encode("{not gzip"))).rejects.toBeDefined();
  });
});

describe("slot store", () => {
  it("writes gzip to the database, lists from memory, reads back", async () => {
    const db = new FakeDb();
    const store = new SlotStore(db, new FakeText());
    await store.init();
    expect(store.infos()).toEqual([null, null, null]);

    const text = saveText(7);
    expect(await store.write(1, text, info(1), SAVE_VERSION)).toBe(true);
    expect(store.infos()[1]).toEqual(info(1));
    const row = db.rows.get(1);
    expect(row?.data).toBeInstanceOf(ArrayBuffer);
    expect(row?.meta.bytes).toBeLessThan(text.length / 4);
    expect(row?.meta.version).toBe(SAVE_VERSION);

    // A fresh store (the next boot) sees the slot without opening its data.
    const next = new SlotStore(db, new FakeText());
    await next.init();
    expect(next.infos()[1]).toEqual(info(1));
    const file = await next.read(1);
    expect(file && hashState(file.state)).toBe(hashState(Sim.newGame(catalog, 7).state));

    await next.clear(1);
    expect(next.infos()[1]).toBeNull();
    expect(db.rows.has(1)).toBe(false);
  });

  it("queues writes per slot: the last one asked for is what remains, and a read waits its turn", async () => {
    const db = new FakeDb();
    const store = new SlotStore(db, null);
    await store.init();
    const first = new Date(1000);
    const second = new Date(2000);
    const a = store.write(0, saveText(1, first), info(0, first), SAVE_VERSION);
    const b = store.write(0, saveText(2, second), info(0, second), SAVE_VERSION);
    const read = store.read(0);
    expect(await Promise.all([a, b])).toEqual([true, true]);
    expect(db.log).toEqual([`begin ${first.toISOString()}`, `end ${first.toISOString()}`, `begin ${second.toISOString()}`, `end ${second.toISOString()}`]);
    expect(store.infos()[0]?.savedAt).toBe(second.toISOString());
    expect((await read)?.state.seed).toBe(2);
  });

  it("falls back to plain text when the database throws", async () => {
    const db = new FakeDb();
    db.broken = true;
    const text = new FakeText();
    const store = new SlotStore(db, text);
    await store.init();
    expect(await store.write(2, saveText(9), info(2), SAVE_VERSION)).toBe(true);
    expect(decodeSave(text.items.get("jane.save.2") ?? "").ok).toBe(true);
    expect(store.infos()[2]).toEqual(info(2));
    expect((await store.read(2))?.state.seed).toBe(9);
    // With nowhere at all to write, a save fails quietly.
    expect(await new SlotStore(null, null).write(0, saveText(9), info(0), SAVE_VERSION)).toBe(false);
  });

  it("stores plain JSON in the database when the platform cannot gzip", async () => {
    const db = new FakeDb();
    const store = new SlotStore(db, null, false);
    await store.init();
    await store.write(0, saveText(5), info(0), SAVE_VERSION);
    expect(typeof db.rows.get(0)?.data).toBe("string");
    expect((await store.read(0))?.state.seed).toBe(5);
  });

  it("moves localStorage slots into the database once, old key included, and leaves junk alone", async () => {
    const db = new FakeDb();
    const text = new FakeText();
    text.items.set("jane.save.0", saveText(11, new Date(5000)));
    text.items.set("junqi.save.1", saveText(12));
    text.items.set("jane.save.2", "{not a save");
    const store = new SlotStore(db, text);
    await store.init();
    // Listed at once, before the move has finished.
    expect(store.infos().map((i) => i?.savedAt ?? null)).toEqual([new Date(5000).toISOString(), new Date(0).toISOString(), null]);
    await store.idle();
    expect([...db.rows.keys()].sort()).toEqual([0, 1]);
    expect([...text.items.keys()]).toEqual(["jane.save.2"]);
    expect((await store.read(1))?.state.seed).toBe(12);

    // Next boot: nothing left to move, the same slots, now from the database.
    const next = new SlotStore(db, text);
    await next.init();
    await next.idle();
    expect(next.infos().map((i) => i !== null)).toEqual([true, true, false]);
    expect((await next.read(0))?.state.seed).toBe(11);
  });

  it("reads a corrupt slot as empty, never as a throw", async () => {
    const db = new FakeDb();
    db.rows.set(0, { meta: { ...info(0), version: SAVE_VERSION, bytes: 3 }, data: new Uint8Array([1, 2, 3]).buffer });
    db.rows.set(1, { nonsense: true } as unknown as SlotRecord);
    const store = new SlotStore(db, null);
    await store.init();
    expect(store.infos().map((i) => i !== null)).toEqual([true, false, false]);
    expect(await store.read(0)).toBeNull();
    expect(store.infos()[0]).toBeNull();
    expect(await store.read(1)).toBeNull();
  });
});
