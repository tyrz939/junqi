import "./style.css";
import { App } from "@/app/app";

const canvas = document.getElementById("world") as HTMLCanvasElement;
const ui = document.getElementById("ui") as HTMLElement;

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
