// The app's one HTTP server (spec §10.1): the UI, its JSON API, the live
// event stream, and the agent endpoint, all in the process that owns the
// database. The UI API acts as the owner; the agent endpoint acts as the
// calling agent; both call the same capability layer.

import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { existsSync, readFileSync, statSync } from "node:fs";
import { extname, join, resolve, sep } from "node:path";
import type { Database } from "./sqlite.ts";
import { OWNER } from "./actor.ts";
import { listActivity } from "./activity.ts";
import { cardsNamed, requireCardById, suggestNames } from "./cards.ts";
import { PKG_ROOT, SCHEMA_VERSION, withTransaction } from "./db.ts";
import { AppError, ServiceError } from "./errors.ts";
import { getFocus, setFocus } from "./focus.ts";
import { removeEngine, setEngine, updateBrief } from "./deck/brief.ts";
import { compareDecks, forkDeck } from "./deck/fork.ts";
import { applyImport, diffImport, exportDeck } from "./deck/interop.ts";
import { cardHistory, listLog, unreviewedCount } from "./deck/log.ts";
import { applyChanges, setQuantity, undo } from "./deck/membership.ts";
import { getPower, setPowerTarget, recordPowerAssessment } from "./deck/power.ts";
import {
  acceptItems,
  acceptProposals,
  listProposals,
  pendingItemIds,
  rejectItems,
} from "./deck/proposals.ts";
import {
  createDeck,
  createSlot,
  createTag,
  deleteDeck,
  deleteSlot,
  deleteTag,
  findDeckCard,
  listDecks,
  organizeCard,
  renameDeck,
  requireDeck,
  renameTag,
  setDeckCategory,
  setDeckParent,
  shortlistCard,
  unshortlistCard,
  updateSlot,
} from "./deck/service.ts";
import { getProfile, getDocumentEdit, listDocumentEdits, ruleDocumentEdit, setProfile } from "./documents.ts";
import { MCP_PATH, createAgentEndpoint } from "./mcp/http.ts";
import {
  getBudgets,
  getMemory,
  listMemories,
  loadScope,
  memoriesForCards,
  remember,
  reviseMemory,
  setBudgets,
} from "./memory.ts";
import { priceStatus } from "./prices.ts";
import type { Jobs } from "./jobs.ts";
import { SearchError, search } from "./search/index.ts";
import { deckState } from "./state.ts";
import { getPlaytest, startPlaytest, changePlaytest, savePlaytestUi, saveObservation, tableCard } from "./playtest/service.ts";
import { playerStats } from "./stats.ts";

export const APP_VERSION = "2.0.0";
const WEB_DIST = join(PKG_ROOT, "web", "dist");

export interface AppOptions {
  /** The agent token; see token.ts. */
  token: string;
  /** Replace the token on disk and return the new one. */
  rotateToken?: () => string;
  /** The port the server listens on, for the connection details shown in Settings. */
  port?: number;
  jobs?: Jobs;
  /** Testable static root; production uses the Vite output. */
  webDist?: string;
}

type Handler = (params: string[], body: any, url: URL) => unknown | Promise<unknown>;

const LOOPBACK = new Set(["127.0.0.1", "localhost", "[::1]", "::1"]);

// The UI's API has no token — it is the owner's own browser — so the two
// ways a web page elsewhere could reach it are closed here: a Host that is
// not loopback (DNS rebinding) and an Origin that is not loopback (a
// cross-site request).
function sameMachine(req: IncomingMessage): boolean {
  const host = (req.headers.host ?? "").replace(/:\d+$/, "");
  if (!LOOPBACK.has(host)) return false;
  const origin = req.headers.origin;
  if (!origin) return true;
  try {
    return LOOPBACK.has(new URL(origin).hostname);
  } catch {
    return false;
  }
}

/** Reject invalid numeric input before it reaches SQLite or arithmetic. */
function num(value: unknown, name = "number", min = 0): number {
  if ((typeof value !== "number" && typeof value !== "string") || String(value).trim() === "")
    throw new ServiceError(`${name} must be an integer`);
  const n = Number(value);
  if (!Number.isSafeInteger(n) || n < min) throw new ServiceError(`${name} must be an integer >= ${min}`);
  return n;
}

function validateNumbers(body: Record<string, any>) {
  for (const key of ["quantity", "slot_id", "deck_id", "parent_id", "target_min", "target_max", "expected_revision"])
    if (body[key] != null) body[key] = num(body[key], key, key === "quantity" ? 1 : 0);
  for (const key of ["tag_ids", "secondary_slot_ids"])
    if (body[key] !== undefined) {
      if (!Array.isArray(body[key])) throw new ServiceError(`${key} must be an array of integers`);
      body[key] = body[key].map((v: unknown) => num(v, key, 1));
    }
}

export function createApp(db: Database, opts: AppOptions) {
  let token = opts.token;
  const routes: Array<{ method: string; pattern: RegExp; handler: Handler; mutates: boolean }> = [];
  const route = (method: string, pattern: RegExp, handler: Handler) =>
    routes.push({ method, pattern, handler, mutates: method !== "GET" });

  // ---------- live updates (spec §11) ----------
  // The stream says only "something changed"; the page refetches what it is
  // showing. A proposal made in a terminal appears without a reload.
  const listeners = new Set<ServerResponse>();
  let version = 0;
  const notify = () => {
    version++;
    for (const res of listeners) res.write(`data: ${version}\n\n`);
  };

  let agents = createAgentEndpoint(db, { token, version: APP_VERSION, onChange: notify });

  // Envelope rule: a response carrying a domain composite names it — `state`,
  // `profile`, `memories` — and never spreads it at top level, so one
  // client-side applier can route every response.
  const state = (id: number) => deckState(db, id);

  // ---------- decks ----------
  route("GET", /^\/api\/health$/, () => ({
    ok: true, schema_version: SCHEMA_VERSION,
    cards: (db.prepare("SELECT COUNT(*) n FROM cards").get() as { n: number }).n,
    last_ingest: (db.prepare("SELECT value FROM meta WHERE key = 'last_ingest'").get() as { value: string } | undefined)?.value ?? null,
  }));
  route("GET", /^\/api\/decks$/, () => listDecks(db));
  route("POST", /^\/api\/decks$/, (_p, body) => withTransaction(db, () => {
    const id = createDeck(db, OWNER, String(body.name ?? ""), { category: body.category });
    if (body.category !== undefined) setDeckCategory(db, OWNER, id, body.category);
    return { state: state(id) };
  }));
  route("GET", /^\/api\/decks\/(\d+)$/, ([id]) => ({ state: state(num(id)) }));
  route("PATCH", /^\/api\/decks\/(\d+)$/, ([id], body) => withTransaction(db, () => {
    if (body.name !== undefined) renameDeck(db, OWNER, num(id), String(body.name));
    if (body.category !== undefined) setDeckCategory(db, OWNER, num(id), body.category);
    if (body.parent_id !== undefined)
      setDeckParent(db, OWNER, num(id), body.parent_id == null ? null : num(body.parent_id));
    return { state: state(num(id)) };
  }));
  route("DELETE", /^\/api\/decks\/(\d+)$/, ([id]) => {
    deleteDeck(db, OWNER, num(id));
    return { ok: true };
  });
  route("POST", /^\/api\/decks\/(\d+)\/fork$/, ([id], body) => ({
    state: state(forkDeck(db, OWNER, num(id), String(body.name ?? ""))),
  }));
  route("GET", /^\/api\/decks\/(\d+)\/compare$/, ([id], _b, url) => {
    const other = url.searchParams.get("other");
    return compareDecks(db, num(id), other ? num(other) : undefined);
  });

  route("GET", /^\/api\/cards\/([0-9a-z-]+)\/table$/, ([id]) => ({ card: tableCard(db, id) }));
  // Owner's private solo table: capability-layer permissions and validation.
  route("GET", /^\/api\/decks\/(\d+)\/playtest$/, ([id]) => ({ playtest: getPlaytest(db, OWNER, num(id)) }));
  route("POST", /^\/api\/decks\/(\d+)\/playtest$/, ([id], body) => ({ playtest: startPlaytest(db, OWNER, num(id), body) }));
  route("POST", /^\/api\/decks\/(\d+)\/playtest\/actions$/, ([id], body) => ({ playtest: changePlaytest(db, OWNER, num(id), body) }));
  route("PATCH", /^\/api\/decks\/(\d+)\/playtest\/ui$/, ([id], body) => ({ playtest: savePlaytestUi(db, OWNER, num(id), body) }));
  route("POST", /^\/api\/decks\/(\d+)\/playtest\/observations$/, ([id], body) => ({ memory: saveObservation(db, OWNER, num(id), body) }));

  // ---------- cards in a deck ----------
  route("POST", /^\/api\/decks\/(\d+)\/cards$/, ([id], body) => withTransaction(db, () => {
    const deckId = num(id);
    const oracleId = String(body.oracle_id);
    if (body.maybe) {
      shortlistCard(db, deckId, oracleId, String(body.maybe_note ?? ""));
      organizeCard(db, deckId, oracleId, { slotId: body.slot_id, secondarySlotIds: body.secondary_slot_ids });
    }
    else
      applyChanges(
        db,
        OWNER,
        deckId,
        [{ action: "add", oracle_id: oracleId, slot_id: body.slot_id ?? undefined, secondary_slot_ids: body.secondary_slot_ids, role: body.role, quantity: body.quantity }],
        { kind: "edit" },
      );
    return { state: state(deckId) };
  }));
  route("PATCH", /^\/api\/decks\/(\d+)\/cards\/([0-9a-z-]+)$/, ([id, oid], body) => withTransaction(db, () => {
    const deckId = num(id);
    if (body.maybe !== undefined) {
      const row = findDeckCard(db, deckId, oid);
      if (!row) throw new ServiceError(`${requireCardById(db, oid).name} is not in this deck`, 404);
      if (body.maybe && !row.maybe)
        applyChanges(db, OWNER, deckId, [{ action: "cut", oracle_id: oid, park: true }], { kind: "edit" });
      if (!body.maybe && row.maybe)
        applyChanges(db, OWNER, deckId, [{ action: "add", oracle_id: oid }], { kind: "edit" });
    }
    if (body.maybe_note !== undefined) shortlistCard(db, deckId, oid, String(body.maybe_note));
    if (body.role !== undefined)
      applyChanges(db, OWNER, deckId, [{ action: "role", oracle_id: oid, role: body.role }], { kind: "edit" });
    if (body.quantity !== undefined) setQuantity(db, OWNER, deckId, oid, num(body.quantity));
    if (body.slot_id !== undefined || body.secondary_slot_ids !== undefined || body.tag_ids !== undefined)
      organizeCard(db, deckId, oid, { slotId: body.slot_id, secondarySlotIds: body.secondary_slot_ids, tagIds: body.tag_ids });
    return { state: state(deckId) };
  }));
  route("DELETE", /^\/api\/decks\/(\d+)\/cards\/([0-9a-z-]+)$/, ([id, oid]) => {
    const deckId = num(id);
    const row = findDeckCard(db, deckId, oid);
    if (row?.maybe) unshortlistCard(db, deckId, oid);
    else applyChanges(db, OWNER, deckId, [{ action: "cut", oracle_id: oid, quantity: "all" }], { kind: "edit" });
    return { state: state(deckId) };
  });
  route("GET", /^\/api\/decks\/(\d+)\/cards\/([0-9a-z-]+)\/history$/, ([id, oid]) => ({
    rulings: cardHistory(db, num(id), oid, 20),
    memories: memoriesForCards(db, [oid], num(id)).get(oid) ?? [],
  }));

  // ---------- slots and tags ----------
  route("POST", /^\/api\/decks\/(\d+)\/slots$/, ([id], body) => {
    createSlot(db, num(id), String(body.name ?? ""), body.target_min ?? null, body.target_max ?? null);
    return { state: state(num(id)) };
  });
  route("PATCH", /^\/api\/decks\/(\d+)\/slots\/(\d+)$/, ([id, sid], body) => {
    updateSlot(db, num(id), num(sid), {
      name: body.name,
      targetMin: body.target_min,
      targetMax: body.target_max,
    });
    return { state: state(num(id)) };
  });
  route("DELETE", /^\/api\/decks\/(\d+)\/slots\/(\d+)$/, ([id, sid]) => {
    deleteSlot(db, num(id), num(sid));
    return { state: state(num(id)) };
  });
  route("POST", /^\/api\/decks\/(\d+)\/tags$/, ([id], body) => {
    createTag(db, num(id), String(body.name ?? ""));
    return { state: state(num(id)) };
  });
  route("PATCH", /^\/api\/decks\/(\d+)\/tags\/(\d+)$/, ([id, tid], body) => {
    renameTag(db, num(id), num(tid), String(body.name ?? ""));
    return { state: state(num(id)) };
  });
  route("DELETE", /^\/api\/decks\/(\d+)\/tags\/(\d+)$/, ([id, tid]) => {
    deleteTag(db, num(id), num(tid));
    return { state: state(num(id)) };
  });

  // ---------- proposals and rulings (spec §8) ----------
  const inbox = () => ({
    proposals: listProposals(db, { status: "open" }).filter(p => requireDeck(db, p.deck_id).category !== "legacy"),
    document_edits: listDocumentEdits(db, { status: "pending" }).filter(e => e.deck_id == null || requireDeck(db, e.deck_id).category !== "legacy"),
    unreviewed: unreviewedCount(db),
  });
  route("GET", /^\/api\/inbox$/, inbox);
  route("GET", /^\/api\/proposal-history$/, () => ({
    proposals: listProposals(db), document_edits: listDocumentEdits(db),
  }));
  const rule = (ids: number[], body: any) =>
    body.outcome === "accept"
      ? acceptItems(db, OWNER, ids, String(body.reason ?? ""), { expected_revision: body.expected_revision, acknowledge_stale: body.acknowledge_stale })
      : body.outcome === "reject"
        ? rejectItems(db, OWNER, ids, String(body.reason ?? ""))
        : (() => {
            throw new ServiceError("A ruling's outcome is 'accept' or 'reject'");
          })();
  const assertItemsInDeck = (deckId: number, ids: number[]) => {
    requireDeck(db, deckId);
    const belongs = db.prepare("SELECT 1 FROM proposal_items i JOIN proposals p ON p.id = i.proposal_id WHERE i.id = ? AND p.deck_id = ?");
    if (ids.some(id => !belongs.get(id, deckId))) throw new ServiceError("Every proposal item must belong to this deck");
  };
  route("POST", /^\/api\/decks\/(\d+)\/items\/rule$/, ([id], body) => {
    if (!Array.isArray(body.item_ids) || !body.item_ids.length || !body.item_ids.every((v: unknown) => typeof v === "number" && Number.isSafeInteger(v) && v > 0))
      throw new ServiceError("item_ids must be a non-empty array of integers");
    assertItemsInDeck(num(id), body.item_ids);
    rule(body.item_ids, body);
    return { state: state(num(id)), inbox: inbox() };
  });
  route("POST", /^\/api\/decks\/(\d+)\/proposals\/rule$/, ([id], body) => {
    if (!Array.isArray(body.proposals) || !body.proposals.every((p: any) => p && typeof p === "object" && Number.isSafeInteger(p.proposal_id) && p.proposal_id > 0 && (p.acknowledge_stale === undefined || typeof p.acknowledge_stale === "boolean")))
      throw new ServiceError("proposals must be an array of proposal IDs and optional stale acknowledgements");
    acceptProposals(db, OWNER, num(id), body.proposals, num(body.expected_revision, "expected_revision"));
    return { state: state(num(id)), inbox: inbox() };
  });
  route("POST", /^\/api\/decks\/(\d+)\/proposals\/(\d+)\/rule$/, ([id, pid], body) => {
    const ids = pendingItemIds(db, num(pid));
    assertItemsInDeck(num(id), ids);
    rule(ids, body);
    return { state: state(num(id)), inbox: inbox() };
  });
  route("POST", /^\/api\/document-edits\/(\d+)\/rule$/, ([editId], body) => {
    if (body.outcome !== "accept" && body.outcome !== "reject")
      throw new ServiceError("A ruling's outcome is 'accept' or 'reject'");
    const edit = getDocumentEdit(db, num(editId));
    ruleDocumentEdit(db, OWNER, num(editId), body.outcome, String(body.reason ?? ""), { expected_revision: body.expected_revision, acknowledge_stale: body.acknowledge_stale, expected_documents: body.expected_documents });
    return {
      ...(edit?.deck_id != null ? { state: state(edit.deck_id) } : { profile: getProfile(db) }),
      inbox: inbox(),
    };
  });
  route("POST", /^\/api\/decks\/(\d+)\/log\/(\d+)\/undo$/, ([id, logId]) => {
    if (!db.prepare("SELECT 1 FROM log WHERE id = ? AND deck_id = ?").get(num(logId), num(id)))
      throw new ServiceError("Log entry does not belong to this deck");
    const result = undo(db, OWNER, num(logId));
    return { state: state(num(id)), skipped: result.skipped };
  });
  route("GET", /^\/api\/log$/, (_p, _b, url) =>
    listLog(db, {
      deckId: url.searchParams.get("deck") ? num(url.searchParams.get("deck")) : undefined,
      unreviewedOnly: url.searchParams.get("unreviewed") === "1",
      limit: num(url.searchParams.get("limit") ?? 100),
    }),
  );

  // ---------- deck power (spec §5.7) ----------
  route("GET", /^\/api\/decks\/(\d+)\/power$/, ([id]) => getPower(db, num(id)));
  route("PUT", /^\/api\/decks\/(\d+)\/power\/target$/, ([id], body) => {
    setPowerTarget(db, OWNER, num(id), body.target);
    return { state: state(num(id)) };
  });
  route("POST", /^\/api\/decks\/(\d+)\/power\/assessments$/, ([id], body) => {
    const assessmentId = recordPowerAssessment(db, OWNER, num(id), body);
    return { assessment_id: assessmentId, state: state(num(id)) };
  });

  // ---------- documents (spec §6) ----------
  route("PUT", /^\/api\/decks\/(\d+)\/brief$/, ([id], body) => {
    updateBrief(db, OWNER, num(id), { thesis: body.thesis, constraints_md: body.constraints_md });
    return { state: state(num(id)) };
  });
  route("POST", /^\/api\/decks\/(\d+)\/engines$/, ([id], body) => {
    setEngine(db, OWNER, num(id), String(body.name ?? ""), String(body.description ?? ""), body.pieces ?? []);
    return { state: state(num(id)) };
  });
  route("DELETE", /^\/api\/decks\/(\d+)\/engines\/(\d+)$/, ([id, engineId]) => {
    removeEngine(db, OWNER, num(id), num(engineId));
    return { state: state(num(id)) };
  });
  route("GET", /^\/api\/profile$/, () => ({ profile: getProfile(db) }));
  route("PUT", /^\/api\/profile$/, (_p, body) => {
    setProfile(db, OWNER, String(body.body_md ?? ""));
    return { profile: getProfile(db) };
  });

  // ---------- memories (spec §7.5) ----------
  route("GET", /^\/api\/memories$/, (_p, _b, url) => {
    const q = url.searchParams;
    const global = loadScope(db, "global");
    return {
      memories: listMemories(db, {
        scope: (q.get("scope") as "global" | "deck" | null) ?? undefined,
        deckId: q.get("deck") ? num(q.get("deck")) : undefined,
        cardId: q.get("card") ?? undefined,
        query: q.get("q") ?? undefined,
        status: (q.get("status") as any) ?? "active",
        limit: 500,
      }),
      budgets: getBudgets(db),
      global_budget: { budget: global.budget, used: global.used, omitted: global.omitted },
    };
  });
  route("POST", /^\/api\/memories$/, (_p, body) => ({
    memory: remember(db, OWNER, {
      text: String(body.text ?? ""),
      scope: body.scope,
      deck_id: body.deck_id ?? null,
      cards: body.cards ?? [],
      basis: body.basis,
    }).memory,
  }));
  route("PATCH", /^\/api\/memories\/(\d+)$/, ([id], body) => ({
    memory: reviseMemory(db, OWNER, num(id), body),
  }));
  route("GET", /^\/api\/memories\/(\d+)$/, ([id]) => ({ memory: getMemory(db, num(id)) }));

  // ---------- activity, stats, focus, settings ----------
  route("GET", /^\/api\/activity$/, (_p, _b, url) =>
    listActivity(db, num(url.searchParams.get("limit") ?? 200)),
  );
  route("GET", /^\/api\/stats$/, () => playerStats(db));
  route("GET", /^\/api\/focus$/, () => ({ focus: getFocus(db) }));
  const settings = () => {
    const cards = db.prepare("SELECT COUNT(*) n FROM cards").get() as { n: number };
    const ingested = db.prepare("SELECT value FROM meta WHERE key = 'last_ingest'").get() as
      | { value: string }
      | undefined;
    return {
      jobs: opts.jobs?.status() ?? null,
      budgets: getBudgets(db),
      cards: { count: cards.n, updated_at: ingested?.value ?? null, prices: priceStatus(db) },
      agent: {
        url: `http://127.0.0.1:${opts.port ?? 8787}${MCP_PATH}`,
        token,
        stdio_command: `node ${join(PKG_ROOT, "src", "mcp", "stdio.ts")}`,
      },
    };
  };
  route("GET", /^\/api\/settings$/, settings);
  route("PUT", /^\/api\/settings$/, (_p, body) => {
    if (body.budgets) setBudgets(db, body.budgets);
    return settings();
  });
  route("POST", /^\/api\/settings\/rotate-token$/, async () => {
    if (!opts.rotateToken) throw new ServiceError("This server cannot rotate its token");
    const nextToken = opts.rotateToken();
    await agents.close();
    token = nextToken;
    agents = createAgentEndpoint(db, { token, version: APP_VERSION, onChange: notify });
    return settings();
  });
  route("POST", /^\/api\/cards\/refresh$/, () => {
    if (!opts.jobs) throw new ServiceError("Background jobs are not configured", 503);
    opts.jobs.enqueue("refresh");
    return settings();
  });

  // ---------- Archidekt interop (spec §12) ----------
  route("GET", /^\/api\/decks\/(\d+)\/export$/, ([id], _b, url) =>
    exportDeck(db, num(id), {
      categories: url.searchParams.get("categories") !== "off",
      oracleIds: url.searchParams.has("oracle_ids") ? url.searchParams.get("oracle_ids")!.split(",").filter(Boolean) : undefined,
      includeMaybe: url.searchParams.get("include_maybe") === "on",
    }),
  );
  route("POST", /^\/api\/decks\/(\d+)\/import\/preview$/, ([id], body) =>
    diffImport(db, num(id), String(body.text ?? "")),
  );
  route("POST", /^\/api\/decks\/(\d+)\/import$/, ([id], body) => {
    const r = applyImport(db, OWNER, num(id), String(body.text ?? ""), body.note);
    return { import: r, state: state(num(id)) };
  });

  // ---------- card search ----------
  route("GET", /^\/api\/resolve$/, (_p, _b, url) => {
    const name = url.searchParams.get("name") ?? "";
    const matches = cardsNamed(db, name);
    return { matches, suggestions: matches.length ? [] : suggestNames(db, name) };
  });
  route("GET", /^\/api\/search$/, (_p, _b, url) => {
    const q = url.searchParams.get("q") ?? "";
    const deckParam = url.searchParams.get("deck");
    try {
      return search(db, q, {
        limit: Math.min(num(url.searchParams.get("limit") ?? 30), 100),
        deckId: deckParam ? num(deckParam) : undefined,
        filterByDeckIdentity: url.searchParams.get("filter") !== "off",
      });
    } catch (e) {
      if (e instanceof SearchError) throw new ServiceError(e.message);
      throw e;
    }
  });

  // Focus is reported by the page, constantly, and changes nothing an agent
  // or another tab needs to redraw for, so it is the one write that does not
  // notify.
  const quiet = new Set(["/api/focus"]);
  route("PUT", /^\/api\/focus$/, (_p, body) => {
    setFocus(db, body.deck_id ?? null, Array.isArray(body.selected) ? body.selected.map(String) : []);
    return { ok: true };
  });

  // ---------- plumbing ----------
  const MIME: Record<string, string> = {
    ".html": "text/html",
    ".js": "text/javascript",
    ".css": "text/css",
    ".svg": "image/svg+xml",
    ".ico": "image/x-icon",
    ".woff2": "font/woff2",
    ".png": "image/png",
  };

  function serveStatic(pathname: string, res: ServerResponse): boolean {
    const root = resolve(opts.webDist ?? WEB_DIST);
    if (pathname.startsWith("/mcp") || /^\/(api|\.well-known)(\/|$)/.test(pathname)) return false;
    const file = resolve(root, pathname === "/" ? "index.html" : pathname.slice(1));
    if (!file.startsWith(root + sep)) return false;
    const appRoute = /^\/(?:decks|inbox|memories|profile|activity|settings|design|signin|privacy|terms|deck\/[^/]+(?:\/(?:review|playtest))?|[du]\/[^/]+)?$/.test(pathname);
    const target = existsSync(file) && statSync(file).isFile() ? file :
      appRoute && !extname(pathname) ? join(root, "index.html") : null;
    if (!target || !existsSync(target)) return false;
    try {
      const content = readFileSync(target);
      res.writeHead(200, {
        "Content-Type": MIME[extname(target)] ?? "application/octet-stream",
        "Cache-Control": pathname.startsWith("/assets/") ? "public, max-age=31536000, immutable" : "no-cache",
      });
      res.end(content);
      return true;
    } catch { return false; }
  }

  function readJson(req: IncomingMessage): Promise<Record<string, any>> {
    return new Promise((resolve, reject) => {
      const chunks: Buffer[] = [];
      let size = 0, failed = false;
      const fail = (error: Error) => { failed = true; chunks.length = 0; reject(error); };
      req.on("data", (chunk: Buffer) => {
        if (failed) return;
        size += chunk.length;
        if (size > 1024 * 1024) return fail(new ServiceError("JSON body exceeds 1 MiB", 413));
        const type = req.headers["content-type"]?.split(";")[0].trim().toLowerCase();
        if (type !== "application/json") return fail(new ServiceError("Content-Type must be application/json", 415));
        chunks.push(chunk);
      });
      req.on("error", reject);
      req.on("end", () => {
        if (failed) return;
        if (!chunks.length) return resolve({});
        try {
          const value: unknown = JSON.parse(Buffer.concat(chunks).toString("utf8"));
          if (!value || typeof value !== "object" || Array.isArray(value)) throw Error();
          resolve(value as Record<string, any>);
        } catch { reject(new ServiceError("JSON body must be a valid object")); }
      });
    });
  }

  const server: Server = createServer(async (req, res) => {
    const url = new URL(req.url ?? "/", "http://127.0.0.1");
    const json = (status: number, data: unknown) => {
      res.writeHead(status, { "Content-Type": "application/json" });
      res.end(JSON.stringify(data));
    };

    try {
      if (url.pathname === MCP_PATH || url.pathname.startsWith(`${MCP_PATH}/`))
        return await agents.handle(req, res);

      if (!sameMachine(req)) return json(403, { error: "This app only answers requests from this machine." });

      if (req.method === "GET" && url.pathname === "/api/events") {
        res.writeHead(200, {
          "Content-Type": "text/event-stream",
          "Cache-Control": "no-cache",
          Connection: "keep-alive",
          "X-Accel-Buffering": "no",
        });
        res.write(`data: ${version}\n\n`);
        listeners.add(res);
        const heartbeat = setInterval(() => res.write(": heartbeat\n\n"), 25_000);
        heartbeat.unref();
        res.on("close", () => { clearInterval(heartbeat); listeners.delete(res); });
        return;
      }

      for (const r of routes) {
        if (r.method !== req.method) continue;
        const m = url.pathname.match(r.pattern);
        if (!m) continue;
        const body = req.method === "GET" ? {} : await readJson(req);
        validateNumbers(body);
        const result = await r.handler(m.slice(1), body, url);
        if (r.mutates && !quiet.has(url.pathname)) notify();
        return json(200, result);
      }
      if (req.method === "GET" && !url.pathname.startsWith("/api/") && serveStatic(url.pathname, res))
        return;
      json(404, { error: `Not found: ${req.method} ${url.pathname}` });
    } catch (e: any) {
      if (res.headersSent) return void res.end();
      if (e instanceof AppError) return json(e.status, { error: e.message });
      console.error(e);
      json(500, { error: "Internal error" });
    }
  });

  return {
    server,
    notify,
    close: async () => {
      await agents.close();
      await new Promise<void>((resolve) => {
        for (const res of listeners) res.end();
        server.close(() => resolve());
        server.closeAllConnections?.();
      });
    },
  };
}
