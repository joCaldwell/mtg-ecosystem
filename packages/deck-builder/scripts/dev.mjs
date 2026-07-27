#!/usr/bin/env node
// Frontend development loop: the API server on :8787 with --watch, and Vite
// on :5173 with hot module replacement. Vite proxies /api to the API server
// (see vite.config.ts), so :5173 is the address to use — edits to .tsx files
// appear without a rebuild or a page reload.
//
// One Ctrl-C stops both. No `concurrently` dependency; this package has no
// runtime deps beyond React and keeps it that way.

import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const PKG_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const API_PORT = Number(process.env.DECKBUILDER_PORT ?? 8787);
const WEB_PORT = Number(process.env.DECKBUILDER_WEB_PORT ?? 5173);

const children = [];
let shuttingDown = false;

function launch(name, cmd, args, color) {
  const child = spawn(cmd, args, {
    cwd: PKG_ROOT,
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, DECKBUILDER_PORT: String(API_PORT) },
  });
  const prefix = `\x1b[${color}m[${name}]\x1b[0m `;
  const pipe = (stream) => {
    stream.setEncoding("utf-8");
    let buffer = "";
    stream.on("data", (chunk) => {
      buffer += chunk;
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";
      for (const line of lines) if (line.trim()) process.stdout.write(prefix + line + "\n");
    });
  };
  pipe(child.stdout);
  pipe(child.stderr);
  child.on("exit", (code) => {
    if (shuttingDown) return;
    console.log(`${prefix}exited with code ${code} — shutting the other process down too.`);
    shutdown(code ?? 1);
  });
  children.push(child);
  return child;
}

function shutdown(code = 0) {
  if (shuttingDown) return;
  shuttingDown = true;
  for (const c of children) {
    try {
      c.kill("SIGTERM");
    } catch {
      /* already gone */
    }
  }
  setTimeout(() => process.exit(code), 300);
}

process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));

launch("api", process.execPath, ["--watch", "src/server.ts"], "36"); // cyan
launch("web", "npx", ["vite", "--port", String(WEB_PORT), "--strictPort"], "35"); // magenta

console.log(`\n  UI (hot reload) → http://localhost:${WEB_PORT}`);
console.log(`  API             → http://localhost:${API_PORT}`);
console.log(`  Ctrl-C stops both.\n`);
