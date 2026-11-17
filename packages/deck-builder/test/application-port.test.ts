import test from "node:test";
import assert from "node:assert/strict";
import { cpSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

test("standalone public deck host starts without game source, SDK or generated artifacts", () => {
  const directory = mkdtempSync(join(tmpdir(), "standalone-deck-host-"));
  const source = fileURLToPath(new URL("../", import.meta.url));
  for (const path of ["src", "package.json", "node_modules"]) cpSync(join(source, path), join(directory, path), { recursive: true, verbatimSymlinks: true });
  mkdirSync(join(directory, "private"), { mode: 0o700 });
  writeFileSync(join(directory, "probe.mjs"), `import assert from 'node:assert/strict';
import { createHost } from './src/host.ts';
const host = await createHost({ mode: 'local', port: 0, databasePath: new URL('./private/decks.sqlite', import.meta.url).pathname, dataDirectory: new URL('./private/', import.meta.url).pathname, backgroundJobs: false });
try { const { port } = await host.start(); const response = await fetch('http://127.0.0.1:'+port+'/api/decks'); assert.equal(response.status,200); assert.deepEqual(await response.json(), []); } finally { await host.close(); }\n`);
  execFileSync(process.execPath, ["--permission", `--allow-fs-read=${directory}`, `--allow-fs-write=${directory}`, "--allow-net", "probe.mjs"], { cwd: directory, encoding: "utf8", timeout: 15_000, stdio: ["ignore", "pipe", "pipe"] });
});
