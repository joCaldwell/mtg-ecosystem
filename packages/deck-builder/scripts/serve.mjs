#!/usr/bin/env node
// Start/stop/restart/status/logs for the local app, with a pidfile so
// stopping never means guessing at `pkill -f node` and taking something else
// down with it.
//
//   node scripts/serve.mjs start [--no-build]
//   node scripts/serve.mjs stop | restart | status | logs [-n N] [-f]

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, openSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const PKG_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const RUN_DIR = join(PKG_ROOT, ".dev");
const PID_FILE = join(RUN_DIR, "server.pid");
const LOG_FILE = join(RUN_DIR, "server.log");
const PORT = Number(process.env.DECKBUILDER_PORT ?? 8787);
const URL = `http://localhost:${PORT}`;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function readPid() {
  if (!existsSync(PID_FILE)) return null;
  const pid = Number(readFileSync(PID_FILE, "utf-8").trim());
  return Number.isInteger(pid) && pid > 0 ? pid : null;
}

function alive(pid) {
  try {
    process.kill(pid, 0); // signal 0 tests for existence without delivering
    return true;
  } catch {
    return false;
  }
}

// The pidfile can go stale (crash, reboot, manual kill), so it is always
// cross-checked against the process table before being believed.
function running() {
  const pid = readPid();
  if (pid && alive(pid)) return pid;
  if (pid) rmSync(PID_FILE, { force: true });
  return null;
}

async function portResponds(timeoutMs = 500) {
  try {
    const res = await fetch(`${URL}/api/decks`, {
      signal: AbortSignal.timeout(timeoutMs),
    });
    return res.ok;
  } catch {
    return false;
  }
}

function run(cmd, args, opts = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd: PKG_ROOT, stdio: "inherit", ...opts });
    child.on("exit", (code) =>
      code === 0 ? resolve() : reject(new Error(`${cmd} exited with ${code}`)),
    );
    child.on("error", reject);
  });
}

async function start({ build = true } = {}) {
  const pid = running();
  if (pid) {
    console.log(`Already running (pid ${pid}) at ${URL}`);
    return;
  }
  // Someone started it by hand, or another process owns the port. Say so
  // rather than starting a second copy that will fail to bind.
  if (await portResponds()) {
    console.error(
      `Port ${PORT} is already serving something, but no pidfile exists.\n` +
        `It was probably started by hand. Stop it yourself, or set DECKBUILDER_PORT to use another port.`,
    );
    process.exitCode = 1;
    return;
  }

  if (build) {
    console.log("Building the web UI…");
    await run("npx", ["vite", "build", "--logLevel", "warn"]);
  }

  mkdirSync(RUN_DIR, { recursive: true });
  const out = openSync(LOG_FILE, "a");
  const child = spawn(process.execPath, ["src/server.ts"], {
    cwd: PKG_ROOT,
    detached: true,
    stdio: ["ignore", out, out],
    env: { ...process.env, DECKBUILDER_PORT: String(PORT) },
  });
  child.unref();
  writeFileSync(PID_FILE, String(child.pid));

  for (let i = 0; i < 40; i++) {
    if (await portResponds()) {
      console.log(`Deck builder running at ${URL} (pid ${child.pid})`);
      console.log(`Logs: npm run logs   ·   Stop: npm run stop`);
      return;
    }
    if (!alive(child.pid)) break;
    await sleep(150);
  }

  rmSync(PID_FILE, { force: true });
  console.error(`Server failed to come up. Last log lines:\n`);
  console.error(tail(30));
  process.exitCode = 1;
}

async function stop() {
  const pid = running();
  if (!pid) {
    if (await portResponds()) {
      console.log(
        `Nothing in the pidfile, but port ${PORT} is still serving — that instance was started by hand; stop it the same way.`,
      );
      process.exitCode = 1;
      return;
    }
    console.log("Not running.");
    return;
  }

  process.kill(pid, "SIGTERM");
  for (let i = 0; i < 40; i++) {
    if (!alive(pid)) break;
    await sleep(100);
  }
  if (alive(pid)) {
    console.log("Did not exit on SIGTERM; sending SIGKILL.");
    process.kill(pid, "SIGKILL");
    await sleep(200);
  }
  rmSync(PID_FILE, { force: true });
  console.log(`Stopped (pid ${pid}).`);
}

async function status() {
  const pid = running();
  const responds = await portResponds(1500);
  console.log(`pid file : ${pid ? `${pid} (alive)` : "none"}`);
  console.log(`port ${PORT} : ${responds ? "responding" : "no response"}`);
  console.log(`url      : ${URL}`);
  // Reporting "not running" is a successful status check, not a failure —
  // exiting non-zero here just makes npm print an error block over the answer.
  if (!responds) {
    console.log(pid ? "\nAlive but not answering — check `npm run logs`." : "\nNot running. Start it with `npm start`.");
    return;
  }
  try {
    const decks = await (await fetch(`${URL}/api/decks`)).json();
    console.log(`decks    : ${decks.length}`);
    for (const d of decks) console.log(`  #${d.id} ${d.name} — ${d.card_count}/100 (${d.color_identity || "no commander"})`);
  } catch {
    /* status is best-effort past this point */
  }
}

function tail(n) {
  if (!existsSync(LOG_FILE)) return "(no log file yet)";
  return readFileSync(LOG_FILE, "utf-8").split("\n").slice(-n).join("\n");
}

async function logs(args) {
  const nFlag = args.indexOf("-n");
  const n = nFlag >= 0 ? Number(args[nFlag + 1]) || 50 : 50;
  console.log(tail(n));
  if (args.includes("-f")) await run("tail", ["-f", LOG_FILE]);
}

const [cmd, ...rest] = process.argv.slice(2);
switch (cmd) {
  case "start":
    await start({ build: !rest.includes("--no-build") });
    break;
  case "stop":
    await stop();
    break;
  case "restart":
    await stop();
    await start({ build: !rest.includes("--no-build") });
    break;
  case "status":
    await status();
    break;
  case "logs":
    await logs(rest);
    break;
  default:
    console.error("Usage: serve.mjs start [--no-build] | stop | restart | status | logs [-n N] [-f]");
    process.exitCode = 1;
}
