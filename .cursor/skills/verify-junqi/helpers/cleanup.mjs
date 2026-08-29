#!/usr/bin/env node
import { existsSync, readFileSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const HERE = dirname(fileURLToPath(import.meta.url));
const SKILL = resolve(HERE, "..");
const PID_FILE = resolve(SKILL, ".run/pid");
const RUN = resolve(SKILL, ".run");

if (!existsSync(PID_FILE)) {
  console.log("nothing to stop");
  process.exit(0);
}

const pid = Number(readFileSync(PID_FILE, "utf8"));
if (!pid) {
  console.error("bad pid file");
  process.exit(1);
}

if (process.platform === "win32") {
  spawnSync("taskkill", ["/PID", String(pid), "/T", "/F"], { stdio: "inherit" });
} else {
  try {
    process.kill(-pid, "SIGTERM");
  } catch {
    try {
      process.kill(pid, "SIGTERM");
    } catch {
      /* already gone */
    }
  }
}

rmSync(RUN, { recursive: true, force: true });
console.log(`stopped pid=${pid}; evidence left in place`);
