// Save slots. Three of them, each a versioned SaveFile (sim/save.ts), gzipped into
// IndexedDB: a county-sized world is several MB of JSON, and localStorage gives a
// whole origin about five. The rules live in app/slots.ts; this file is only the
// browser's end of it: the database, the old localStorage home, and one store.
//
// Every call is wrapped: private windows and blocked site data throw on access,
// and a corrupt slot must read as "empty", never as a crash on the title screen.

import { SlotStore, type SlotDb, type SlotRecord, type TextStore } from "@/app/slots";
import { SAVE_VERSION, TICKS_PER_HOUR } from "@/sim/constants";
import { encodeSave, type SaveFile } from "@/sim/save";
import type { Sim } from "@/sim/sim";
import { maxHp } from "@/sim/units";
import type { SlotInfo } from "@/ui/host";

export { SLOT_COUNT } from "@/app/slots";

const DB_NAME = "jane";
const DB_STORE = "saves";
/** Some browsers never answer `open` when site data is blocked. Boot must not wait on that. */
const OPEN_TIMEOUT_MS = 2500;

/** One connection, opened on first use. A failed open fails every call, and the store falls back. */
class IdbSlots implements SlotDb {
  private opened: Promise<IDBDatabase> | null = null;

  get(slot: number): Promise<unknown> {
    return this.run("readonly", (s) => s.get(slot));
  }

  async put(slot: number, record: SlotRecord): Promise<void> {
    await this.run("readwrite", (s) => s.put(record, slot));
  }

  async delete(slot: number): Promise<void> {
    await this.run("readwrite", (s) => s.delete(slot));
  }

  private open(): Promise<IDBDatabase> {
    this.opened ??= new Promise<IDBDatabase>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("IndexedDB did not open")), OPEN_TIMEOUT_MS);
      const req = indexedDB.open(DB_NAME, 1);
      req.onupgradeneeded = () => {
        if (!req.result.objectStoreNames.contains(DB_STORE)) req.result.createObjectStore(DB_STORE);
      };
      req.onsuccess = () => {
        clearTimeout(timer);
        const db = req.result;
        // Another tab on a newer build wants to upgrade: let go rather than block it.
        db.onversionchange = () => db.close();
        resolve(db);
      };
      req.onerror = () => {
        clearTimeout(timer);
        reject(req.error ?? new Error("IndexedDB refused to open"));
      };
    });
    return this.opened;
  }

  /** One request in its own transaction. Resolves on COMMIT, not on the request: "Saved" must mean saved. */
  private async run<T>(mode: IDBTransactionMode, ask: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
    const db = await this.open();
    return new Promise<T>((resolve, reject) => {
      const tx = db.transaction(DB_STORE, mode);
      const req = ask(tx.objectStore(DB_STORE));
      tx.oncomplete = () => resolve(req.result);
      tx.onerror = () => reject(tx.error ?? req.error ?? new Error("IndexedDB transaction failed"));
      tx.onabort = () => reject(tx.error ?? new Error("IndexedDB transaction aborted"));
    });
  }
}

function localText(): TextStore | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

function database(): SlotDb | null {
  try {
    // Merely touching `indexedDB` throws in some sandboxed frames.
    return typeof indexedDB === "undefined" || indexedDB === null ? null : new IdbSlots();
  } catch {
    return null;
  }
}

const store = new SlotStore(database(), localText());

/** Boot awaits this before the UI exists, so the title's first slot list is already true. Never rejects. */
export function initStorage(): Promise<void> {
  return store.init().catch(() => undefined);
}

/**
 * The state is taken off the sim HERE, synchronously, so the save is the tick it was asked
 * for on. Compressing and writing happen afterwards and never hold up a frame. Resolves true
 * once the slot is really stored.
 */
export function writeSlot(slot: number, sim: Sim): Promise<boolean> {
  const state = sim.prepareSave();
  const p = sim.player;
  const now = new Date();
  const summary = { zone: sim.rt.bp.name, day: state.day, hour: state.clock / TICKS_PER_HOUR, hp: Math.round(p.hp), maxhp: maxHp(p) };
  const json = encodeSave(state, summary, now);
  return store.write(slot, json, { slot, savedAt: now.toISOString(), ...summary }, SAVE_VERSION);
}

export function readSlot(slot: number): Promise<SaveFile | null> {
  return store.read(slot).catch(() => null);
}

/** From memory: the menus call this while building a page. */
export function slotInfos(): (SlotInfo | null)[] {
  return store.infos();
}

export function clearSlot(slot: number): Promise<void> {
  return store.clear(slot).catch(() => undefined);
}
