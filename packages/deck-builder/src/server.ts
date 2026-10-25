import { existsSync } from "node:fs";
import { join } from "node:path";
import { createApp } from "./app.ts";
import { DEFAULT_DB_PATH, PKG_ROOT, openDb } from "./db.ts";
import { MCP_PATH } from "./mcp/http.ts";
import { createJobs } from "./jobs.ts";
import { acquireProcessLock } from "./process-lock.ts";
import { ensureAgentToken, rotateAgentToken } from "./token.ts";

const PORT = Number(process.env.DECKBUILDER_PORT ?? 8787);

const release = acquireProcessLock(`${DEFAULT_DB_PATH}.server-lock`);
process.once("exit", release);
const db = openDb();
const jobs = createJobs(db, { onChange: () => app.notify() });
const app = createApp(db, { token: ensureAgentToken(), rotateToken: rotateAgentToken, port: PORT, jobs });
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
app.server.on("error", error => { console.error(error); void shutdown().then(() => { process.exitCode = 1; }); });

// Loopback only. This is necessary and not sufficient: the agent endpoint
// also requires a token and both doors check Host and Origin (see app.ts and
// mcp/http.ts).
app.server.listen(PORT, "127.0.0.1", () => {
  jobs.start();
  console.log(`Deck builder running at http://localhost:${PORT}`);
  console.log(`Agent endpoint (MCP): http://127.0.0.1:${PORT}${MCP_PATH} — token and client setup are on the Settings page`);
  console.log(`Database: ${DEFAULT_DB_PATH}`);
  const cards = (db.prepare("SELECT COUNT(*) n FROM cards").get() as { n: number }).n;
  if (!cards) console.log("The card database is empty — a background refresh has been queued to download card data.");
  if (!existsSync(join(PKG_ROOT, "web", "dist")))
    console.log("(web/dist not found — run 'npm run app' to build the UI, or 'npm run dev:web' for the dev server)");
});
