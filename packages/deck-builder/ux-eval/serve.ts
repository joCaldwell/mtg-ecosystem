#!/usr/bin/env node
// Serves the app for capture: createApp with no background jobs, in hosted
// mode, against a fresh copy of the seed database, serving a web build made
// outside the repository.
//
//   node serve.ts [--port 8830] [--seed /tmp/ux-eval/seed/seed.db] [--dist <dir>]
//   node serve.ts --build-only [--build-dir <dir>]     prints the dist path
//
// Sessions are mocked the way test/sharing-http.test.ts and
// test/permissions.test.ts do it, through createApp's resolveUiActor seam: the
// cookie `uxeval_user=<id>` signs that user in, and no cookie is signed out.
// Hosted auth is still configured, so the sign-in page, legal footer and
// signed-out public pages render as they do in production.
//
// Rate limits are raised in-process: a capture makes hundreds of requests a
// minute as one user, which a real person never does.

import { copyFileSync, cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { FROZEN_NOW, installClock, installRandom, installSqliteClock } from "./lib/determinism.ts";

const HERE = dirname(fileURLToPath(import.meta.url));
const PKG = resolve(HERE, "..");

function arg(name: string, fallback?: string): string | undefined {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] && !process.argv[i + 1].startsWith("--") ? process.argv[i + 1] : fallback;
}
const flag = (name: string) => process.argv.includes(`--${name}`);

/**
 * vite build into <dir>/web/dist, never the repository's web/dist. The bundle
 * check script is copied beside it as <dir>/scripts/, because it resolves
 * ../web/dist/ from its own location.
 */
export function buildWeb(dir: string): string {
  const dist = join(dir, "web", "dist");
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(join(dir, "scripts"), { recursive: true });
  const vite = join(PKG, "node_modules", ".bin", "vite");
  const bin = existsSync(vite) ? vite : join(PKG, "..", "..", "node_modules", ".bin", "vite");
  const r = spawnSync(bin, ["build", "--outDir", dist, "--emptyOutDir", "--logLevel", "warn"], { cwd: PKG, stdio: ["ignore", "inherit", "inherit"] });
  if (r.status !== 0) throw new Error(`vite build failed (${r.status})`);
  cpSync(join(PKG, "scripts", "check-public-bundle.mjs"), join(dir, "scripts", "check-public-bundle.mjs"));
  return dist;
}

if (flag("build-only")) {
  const dir = resolve(arg("build-dir") ?? join("/tmp/ux-eval/build", randomUUID().slice(0, 8)));
  console.log(buildWeb(dir));
  process.exit(0);
}

const PORT = Number(arg("port", "8830"));
const SEED = resolve(arg("seed", "/tmp/ux-eval/seed/seed.db")!);
if (!existsSync(SEED)) throw new Error(`No seed database at ${SEED}; run node seed.ts first`);
const ORIGIN = `http://127.0.0.1:${PORT}`;
const work = join("/tmp/ux-eval/serve", String(PORT));
rmSync(work, { recursive: true, force: true });
mkdirSync(join(work, "data"), { recursive: true });
const dist = arg("dist") ? resolve(arg("dist")!) : buildWeb(join(work, "build"));
const runtimeDb = join(work, "runtime.db");
copyFileSync(SEED, runtimeDb);

const env = {
  DECKBUILDER_MODE: "hosted",
  DECKBUILDER_PUBLIC_ORIGIN: ORIGIN,
  DECKBUILDER_DATA: join(work, "data"),
  DECKBUILDER_DB: runtimeDb,
  DECKBUILDER_OPERATOR_NAME: "Deck builder operator",
  DECKBUILDER_CONTACT_EMAIL: "operator@example.invalid",
  BETTER_AUTH_SECRET: "ux-eval-only-auth-secret-0123456789-abcdef",
  DISCORD_CLIENT_ID: "ux-eval-fixture",
  DISCORD_CLIENT_SECRET: "ux-eval-fixture",
  GOOGLE_CLIENT_ID: "ux-eval-fixture",
  GOOGLE_CLIENT_SECRET: "ux-eval-fixture",
};
Object.assign(process.env, env);
delete process.env.DECKBUILDER_CLAIM_CODE;
delete process.env.DECKBUILDER_INVITES_REQUIRED;
delete process.env.DECKBUILDER_TRUST_PROXY;

installClock(); // frozen at FROZEN_NOW: writes made during capture read "just now"
installRandom(0x5e7e);

const { openDb } = await import("../src/db.ts");
const { createApp } = await import("../src/app.ts");
const { createHostedAuth } = await import("../src/auth.ts");
const { LIMITS } = await import("../src/ratelimit.ts");

for (const rule of Object.values(LIMITS) as { count: number }[]) rule.count = 1_000_000;

const db = openDb(runtimeDb);
installSqliteClock(db as any);
const auth = createHostedAuth(db, process.env);
await auth.ready;

function userFrom(cookie = ""): number | null {
  const m = cookie.match(/(?:^|;\s*)uxeval_user=(\d+)/);
  if (!m) return null;
  const row = db.prepare("SELECT id FROM users WHERE id = ? AND deleted_at IS NULL").get(Number(m[1])) as { id: number } | undefined;
  return row ? row.id : null;
}

const app = createApp(db, {
  mode: "hosted",
  auth,
  webDist: dist,
  port: PORT,
  resolveUiActor: req => {
    const id = userFrom(req.headers.cookie);
    return id ? { user_id: id, via: "ui", label: "ui" } : null;
  },
});
await new Promise<void>(r => app.server.listen(PORT, "127.0.0.1", r));
process.stderr.write(`UX-EVAL READY ${ORIGIN} dist=${dist} now=${FROZEN_NOW}\n`);

let closing = false;
async function shutdown() {
  if (closing) return;
  closing = true;
  await app.close();
  db.close();
  process.exit(0);
}
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
