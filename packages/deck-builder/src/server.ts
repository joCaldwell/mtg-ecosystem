import { existsSync } from "node:fs";
import { join } from "node:path";
import { createApp } from "./app.ts";
import { DEFAULT_DB_PATH, PKG_ROOT, openDb } from "./db.ts";
import { MCP_PATH } from "./mcp/http.ts";
import { getAgentToken, rotateAgentToken } from "./token.ts";

const PORT = Number(process.env.DECKBUILDER_PORT ?? 8787);

const db = openDb();
const app = createApp(db, { token: getAgentToken(), rotateToken: rotateAgentToken, port: PORT });

// Loopback only. This is necessary and not sufficient: the agent endpoint
// also requires a token and both doors check Host and Origin (see app.ts and
// mcp/http.ts).
app.server.listen(PORT, "127.0.0.1", () => {
  console.log(`Deck builder running at http://localhost:${PORT}`);
  console.log(`Agent endpoint (MCP): http://127.0.0.1:${PORT}${MCP_PATH} — token and client setup are on the Settings page`);
  console.log(`Database: ${DEFAULT_DB_PATH}`);
  const cards = (db.prepare("SELECT COUNT(*) n FROM cards").get() as { n: number }).n;
  if (!cards) console.log("The card database is empty — run 'npm run refresh' to download card data.");
  if (!existsSync(join(PKG_ROOT, "web", "dist")))
    console.log("(web/dist not found — run 'npm run app' to build the UI, or 'npm run dev:web' for the dev server)");
});
