import { existsSync } from "node:fs";
import { join } from "node:path";
import { createHostedAuth } from "./auth.ts";
import { createApp } from "./app.ts";
import { DEFAULT_DB_PATH, PKG_ROOT, openDb } from "./db.ts";
import { MCP_PATH } from "./mcp/http.ts";
import { createJobs } from "./jobs.ts";
import { acquireProcessLock } from "./process-lock.ts";
import { migrateLegacyToken } from "./tokens.ts";
import { getMode } from "./mode.ts";
import { logging } from "./logging.ts";

const PORT = Number(process.env.DECKBUILDER_PORT ?? 8787);

const release = acquireProcessLock(`${DEFAULT_DB_PATH}.server-lock`);
process.once("exit", release);
const mode = getMode();
const db = openDb();
if (mode === "local") migrateLegacyToken(db);
const jobs = createJobs(db, {
  onChange: () => app.notifyAll({ kind: "jobs" }),
  ...(mode === "hosted" ? { writeFailure: (write: () => void) => app.withDatabase(write) } : {}),
});
const auth = mode === "hosted" ? createHostedAuth(db) : undefined;
await auth?.ready;
const app = createApp(db, { mode, port: PORT, jobs, auth });
let closing = false;
const shutdown = async () => {
  if (closing) return;
  closing = true;
  await app.close();
  await jobs.close();
  db.close();
  release();
};
process.on("SIGTERM", () => void shutdown());
process.on("SIGINT", () => void shutdown());
app.server.on("error", error => { logging.failure("server_error", error); void shutdown().then(() => { process.exitCode = 1; }); });

// Local remains loopback-only; hosted uses all interfaces. Both modes check
// Host/Origin, MCP resolves a bearer, and hosted UI requests await sessions.
app.server.listen(PORT, mode === "local" ? "127.0.0.1" : "0.0.0.0", () => {
  jobs.start();
  logging.event("server_started", { mode, port: PORT, mcp: MCP_PATH, pid: process.pid });
  const cards = (db.prepare("SELECT COUNT(*) n FROM cards").get() as { n: number }).n;
  if (!cards) logging.event("empty_cards_refresh_queued");
  if (!existsSync(join(PKG_ROOT, "web", "dist")))
    logging.event("web_build_missing");
});
