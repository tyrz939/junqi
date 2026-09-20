#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const SKILL = resolve(HERE, "..");
const PORT = Number(process.env.JANE_VERIFY_PORT || 5188);
const HOST = "127.0.0.1";
const URL = `http://${HOST}:${PORT}/`;
const PID_FILE = resolve(SKILL, ".run/pid");

function pidAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

const ours = existsSync(PID_FILE) ? Number(readFileSync(PID_FILE, "utf8")) : 0;
const report = {
  ok: false,
  url: URL,
  port: PORT,
  oursPid: ours || null,
  oursAlive: ours ? pidAlive(ours) : false,
  http: 0,
  title: "",
  hasWorld: false,
  hasUi: false,
  error: "",
};

try {
  const res = await fetch(URL);
  report.http = res.status;
  const html = await res.text();
  const title = html.match(/<title>([^<]+)<\/title>/);
  report.title = title?.[1] ?? "";
  // The live build's page shell: a world canvas and a DOM UI root. Everything else is built at run time.
  report.hasWorld = html.includes('id="world"');
  report.hasUi = html.includes('id="ui"');
  report.ok =
    report.http === 200 &&
    report.title === "Project Jane" &&
    report.hasWorld &&
    report.hasUi &&
    report.oursAlive;
  if (!report.oursAlive) {
    report.error = ours
      ? "pid file is stale; this URL may be someone else's session"
      : "no .run/pid — do not drive a shared instance";
  }
} catch (err) {
  report.error = err instanceof Error ? err.message : String(err);
}

console.log(JSON.stringify(report, null, 2));
process.exit(report.ok ? 0 : 1);
