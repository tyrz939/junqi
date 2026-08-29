#!/usr/bin/env node
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createConnection } from "node:net";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const SKILL = resolve(HERE, "..");
const JUNQI = resolve(SKILL, "../../../junqi");
const PORT = Number(process.env.JUNQI_VERIFY_PORT || 5188);
const RUN = resolve(SKILL, ".run");
const PID_FILE = resolve(RUN, "pid");
const HOST = "127.0.0.1";

function listening() {
  return new Promise((done) => {
    const sock = createConnection({ host: HOST, port: PORT }, () => {
      sock.end();
      done(true);
    });
    sock.on("error", () => done(false));
  });
}

function pidAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

if (await listening()) {
  const ours = existsSync(PID_FILE) ? Number(readFileSync(PID_FILE, "utf8")) : 0;
  if (ours && pidAlive(ours)) {
    console.log(`already running pid=${ours} url=http://${HOST}:${PORT}/`);
    process.exit(0);
  }
  console.error(`port ${PORT} is up but not our verify instance. refuse to drive. pick another JUNQI_VERIFY_PORT or stop the occupant.`);
  process.exit(2);
}

mkdirSync(RUN, { recursive: true });
const vite = resolve(JUNQI, "node_modules/vite/bin/vite.js");
if (!existsSync(vite)) {
  console.error(`missing ${vite} — run npm install in junqi/`);
  process.exit(1);
}
const child = spawn(
  process.execPath,
  [vite, "--port", String(PORT), "--strictPort", "--host", HOST],
  {
    cwd: JUNQI,
    detached: true,
    stdio: "ignore",
    windowsHide: true,
  },
);
child.unref();
writeFileSync(PID_FILE, String(child.pid));

const deadline = Date.now() + 40000;
while (Date.now() < deadline) {
  if (await listening()) {
    console.log(`ready pid=${child.pid} url=http://${HOST}:${PORT}/`);
    process.exit(0);
  }
  await new Promise((r) => setTimeout(r, 250));
}

console.error("vite did not bind in 40s");
process.exit(1);
