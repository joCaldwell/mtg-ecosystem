// The app's one HTTP server (spec §10.1): the UI, its JSON API, the live
// event stream, and the agent endpoint, all in the process that owns the
// database. Every request resolves a principal and both doors call the same
// actor-scoped capability layer.

import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { existsSync, readFileSync, statSync } from "node:fs";
import { extname, join, resolve, sep } from "node:path";
import type { Database } from "./sqlite.ts";
import { type Actor, requireAdmin, requireDeckOwner } from "./actor.ts";
import { allowedRequest, getMode, localActor, publicOrigin, type Mode } from "./mode.ts";
import { createToken, listTokens, revokeToken } from "./tokens.ts";
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
  mode?: Mode;
  /** Phase 2 plugs session authentication into this request-to-Actor seam. */
  resolveUiActor?: (req: IncomingMessage, mode: Mode) => Actor | null | Promise<Actor | null>;
  /** The port the server listens on, for the connection details shown in Settings. */
  port?: number;
  jobs?: Jobs;
  /** Testable static root; production uses the Vite output. */
  webDist?: string;
}

type Handler = (actor: Actor, params: string[], body: any, url: URL) => unknown | Promise<unknown>;

/** Phase 2 replaces the hosted null with its cookie-session resolver. */
export function resolveUiActor(_req: IncomingMessage, mode: Mode): Actor | null {
  return mode === "local" ? localActor() : null;
}

export interface Change { kind: string; deck_id?: number }

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

export function createApp(db: Database, opts: AppOptions = {}) {
  const mode = opts.mode ?? getMode();
  const routes: Array<{ method: string; pattern: RegExp; handler: Handler; mutates: boolean }> = [];
  const route = (method: string, pattern: RegExp, handler: Handler) =>
    routes.push({ method, pattern, handler, mutates: method !== "GET" && !pattern.source.includes("preview") });

  // ---------- live updates (spec §11) ----------
  // The stream says only "something changed"; the page refetches what it is
  // showing. A proposal made in a terminal appears without a reload.
  const listeners = new Map<number, Set<ServerResponse>>();
  const notify = (userId: number, change: Change) => {
    for (const res of listeners.get(userId) ?? []) res.write(`data: ${JSON.stringify(change)}\n\n`);
  };
  const notifyAll = (change: Change) => { for (const id of listeners.keys()) notify(id, change); };
  const agents = createAgentEndpoint(db, { mode, version: APP_VERSION, onChange: notify });

  // Envelope rule: a response carrying a domain composite names it — `state`,
  // `profile`, `memories` — and never spreads it at top level, so one
  // client-side applier can route every response.
  const state = (actor: Actor, id: number) => deckState(db, actor, id);

  // ---------- decks ----------
  route("GET", /^\/api\/health$/, (actor) => ({
    ok: true, schema_version: SCHEMA_VERSION,
    cards: (db.prepare("SELECT COUNT(*) n FROM cards").get() as { n: number }).n,
    last_ingest: (db.prepare("SELECT value FROM meta WHERE key = 'last_ingest'").get() as { value: string } | undefined)?.value ?? null,
  }));
  route("GET", /^\/api\/decks$/, (actor) => listDecks(db, actor));
  route("POST", /^\/api\/decks$/, (actor, _p, body) => withTransaction(db, () => {
    const id = createDeck(db, actor, String(body.name ?? ""), { category: body.category });
    if (body.category !== undefined) setDeckCategory(db, actor, id, body.category);
    return { state: state(actor, id) };
  }));
  route("GET", /^\/api\/decks\/(\d+)$/, (actor, [id]) => ({ state: state(actor, num(id)) }));
  route("PATCH", /^\/api\/decks\/(\d+)$/, (actor, [id], body) => withTransaction(db, () => {
    if (body.name !== undefined) renameDeck(db, actor, num(id), String(body.name));
    if (body.category !== undefined) setDeckCategory(db, actor, num(id), body.category);
    if (body.parent_id !== undefined)
      setDeckParent(db, actor, num(id), body.parent_id == null ? null : num(body.parent_id));
    return { state: state(actor, num(id)) };
  }));
  route("DELETE", /^\/api\/decks\/(\d+)$/, (actor, [id]) => {
    deleteDeck(db, actor, num(id));
    return { ok: true };
  });
  route("POST", /^\/api\/decks\/(\d+)\/fork$/, (actor, [id], body) => ({
    state: state(actor, forkDeck(db, actor, num(id), String(body.name ?? ""))),
  }));
  route("GET", /^\/api\/decks\/(\d+)\/compare$/, (actor, [id], _b, url) => {
    const other = url.searchParams.get("other");
    return compareDecks(db, actor, num(id), other ? num(other) : undefined);
  });

  route("GET", /^\/api\/cards\/([0-9a-z-]+)\/table$/, (actor, [id]) => ({ card: tableCard(db, actor, id) }));
  // Owner's private solo table: capability-layer permissions and validation.
  route("GET", /^\/api\/decks\/(\d+)\/playtest$/, (actor, [id]) => ({ playtest: getPlaytest(db, actor, num(id)) }));
  route("POST", /^\/api\/decks\/(\d+)\/playtest$/, (actor, [id], body) => ({ playtest: startPlaytest(db, actor, num(id), body) }));
  route("POST", /^\/api\/decks\/(\d+)\/playtest\/actions$/, (actor, [id], body) => ({ playtest: changePlaytest(db, actor, num(id), body) }));
  route("PATCH", /^\/api\/decks\/(\d+)\/playtest\/ui$/, (actor, [id], body) => ({ playtest: savePlaytestUi(db, actor, num(id), body) }));
  route("POST", /^\/api\/decks\/(\d+)\/playtest\/observations$/, (actor, [id], body) => ({ memory: saveObservation(db, actor, num(id), body) }));

  // ---------- cards in a deck ----------
  route("POST", /^\/api\/decks\/(\d+)\/cards$/, (actor, [id], body) => withTransaction(db, () => {
    const deckId = num(id);
    const oracleId = String(body.oracle_id);
    if (body.maybe) {
      shortlistCard(db, actor, deckId, oracleId, String(body.maybe_note ?? ""));
      organizeCard(db, actor, deckId, oracleId, { slotId: body.slot_id, secondarySlotIds: body.secondary_slot_ids });
    }
    else
      applyChanges(
        db,
        actor,
        deckId,
        [{ action: "add", oracle_id: oracleId, slot_id: body.slot_id ?? undefined, secondary_slot_ids: body.secondary_slot_ids, role: body.role, quantity: body.quantity }],
        { kind: "edit" },
      );
    return { state: state(actor, deckId) };
  }));
  route("PATCH", /^\/api\/decks\/(\d+)\/cards\/([0-9a-z-]+)$/, (actor, [id, oid], body) => withTransaction(db, () => {
    const deckId = num(id);
    if (body.maybe !== undefined) {
      const row = findDeckCard(db, actor, deckId, oid);
      if (!row) throw new ServiceError(`${requireCardById(db, oid).name} is not in this deck`, 404);
      if (body.maybe && !row.maybe)
        applyChanges(db, actor, deckId, [{ action: "cut", oracle_id: oid, park: true }], { kind: "edit" });
      if (!body.maybe && row.maybe)
        applyChanges(db, actor, deckId, [{ action: "add", oracle_id: oid }], { kind: "edit" });
    }
    if (body.maybe_note !== undefined) shortlistCard(db, actor, deckId, oid, String(body.maybe_note));
    if (body.role !== undefined)
      applyChanges(db, actor, deckId, [{ action: "role", oracle_id: oid, role: body.role }], { kind: "edit" });
    if (body.quantity !== undefined) setQuantity(db, actor, deckId, oid, num(body.quantity));
    if (body.slot_id !== undefined || body.secondary_slot_ids !== undefined || body.tag_ids !== undefined)
      organizeCard(db, actor, deckId, oid, { slotId: body.slot_id, secondarySlotIds: body.secondary_slot_ids, tagIds: body.tag_ids });
    return { state: state(actor, deckId) };
  }));
  route("DELETE", /^\/api\/decks\/(\d+)\/cards\/([0-9a-z-]+)$/, (actor, [id, oid]) => {
    const deckId = num(id);
    const row = findDeckCard(db, actor, deckId, oid);
    if (row?.maybe) unshortlistCard(db, actor, deckId, oid);
    else applyChanges(db, actor, deckId, [{ action: "cut", oracle_id: oid, quantity: "all" }], { kind: "edit" });
    return { state: state(actor, deckId) };
  });
  route("GET", /^\/api\/decks\/(\d+)\/cards\/([0-9a-z-]+)\/history$/, (actor, [id, oid]) => ({
    rulings: cardHistory(db, actor, num(id), oid, 20),
    memories: memoriesForCards(db, actor, [oid], num(id)).get(oid) ?? [],
  }));

  // ---------- slots and tags ----------
  route("POST", /^\/api\/decks\/(\d+)\/slots$/, (actor, [id], body) => {
    createSlot(db, actor, num(id), String(body.name ?? ""), body.target_min ?? null, body.target_max ?? null);
    return { state: state(actor, num(id)) };
  });
  route("PATCH", /^\/api\/decks\/(\d+)\/slots\/(\d+)$/, (actor, [id, sid], body) => {
    updateSlot(db, actor, num(id), num(sid), {
      name: body.name,
      targetMin: body.target_min,
      targetMax: body.target_max,
    });
    return { state: state(actor, num(id)) };
  });
  route("DELETE", /^\/api\/decks\/(\d+)\/slots\/(\d+)$/, (actor, [id, sid]) => {
    deleteSlot(db, actor, num(id), num(sid));
    return { state: state(actor, num(id)) };
  });
  route("POST", /^\/api\/decks\/(\d+)\/tags$/, (actor, [id], body) => {
    createTag(db, actor, num(id), String(body.name ?? ""));
    return { state: state(actor, num(id)) };
  });
  route("PATCH", /^\/api\/decks\/(\d+)\/tags\/(\d+)$/, (actor, [id, tid], body) => {
    renameTag(db, actor, num(id), num(tid), String(body.name ?? ""));
    return { state: state(actor, num(id)) };
  });
  route("DELETE", /^\/api\/decks\/(\d+)\/tags\/(\d+)$/, (actor, [id, tid]) => {
    deleteTag(db, actor, num(id), num(tid));
    return { state: state(actor, num(id)) };
  });

  // ---------- proposals and rulings (spec §8) ----------
  const inbox = (actor: Actor) => ({
    proposals: listProposals(db, actor, { status: "open" }).filter(p => requireDeck(db, actor, p.deck_id).category !== "legacy"),
    document_edits: listDocumentEdits(db, actor, { status: "pending" }).filter(e => e.deck_id == null || requireDeck(db, actor, e.deck_id).category !== "legacy"),
    unreviewed: unreviewedCount(db, actor),
  });
  route("GET", /^\/api\/inbox$/, inbox);
  route("GET", /^\/api\/proposal-history$/, (actor) => ({
    proposals: listProposals(db, actor), document_edits: listDocumentEdits(db, actor),
  }));
  const rule = (actor: Actor, ids: number[], body: any) =>
    body.outcome === "accept"
      ? acceptItems(db, actor, ids, String(body.reason ?? ""), { expected_revision: body.expected_revision, acknowledge_stale: body.acknowledge_stale })
      : body.outcome === "reject"
        ? rejectItems(db, actor, ids, String(body.reason ?? ""))
        : (() => {
            throw new ServiceError("A ruling's outcome is 'accept' or 'reject'");
          })();
  const assertItemsInDeck = (actor: Actor, deckId: number, ids: number[]) => {
    requireDeck(db, actor, deckId);
    const belongs = db.prepare("SELECT 1 FROM proposal_items i JOIN proposals p ON p.id = i.proposal_id WHERE i.id = ? AND p.deck_id = ?");
    if (ids.some(id => !belongs.get(id, deckId))) throw new ServiceError("Every proposal item must belong to this deck");
  };
  route("POST", /^\/api\/decks\/(\d+)\/items\/rule$/, (actor, [id], body) => {
    if (!Array.isArray(body.item_ids) || !body.item_ids.length || !body.item_ids.every((v: unknown) => typeof v === "number" && Number.isSafeInteger(v) && v > 0))
      throw new ServiceError("item_ids must be a non-empty array of integers");
    assertItemsInDeck(actor, num(id), body.item_ids);
    rule(actor, body.item_ids, body);
    return { state: state(actor, num(id)), inbox: inbox(actor) };
  });
  route("POST", /^\/api\/decks\/(\d+)\/proposals\/rule$/, (actor, [id], body) => {
    if (!Array.isArray(body.proposals) || !body.proposals.every((p: any) => p && typeof p === "object" && Number.isSafeInteger(p.proposal_id) && p.proposal_id > 0 && (p.acknowledge_stale === undefined || typeof p.acknowledge_stale === "boolean")))
      throw new ServiceError("proposals must be an array of proposal IDs and optional stale acknowledgements");
    acceptProposals(db, actor, num(id), body.proposals, num(body.expected_revision, "expected_revision"));
    return { state: state(actor, num(id)), inbox: inbox(actor) };
  });
  route("POST", /^\/api\/decks\/(\d+)\/proposals\/(\d+)\/rule$/, (actor, [id, pid], body) => {
    const ids = pendingItemIds(db, actor, num(pid));
    assertItemsInDeck(actor, num(id), ids);
    rule(actor, ids, body);
    return { state: state(actor, num(id)), inbox: inbox(actor) };
  });
  route("POST", /^\/api\/document-edits\/(\d+)\/rule$/, (actor, [editId], body) => {
    if (body.outcome !== "accept" && body.outcome !== "reject")
      throw new ServiceError("A ruling's outcome is 'accept' or 'reject'");
    const edit = getDocumentEdit(db, actor, num(editId), true);
    ruleDocumentEdit(db, actor, num(editId), body.outcome, String(body.reason ?? ""), { expected_revision: body.expected_revision, acknowledge_stale: body.acknowledge_stale, expected_documents: body.expected_documents });
    return {
      ...(edit?.deck_id != null ? { state: state(actor, edit.deck_id) } : { profile: getProfile(db, actor) }),
      inbox: inbox(actor),
    };
  });
  route("POST", /^\/api\/decks\/(\d+)\/log\/(\d+)\/undo$/, (actor, [id, logId]) => {
    if (!db.prepare("SELECT 1 FROM log WHERE id = ? AND deck_id = ?").get(num(logId), num(id)))
      throw new ServiceError("Log entry does not belong to this deck");
    const result = undo(db, actor, num(logId));
    return { state: state(actor, num(id)), skipped: result.skipped };
  });
  route("GET", /^\/api\/log$/, (actor, _p, _b, url) =>
    listLog(db, actor, {
      deckId: url.searchParams.get("deck") ? num(url.searchParams.get("deck")) : undefined,
      unreviewedOnly: url.searchParams.get("unreviewed") === "1",
      limit: num(url.searchParams.get("limit") ?? 100),
    }),
  );

  // ---------- deck power (spec §5.7) ----------
  route("GET", /^\/api\/decks\/(\d+)\/power$/, (actor, [id]) => getPower(db, actor, num(id)));
  route("PUT", /^\/api\/decks\/(\d+)\/power\/target$/, (actor, [id], body) => {
    setPowerTarget(db, actor, num(id), body.target);
    return { state: state(actor, num(id)) };
  });
  route("POST", /^\/api\/decks\/(\d+)\/power\/assessments$/, (actor, [id], body) => {
    const assessmentId = recordPowerAssessment(db, actor, num(id), body);
    return { assessment_id: assessmentId, state: state(actor, num(id)) };
  });

  // ---------- documents (spec §6) ----------
  route("PUT", /^\/api\/decks\/(\d+)\/brief$/, (actor, [id], body) => {
    updateBrief(db, actor, num(id), { thesis: body.thesis, constraints_md: body.constraints_md });
    return { state: state(actor, num(id)) };
  });
  route("POST", /^\/api\/decks\/(\d+)\/engines$/, (actor, [id], body) => {
    setEngine(db, actor, num(id), String(body.name ?? ""), String(body.description ?? ""), body.pieces ?? []);
    return { state: state(actor, num(id)) };
  });
  route("DELETE", /^\/api\/decks\/(\d+)\/engines\/(\d+)$/, (actor, [id, engineId]) => {
    removeEngine(db, actor, num(id), num(engineId));
    return { state: state(actor, num(id)) };
  });
  route("GET", /^\/api\/profile$/, (actor) => ({ profile: getProfile(db, actor) }));
  route("PUT", /^\/api\/profile$/, (actor, _p, body) => {
    setProfile(db, actor, String(body.body_md ?? ""));
    return { profile: getProfile(db, actor) };
  });

  // ---------- memories (spec §7.5) ----------
  route("GET", /^\/api\/memories$/, (actor, _p, _b, url) => {
    const q = url.searchParams;
    const global = loadScope(db, actor, "global");
    return {
      memories: listMemories(db, actor, {
        scope: (q.get("scope") as "global" | "deck" | null) ?? undefined,
        deckId: q.get("deck") ? num(q.get("deck")) : undefined,
        cardId: q.get("card") ?? undefined,
        query: q.get("q") ?? undefined,
        status: (q.get("status") as any) ?? "active",
        limit: 500,
      }),
      budgets: getBudgets(db, actor),
      global_budget: { budget: global.budget, used: global.used, omitted: global.omitted },
    };
  });
  route("POST", /^\/api\/memories$/, (actor, _p, body) => ({
    memory: remember(db, actor, {
      text: String(body.text ?? ""),
      scope: body.scope,
      deck_id: body.deck_id ?? null,
      cards: body.cards ?? [],
      basis: body.basis,
    }).memory,
  }));
  route("PATCH", /^\/api\/memories\/(\d+)$/, (actor, [id], body) => ({
    memory: reviseMemory(db, actor, num(id), body),
  }));
  route("GET", /^\/api\/memories\/(\d+)$/, (actor, [id]) => ({ memory: getMemory(db, actor, num(id)) }));

  // ---------- activity, stats, focus, settings ----------
  route("GET", /^\/api\/activity$/, (actor, _p, _b, url) =>
    listActivity(db, actor, num(url.searchParams.get("limit") ?? 200)),
  );
  route("GET", /^\/api\/stats$/, (actor) => playerStats(db, actor));
  route("GET", /^\/api\/focus$/, (actor) => ({ focus: getFocus(db, actor) }));
  const settings = (actor: Actor) => {
    const cards = db.prepare("SELECT COUNT(*) n FROM cards").get() as { n: number };
    const ingested = db.prepare("SELECT value FROM meta WHERE key = 'last_ingest'").get() as
      | { value: string }
      | undefined;
    return {
      jobs: opts.jobs?.status() ?? null,
      budgets: getBudgets(db, actor),
      cards: { count: cards.n, updated_at: ingested?.value ?? null, prices: priceStatus(db) },
      agent: {
        url: `${mode === "hosted" ? publicOrigin() : `http://127.0.0.1:${opts.port ?? 8787}`}${MCP_PATH}`,
        mode,
        tokens: listTokens(db, actor),
        stdio_command: `node ${join(PKG_ROOT, "src", "mcp", "stdio.ts")}`,
      },
    };
  };
  route("GET", /^\/api\/settings$/, settings);
  route("PUT", /^\/api\/settings$/, (actor, _p, body) => {
    if (body.budgets) setBudgets(db, actor, body.budgets);
    return settings(actor);
  });
  route("POST", /^\/api\/settings\/tokens$/, (actor, _p, body) => createToken(db, actor, String(body.label ?? "")));
  route("DELETE", /^\/api\/settings\/tokens\/(\d+)$/, (actor, [id]) => {
    revokeToken(db, actor, num(id, "token id", 1));
    return { ok: true };
  });
  route("POST", /^\/api\/cards\/refresh$/, (actor) => {
    requireAdmin(db, actor);
    if (!opts.jobs) throw new ServiceError("Background jobs are not configured", 503);
    opts.jobs.enqueue("refresh");
    return settings(actor);
  });

  // ---------- Archidekt interop (spec §12) ----------
  route("GET", /^\/api\/decks\/(\d+)\/export$/, (actor, [id], _b, url) =>
    exportDeck(db, actor, num(id), {
      categories: url.searchParams.get("categories") !== "off",
      oracleIds: url.searchParams.has("oracle_ids") ? url.searchParams.get("oracle_ids")!.split(",").filter(Boolean) : undefined,
      includeMaybe: url.searchParams.get("include_maybe") === "on",
    }),
  );
  route("POST", /^\/api\/decks\/(\d+)\/import\/preview$/, (actor, [id], body) =>
    diffImport(db, actor, num(id), String(body.text ?? "")),
  );
  route("POST", /^\/api\/decks\/(\d+)\/import$/, (actor, [id], body) => {
    const r = applyImport(db, actor, num(id), String(body.text ?? ""), body.note);
    return { import: r, state: state(actor, num(id)) };
  });

  // ---------- card search ----------
  route("GET", /^\/api\/resolve$/, (actor, _p, _b, url) => {
    const name = url.searchParams.get("name") ?? "";
    const matches = cardsNamed(db, name);
    return { matches, suggestions: matches.length ? [] : suggestNames(db, name) };
  });
  route("GET", /^\/api\/search$/, (actor, _p, _b, url) => {
    const q = url.searchParams.get("q") ?? "";
    const deckParam = url.searchParams.get("deck");
    try {
      return search(db, actor, q, {
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
  route("PUT", /^\/api\/focus$/, (actor, _p, body) => {
    setFocus(db, actor, body.deck_id ?? null, Array.isArray(body.selected) ? body.selected.map(String) : []);
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

      if (!allowedRequest(req, mode)) return json(403, { error: "This app only answers requests from this machine." });
      // Health contains shared facts only; every other API/event request needs a principal.
      const actor = url.pathname === "/api/health" ? localActor() :
        await (opts.resolveUiActor ?? resolveUiActor)(req, mode);
      if (url.pathname.startsWith("/api/") && !actor) return json(401, { error: "Sign in is required" });

      if (req.method === "GET" && url.pathname === "/api/events") {
        res.writeHead(200, {
          "Content-Type": "text/event-stream",
          "Cache-Control": "no-cache",
          Connection: "keep-alive",
          "X-Accel-Buffering": "no",
        });
        res.write(`data: ${JSON.stringify({ kind: "connected" })}\n\n`);
        const userId = actor!.user_id;
        const userListeners = listeners.get(userId) ?? new Set<ServerResponse>();
        userListeners.add(res);
        listeners.set(userId, userListeners);
        const heartbeat = setInterval(() => res.write(": heartbeat\n\n"), 25_000);
        heartbeat.unref();
        res.on("close", () => { clearInterval(heartbeat); userListeners.delete(res); if (!userListeners.size) listeners.delete(userId); });
        return;
      }

      for (const r of routes) {
        if (r.method !== req.method) continue;
        const m = url.pathname.match(r.pattern);
        if (!m) continue;
        const body = req.method === "GET" ? {} : await readJson(req);
        validateNumbers(body);
        const target = url.pathname.match(/^\/api\/decks\/(\d+)/);
        if (r.mutates && target) requireDeckOwner(db, actor!, num(target[1]));
        const result = await r.handler(actor!, m.slice(1), body, url);
        if (r.mutates && !quiet.has(url.pathname)) notify(actor!.user_id, { kind: url.pathname.split("/")[2] ?? "change", ...(url.pathname.match(/^\/api\/decks\/(\d+)/) ? { deck_id: Number(url.pathname.match(/^\/api\/decks\/(\d+)/)![1]) } : {}) });
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
    routes: routes.map(({ method, pattern, mutates }) => ({ method, pattern, mutates })),
    notify, notifyAll,
    close: async () => {
      await agents.close();
      await new Promise<void>((resolve) => {
        for (const responses of listeners.values()) for (const res of responses) res.end();
        server.close(() => resolve());
        server.closeAllConnections?.();
      });
    },
  };
}
