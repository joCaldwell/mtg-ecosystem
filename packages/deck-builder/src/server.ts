import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { readFileSync, existsSync } from "node:fs";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { openDb, getRetentionN, setRetentionN } from "./db.ts";
import { AppError, ServiceError } from "./errors.ts";
import { search } from "./search/index.ts";
import {
  addCard,
  createDeck,
  createSlot,
  createTag,
  deleteDeck,
  deleteSlot,
  deleteTag,
  listDecks,
  removeCard,
  renameDeck,
  renameTag,
  updateCard,
  updateSlot,
} from "./deck/service.ts";
import { acceptItem, createProposal, listProposals, rejectItem } from "./deck/proposals.ts";
import { addCardNote, getCardHistory, getLog, removeHardFilter, undoDecision } from "./deck/log.ts";
import { deckState } from "./state.ts";
import {
  activeDismissals,
  auditState,
  dismissFinding,
  finishAuditRun,
  getAuditRun,
  promoteFinding,
  reclaimStaleRuns,
  runningAuditRun,
  startAuditRun,
  undismissFinding,
} from "./deck/audit.ts";
import { runReasoningPass } from "./agent/reasoning.ts";
import {
  acceptBriefEdit,
  getBrief,
  rejectBriefEdit,
  removeEngine,
  setEngine,
  updateBrief,
} from "./deck/brief.ts";
import {
  addPlaytestNote,
  applyImport,
  deletePlaytestNote,
  diffImport,
  exportDeck,
} from "./deck/interop.ts";
import { getChatHistory, runTurn } from "./agent/agent.ts";
import { assembleContext } from "./agent/context.ts";
import { contextMeter } from "./agent/meter.ts";
import { acceptConsolidation, rejectConsolidation, runConsolidation } from "./agent/consolidate.ts";
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

// Envelope rule: any response carrying a domain composite names it — `state`,
// `audit`, `brief`, `meter`, `consolidation`, `import` — and never spreads it
// at top level. One client-side applier can then route every response.
const state = (id: number) => deckState(db, id);

route("GET", /^\/api\/decks$/, () => listDecks(db));
route("POST", /^\/api\/decks$/, (_p, body) => {
  const id = createDeck(db, String(body.name ?? ""));
  return { state: state(id) };
});
route("GET", /^\/api\/decks\/(\d+)$/, ([id]) => ({ state: state(Number(id)) }));
route("PATCH", /^\/api\/decks\/(\d+)$/, ([id], body) => {
  renameDeck(db, Number(id), String(body.name ?? ""));
  return { state: state(Number(id)) };
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
  return { state: state(Number(id)) };
});
route("PATCH", /^\/api\/decks\/(\d+)\/cards\/([0-9a-f-]+)$/, ([id, oid], body) => {
  updateCard(db, Number(id), oid, {
    slotId: body.slot_id,
    role: body.role,
    owned: body.owned,
    quantity: body.quantity,
    tagIds: body.tag_ids,
  });
  return { state: state(Number(id)) };
});
route("DELETE", /^\/api\/decks\/(\d+)\/cards\/([0-9a-f-]+)$/, ([id, oid]) => {
  removeCard(db, Number(id), oid);
  return { state: state(Number(id)) };
});

route("POST", /^\/api\/decks\/(\d+)\/slots$/, ([id], body) => {
  createSlot(db, Number(id), String(body.name ?? ""), body.target_min ?? null, body.target_max ?? null);
  return { state: state(Number(id)) };
});
route("PATCH", /^\/api\/decks\/(\d+)\/slots\/(\d+)$/, ([id, sid], body) => {
  updateSlot(db, Number(id), Number(sid), {
    name: body.name,
    targetMin: body.target_min,
    targetMax: body.target_max,
  });
  return { state: state(Number(id)) };
});
route("DELETE", /^\/api\/decks\/(\d+)\/slots\/(\d+)$/, ([id, sid]) => {
  deleteSlot(db, Number(id), Number(sid));
  return { state: state(Number(id)) };
});

route("POST", /^\/api\/decks\/(\d+)\/tags$/, ([id], body) => {
  createTag(db, Number(id), String(body.name ?? ""));
  return { state: state(Number(id)) };
});
route("PATCH", /^\/api\/decks\/(\d+)\/tags\/(\d+)$/, ([id, tid], body) => {
  renameTag(db, Number(id), Number(tid), String(body.name ?? ""));
  return { state: state(Number(id)) };
});
route("DELETE", /^\/api\/decks\/(\d+)\/tags\/(\d+)$/, ([id, tid]) => {
  deleteTag(db, Number(id), Number(tid));
  return { state: state(Number(id)) };
});

route("POST", /^\/api\/decks\/(\d+)\/proposals$/, ([id], body) => {
  createProposal(db, Number(id), body.items ?? [], { source: body.source, note: body.note });
  return { state: state(Number(id)) };
});
route("GET", /^\/api\/decks\/(\d+)\/proposals$/, ([id], _b, url) => {
  const status = url.searchParams.get("status") as "open" | "resolved" | null;
  return listProposals(db, Number(id), status ?? undefined);
});
route("POST", /^\/api\/decks\/(\d+)\/items\/(\d+)\/accept$/, ([id, itemId]) => {
  acceptItem(db, Number(itemId));
  return { state: state(Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/items\/(\d+)\/reject$/, ([id, itemId], body) => {
  rejectItem(db, Number(itemId), body.type, String(body.reason ?? ""));
  return { state: state(Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/log\/(\d+)\/undo$/, ([id, logId]) => {
  undoDecision(db, Number(id), Number(logId));
  return { state: state(Number(id)) };
});
route("GET", /^\/api\/decks\/(\d+)\/log$/, ([id], _b, url) =>
  getLog(db, Number(id), Math.min(Number(url.searchParams.get("limit") ?? 100), 500)),
);
route("GET", /^\/api\/decks\/(\d+)\/cards\/([0-9a-f-]+)\/history$/, ([id, oid]) =>
  getCardHistory(db, Number(id), oid),
);
route("DELETE", /^\/api\/decks\/(\d+)\/filters\/([0-9a-f-]+)$/, ([id, oid]) => {
  removeHardFilter(db, Number(id), oid);
  return { state: state(Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/notes$/, ([id], body) => {
  addCardNote(db, Number(id), String(body.oracle_id), String(body.note ?? ""));
  return { state: state(Number(id)) };
});

// The reasoning pass is an LLM round-trip, so a run is a background job: the
// POST opens the run and returns, the client polls GET. Closing the tab, or
// the whole browser, no longer cancels an audit — the result lands in
// audit_runs either way and the section picks it back up.
async function executeAuditRun(deckId: number, runId: number, instructions: string) {
  try {
    const cfg = getAgentConfig();
    const reasoning = await runReasoningPass(
      db,
      deckId,
      instructions,
      openRouterTransport(cfg),
      getRetentionN(db),
      activeDismissals(db, deckId),
    );
    finishAuditRun(db, runId, reasoning);
  } catch (e: any) {
    // A missing key or a provider failure is not a failed run — the
    // deterministic half stands on its own and the run records why §8.2 is
    // missing from it.
    if (e instanceof ConfigError || e instanceof LlmError) {
      finishAuditRun(db, runId, {
        summary: "",
        findings: [],
        dismissed: [],
        dropped: 0,
        error: e.message,
      });
    } else {
      console.error(e);
      finishAuditRun(db, runId, null, e?.message ?? "Internal error");
    }
  }
}

route("GET", /^\/api\/decks\/(\d+)\/audit$/, ([id]) => ({ audit: auditState(db, Number(id)) }));
route("GET", /^\/api\/decks\/(\d+)\/audit\/runs\/(\d+)$/, ([id, runId]) => {
  const run = getAuditRun(db, Number(id), Number(runId));
  if (!run) throw new ServiceError(`No audit run #${runId} for this deck`, 404);
  return run;
});
route("POST", /^\/api\/decks\/(\d+)\/audit$/, ([id], body) => {
  const deckId = Number(id);
  const instructions = String(body.instructions ?? "");

  // One run at a time per deck: a second click while one is in flight joins
  // the run already going rather than paying for a duplicate reasoning pass.
  const inFlight = runningAuditRun(db, deckId);
  if (inFlight) return { run_id: inFlight.id, already_running: true, audit: auditState(db, deckId) };

  const runId = startAuditRun(db, deckId, instructions);
  if (body.skip_reasoning === true) finishAuditRun(db, runId, null);
  else void executeAuditRun(deckId, runId, instructions);
  return { run_id: runId, already_running: false, audit: auditState(db, deckId) };
});
route("POST", /^\/api\/decks\/(\d+)\/audit\/dismiss$/, ([id], body) => {
  dismissFinding(db, Number(id), String(body.key), body.type, String(body.reason ?? ""));
  return { state: state(Number(id)), audit: auditState(db, Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/audit\/undismiss$/, ([id], body) => {
  undismissFinding(db, Number(id), String(body.key));
  return { state: state(Number(id)), audit: auditState(db, Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/audit\/promote$/, ([id], body) => {
  promoteFinding(db, Number(id), String(body.key));
  return { state: state(Number(id)), audit: auditState(db, Number(id)) };
});

route("GET", /^\/api\/decks\/(\d+)\/brief$/, ([id]) => ({ brief: getBrief(db, Number(id)) }));
route("PUT", /^\/api\/decks\/(\d+)\/brief$/, ([id], body) => {
  updateBrief(db, Number(id), { thesis: body.thesis, constraints_md: body.constraints_md });
  return { brief: getBrief(db, Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/engines$/, ([id], body) => {
  setEngine(db, Number(id), String(body.name ?? ""), String(body.description ?? ""), body.pieces ?? []);
  return { brief: getBrief(db, Number(id)) };
});
route("DELETE", /^\/api\/decks\/(\d+)\/engines\/(\d+)$/, ([id, engineId]) => {
  removeEngine(db, Number(id), Number(engineId));
  return { brief: getBrief(db, Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/brief-edits\/(\d+)\/accept$/, ([id, editId]) => {
  acceptBriefEdit(db, Number(id), Number(editId));
  return { state: state(Number(id)), brief: getBrief(db, Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/brief-edits\/(\d+)\/reject$/, ([id, editId], body) => {
  rejectBriefEdit(db, Number(id), Number(editId), String(body.type ?? "soft"), String(body.reason ?? ""));
  return { state: state(Number(id)), brief: getBrief(db, Number(id)) };
});

route("GET", /^\/api\/decks\/(\d+)\/chat$/, ([id]) => getChatHistory(db, Number(id)));
route("POST", /^\/api\/decks\/(\d+)\/chat$/, async ([id], body) => {
  const cfg = getAgentConfig();
  const transport = openRouterTransport(cfg);
  const result = await runTurn(db, Number(id), String(body.message ?? ""), transport, getRetentionN(db));
  return { reply: result.reply, mutated: result.mutatedState, state: state(Number(id)) };
});
// Debug/verification: exactly what the agent sees (spec §10)
route("GET", /^\/api\/decks\/(\d+)\/context-preview$/, ([id]) => {
  const ctx = assembleContext(db, Number(id), getRetentionN(db));
  return {
    system: ctx.system,
    tail_restate: ctx.tailRestate,
    transcript_messages: ctx.transcript.length,
  };
});

// ---------- compaction (spec §11) ----------

route("GET", /^\/api\/decks\/(\d+)\/context-meter$/, ([id]) => ({
  meter: contextMeter(db, Number(id), getRetentionN(db)),
}));
route("POST", /^\/api\/decks\/(\d+)\/consolidate$/, async ([id]) => {
  const cfg = getAgentConfig();
  const consolidation = await runConsolidation(db, Number(id), openRouterTransport(cfg));
  return { consolidation, state: state(Number(id)) };
});
route("POST", /^\/api\/decks\/(\d+)\/consolidations\/(\d+)\/accept$/, ([id, cid]) => {
  const consolidation = acceptConsolidation(db, Number(id), Number(cid));
  return { consolidation, state: state(Number(id)), meter: contextMeter(db, Number(id), getRetentionN(db)) };
});
route("POST", /^\/api\/decks\/(\d+)\/consolidations\/(\d+)\/reject$/, ([id, cid]) => {
  const consolidation = rejectConsolidation(db, Number(id), Number(cid));
  return { consolidation, state: state(Number(id)), meter: contextMeter(db, Number(id), getRetentionN(db)) };
});

// ---------- settings (spec §12: retention N is tunable, not a constant) ----------

route("GET", /^\/api\/settings$/, () => ({ retention_n: getRetentionN(db) }));
route("PUT", /^\/api\/settings$/, (_p, body) => {
  if (body.retention_n !== undefined) setRetentionN(db, Number(body.retention_n));
  return { retention_n: getRetentionN(db) };
});

// ---------- Archidekt interop (spec §9) ----------

route("GET", /^\/api\/decks\/(\d+)\/export$/, ([id], _b, url) =>
  exportDeck(db, Number(id), {
    categories: url.searchParams.get("categories") !== "off",
    onlyUnowned: url.searchParams.get("unowned") === "1",
  }),
);
route("POST", /^\/api\/decks\/(\d+)\/import\/preview$/, ([id], body) =>
  diffImport(db, Number(id), String(body.text ?? "")),
);
// Applies in full and immediately (spec §9) — the preview diff is the
// confirmation, and the single log row it writes undoes the whole thing.
route("POST", /^\/api\/decks\/(\d+)\/import$/, ([id], body) => {
  const r = applyImport(db, Number(id), String(body.text ?? ""), body.note);
  return { import: r, state: state(Number(id)) };
});

route("POST", /^\/api\/decks\/(\d+)\/playtest-notes$/, ([id], body) => {
  addPlaytestNote(db, Number(id), String(body.note ?? ""));
  return { state: state(Number(id)) };
});
route("DELETE", /^\/api\/decks\/(\d+)\/playtest-notes\/(\d+)$/, ([id, noteId]) => {
  deletePlaytestNote(db, Number(id), Number(noteId));
  return { state: state(Number(id)) };
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
    if (e instanceof AppError) return json(e.status, { error: e.message });
    console.error(e);
    json(500, { error: "Internal error" });
  }
});

server.listen(PORT, () => {
  const stale = reclaimStaleRuns(db);
  if (stale) console.log(`Marked ${stale} audit run(s) failed — they were in flight at shutdown.`);
  console.log(`Deck builder running at http://localhost:${PORT}`);
  if (!existsSync(WEB_DIST))
    console.log("(web/dist not found — run 'npm run app' to build the UI, or 'npm run dev:web' for the dev server)");
});
