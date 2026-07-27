import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { readFileSync, existsSync } from "node:fs";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { openDb } from "./db.ts";
import { search, SearchError } from "./search/index.ts";
import {
  ServiceError,
  addCard,
  createDeck,
  createSlot,
  createTag,
  deleteDeck,
  deleteSlot,
  deleteTag,
  getDeck,
  listDecks,
  removeCard,
  renameDeck,
  renameTag,
  updateCard,
  updateSlot,
} from "./deck/service.ts";
import {
  acceptItem,
  addCardNote,
  createProposal,
  getCardHistory,
  getLog,
  listCardNotes,
  listHardFilters,
  listProposals,
  rejectItem,
  removeHardFilter,
  undoDecision,
} from "./deck/proposals.ts";
import {
  activeDismissals,
  auditView,
  dismissFinding,
  promoteFinding,
  runAudit,
  undismissFinding,
} from "./deck/audit.ts";
import { runReasoningPass } from "./agent/reasoning.ts";
import {
  acceptBriefEdit,
  getBrief,
  listBriefEdits,
  rejectBriefEdit,
  removeEngine,
  setEngine,
  updateBrief,
} from "./deck/brief.ts";
import { AgentError, getChatHistory, runTurn } from "./agent/agent.ts";
import { assembleContext } from "./agent/context.ts";
import { LlmError, openRouterTransport } from "./agent/llm.ts";
import { ConfigError, getAgentConfig } from "./config.ts";
import { resolveExactName } from "./search/index.ts";

const PORT = Number(process.env.DECKBUILDER_PORT ?? 8787);
const WEB_DIST = join(fileURLToPath(import.meta.url), "..", "..", "web", "dist");

const db = openDb();

type Handler = (params: string[], body: any, url: URL) => unknown | Promise<unknown>;

const routes: Array<{ method: string; pattern: RegExp; handler: Handler }> = [];
function route(method: string, pattern: RegExp, handler: Handler) {
  routes.push({ method, pattern, handler });
}

// One payload with everything the deck view needs; every mutation returns it.
const deckState = (id: number) => ({
  ...getDeck(db, id),
  proposals: listProposals(db, id, "open"),
  brief_edits: listBriefEdits(db, id, "pending").map((e) => ({
    ...e,
    payload: JSON.parse(e.payload_json),
  })),
  log: getLog(db, id, 30),
  hard_filters: listHardFilters(db, id),
  card_notes: listCardNotes(db, id),
});

route("GET", /^\/api\/decks$/, () => listDecks(db));
route("POST", /^\/api\/decks$/, (_p, body) => {
  const id = createDeck(db, String(body.name ?? ""));
  return deckState(id);
});
route("GET", /^\/api\/decks\/(\d+)$/, ([id]) => deckState(Number(id)));
route("PATCH", /^\/api\/decks\/(\d+)$/, ([id], body) => {
  renameDeck(db, Number(id), String(body.name ?? ""));
  return deckState(Number(id));
});
route("DELETE", /^\/api\/decks\/(\d+)$/, ([id]) => {
  deleteDeck(db, Number(id));
  return { ok: true };
});

route("POST", /^\/api\/decks\/(\d+)\/cards$/, ([id], body) => {
  addCard(db, Number(id), String(body.oracle_id), {
    slotId: body.slot_id ?? null,
    role: body.role,
  });
  return deckState(Number(id));
});
route("PATCH", /^\/api\/decks\/(\d+)\/cards\/([0-9a-f-]+)$/, ([id, oid], body) => {
  updateCard(db, Number(id), oid, {
    slotId: body.slot_id,
    role: body.role,
    owned: body.owned,
    quantity: body.quantity,
    tagIds: body.tag_ids,
  });
  return deckState(Number(id));
});
route("DELETE", /^\/api\/decks\/(\d+)\/cards\/([0-9a-f-]+)$/, ([id, oid]) => {
  removeCard(db, Number(id), oid);
  return deckState(Number(id));
});

route("POST", /^\/api\/decks\/(\d+)\/slots$/, ([id], body) => {
  createSlot(db, Number(id), String(body.name ?? ""), body.target_min ?? null, body.target_max ?? null);
  return deckState(Number(id));
});
route("PATCH", /^\/api\/decks\/(\d+)\/slots\/(\d+)$/, ([id, sid], body) => {
  updateSlot(db, Number(id), Number(sid), {
    name: body.name,
    targetMin: body.target_min,
    targetMax: body.target_max,
  });
  return deckState(Number(id));
});
route("DELETE", /^\/api\/decks\/(\d+)\/slots\/(\d+)$/, ([id, sid]) => {
  deleteSlot(db, Number(id), Number(sid));
  return deckState(Number(id));
});

route("POST", /^\/api\/decks\/(\d+)\/tags$/, ([id], body) => {
  createTag(db, Number(id), String(body.name ?? ""));
  return deckState(Number(id));
});
route("PATCH", /^\/api\/decks\/(\d+)\/tags\/(\d+)$/, ([id, tid], body) => {
  renameTag(db, Number(id), Number(tid), String(body.name ?? ""));
  return deckState(Number(id));
});
route("DELETE", /^\/api\/decks\/(\d+)\/tags\/(\d+)$/, ([id, tid]) => {
  deleteTag(db, Number(id), Number(tid));
  return deckState(Number(id));
});

route("POST", /^\/api\/decks\/(\d+)\/proposals$/, ([id], body) => {
  createProposal(db, Number(id), body.items ?? [], { source: body.source, note: body.note });
  return deckState(Number(id));
});
route("GET", /^\/api\/decks\/(\d+)\/proposals$/, ([id], _b, url) => {
  const status = url.searchParams.get("status") as "open" | "resolved" | null;
  return listProposals(db, Number(id), status ?? undefined);
});
route("POST", /^\/api\/decks\/(\d+)\/items\/(\d+)\/accept$/, ([id, itemId]) => {
  acceptItem(db, Number(itemId));
  return deckState(Number(id));
});
route("POST", /^\/api\/decks\/(\d+)\/items\/(\d+)\/reject$/, ([id, itemId], body) => {
  rejectItem(db, Number(itemId), body.type, String(body.reason ?? ""));
  return deckState(Number(id));
});
route("POST", /^\/api\/decks\/(\d+)\/log\/(\d+)\/undo$/, ([id, logId]) => {
  undoDecision(db, Number(id), Number(logId));
  return deckState(Number(id));
});
route("GET", /^\/api\/decks\/(\d+)\/log$/, ([id], _b, url) =>
  getLog(db, Number(id), Math.min(Number(url.searchParams.get("limit") ?? 100), 500)),
);
route("GET", /^\/api\/decks\/(\d+)\/cards\/([0-9a-f-]+)\/history$/, ([id, oid]) =>
  getCardHistory(db, Number(id), oid),
);
route("DELETE", /^\/api\/decks\/(\d+)\/filters\/([0-9a-f-]+)$/, ([id, oid]) => {
  removeHardFilter(db, Number(id), oid);
  return deckState(Number(id));
});
route("POST", /^\/api\/decks\/(\d+)\/notes$/, ([id], body) => {
  addCardNote(db, Number(id), String(body.oracle_id), String(body.note ?? ""));
  return deckState(Number(id));
});

route("POST", /^\/api\/decks\/(\d+)\/audit$/, async ([id], body) => {
  const deckId = Number(id);
  const instructions = String(body.instructions ?? "");

  // Reasoning pass (§8.2) — graceful when no API key is configured.
  let reasoning: object | null = null;
  if (body.skip_reasoning !== true) {
    try {
      const cfg = getAgentConfig();
      reasoning = await runReasoningPass(
        db,
        deckId,
        instructions,
        openRouterTransport(cfg),
        cfg.retentionN,
        activeDismissals(db, deckId),
      );
    } catch (e: any) {
      if (e instanceof ConfigError || e instanceof LlmError) {
        reasoning = { summary: "", findings: [], dismissed: [], dropped: 0, error: e.message };
      } else {
        throw e;
      }
    }
  }
  return { ...runAudit(db, deckId, instructions, reasoning), reasoning };
});
route("POST", /^\/api\/decks\/(\d+)\/audit\/dismiss$/, ([id], body) => {
  dismissFinding(db, Number(id), String(body.key), body.type, String(body.reason ?? ""));
  return { state: deckState(Number(id)), audit: auditView(db, Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/audit\/undismiss$/, ([id], body) => {
  undismissFinding(db, Number(id), String(body.key));
  return { state: deckState(Number(id)), audit: auditView(db, Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/audit\/promote$/, ([id], body) => {
  promoteFinding(db, Number(id), String(body.key));
  return { state: deckState(Number(id)), audit: auditView(db, Number(id)) };
});

route("GET", /^\/api\/decks\/(\d+)\/brief$/, ([id]) => getBrief(db, Number(id)));
route("PUT", /^\/api\/decks\/(\d+)\/brief$/, ([id], body) => {
  updateBrief(db, Number(id), { thesis: body.thesis, constraints_md: body.constraints_md });
  return getBrief(db, Number(id));
});
route("POST", /^\/api\/decks\/(\d+)\/engines$/, ([id], body) => {
  setEngine(db, Number(id), String(body.name ?? ""), String(body.description ?? ""), body.pieces ?? []);
  return getBrief(db, Number(id));
});
route("DELETE", /^\/api\/decks\/(\d+)\/engines\/(\d+)$/, ([id, engineId]) => {
  removeEngine(db, Number(id), Number(engineId));
  return getBrief(db, Number(id));
});
route("POST", /^\/api\/decks\/(\d+)\/brief-edits\/(\d+)\/accept$/, ([id, editId]) => {
  acceptBriefEdit(db, Number(id), Number(editId));
  return { state: deckState(Number(id)), brief: getBrief(db, Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/brief-edits\/(\d+)\/reject$/, ([id, editId], body) => {
  rejectBriefEdit(db, Number(id), Number(editId), String(body.type ?? "soft"), String(body.reason ?? ""));
  return { state: deckState(Number(id)), brief: getBrief(db, Number(id)) };
});

route("GET", /^\/api\/decks\/(\d+)\/chat$/, ([id]) => getChatHistory(db, Number(id)));
route("POST", /^\/api\/decks\/(\d+)\/chat$/, async ([id], body) => {
  const cfg = getAgentConfig();
  const transport = openRouterTransport(cfg);
  const result = await runTurn(db, Number(id), String(body.message ?? ""), transport, cfg.retentionN);
  return { reply: result.reply, mutated: result.mutatedState, state: deckState(Number(id)) };
});
// Debug/verification: exactly what the agent sees (spec §10)
route("GET", /^\/api\/decks\/(\d+)\/context-preview$/, ([id]) => {
  const retentionN = Number(process.env.DECKBUILDER_RETENTION_N ?? 30);
  const ctx = assembleContext(db, Number(id), retentionN);
  return {
    system: ctx.system,
    tail_restate: ctx.tailRestate,
    transcript_messages: ctx.transcript.length,
  };
});
route("GET", /^\/api\/resolve$/, (_p, _b, url) =>
  resolveExactName(db, url.searchParams.get("name") ?? ""),
);

route("GET", /^\/api\/search$/, (_p, _b, url) => {
  const q = url.searchParams.get("q") ?? "";
  const deckParam = url.searchParams.get("deck");
  const filter = url.searchParams.get("filter") !== "off";
  const limit = Math.min(Number(url.searchParams.get("limit") ?? 30), 100);
  return search(db, q, {
    limit,
    deckId: deckParam ? Number(deckParam) : undefined,
    filterByDeckIdentity: filter,
  });
});

const MIME: Record<string, string> = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".ico": "image/x-icon",
};

function serveStatic(pathname: string, res: ServerResponse): boolean {
  if (!existsSync(WEB_DIST)) return false;
  const rel = pathname === "/" ? "index.html" : pathname.slice(1);
  const file = normalize(join(WEB_DIST, rel));
  if (!file.startsWith(WEB_DIST)) return false;
  const target = existsSync(file) ? file : join(WEB_DIST, "index.html");
  try {
    const content = readFileSync(target);
    res.writeHead(200, { "Content-Type": MIME[extname(target)] ?? "application/octet-stream" });
    res.end(content);
    return true;
  } catch {
    return false;
  }
}

async function readBody(req: IncomingMessage): Promise<any> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) chunks.push(chunk as Buffer);
  if (!chunks.length) return {};
  try {
    return JSON.parse(Buffer.concat(chunks).toString("utf-8"));
  } catch {
    throw new ServiceError("Invalid JSON body");
  }
}

const server = createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", `http://localhost:${PORT}`);
  const json = (status: number, data: unknown) => {
    res.writeHead(status, { "Content-Type": "application/json" });
    res.end(JSON.stringify(data));
  };

  try {
    for (const r of routes) {
      if (r.method !== req.method) continue;
      const m = url.pathname.match(r.pattern);
      if (!m) continue;
      const body = req.method === "GET" ? {} : await readBody(req);
      return json(200, await r.handler(m.slice(1), body, url));
    }
    if (req.method === "GET" && !url.pathname.startsWith("/api/") && serveStatic(url.pathname, res))
      return;
    json(404, { error: `Not found: ${req.method} ${url.pathname}` });
  } catch (e: any) {
    if (e instanceof ServiceError) return json(e.status, { error: e.message });
    if (e instanceof SearchError) return json(400, { error: e.message });
    if (e instanceof ConfigError) return json(400, { error: e.message });
    if (e instanceof AgentError) return json(502, { error: e.message });
    if (e instanceof LlmError) return json(502, { error: e.message });
    console.error(e);
    json(500, { error: "Internal error" });
  }
});

server.listen(PORT, () => {
  console.log(`Deck builder running at http://localhost:${PORT}`);
  if (!existsSync(WEB_DIST))
    console.log("(web/dist not found — run 'npm run app' to build the UI, or 'npm run dev:web' for the dev server)");
});
