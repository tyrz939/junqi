import "./style.css";
import { App } from "@/app/app";
import { initStorage } from "@/app/storage";

const canvas = document.getElementById("world") as HTMLCanvasElement;
const ui = document.getElementById("ui") as HTMLElement;

// The title lists the save slots the moment it is drawn, and it reads them from memory.
// So the slot summaries are fetched first: three small IndexedDB reads, bounded by a
// timeout, and it never rejects (no storage just means three empty slots).
await initStorage();

try {
  const app = new App(canvas, ui);
  // A handle for the browser console and for automated verification. Not used by the game.
  (window as unknown as { jane: App }).jane = app;
} catch (err) {
  // A bad content row is a boot error by design (catalog validation). Show it, do not hide it.
  const pre = document.createElement("pre");
  pre.className = "boot-error";
  pre.textContent = `Jane could not start.\n\n${err instanceof Error ? err.message : String(err)}`;
  document.body.append(pre);
  throw err;
}
