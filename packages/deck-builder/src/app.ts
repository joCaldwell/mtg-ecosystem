// The app's one HTTP server (spec §10.1): the UI, its JSON API, the live
// event stream, and the agent endpoint, all in the process that owns the
// database. Every request resolves a principal and both doors call the same
// actor-scoped capability layer.

import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { existsSync, readFileSync, statSync } from "node:fs";
import { extname, join, resolve, sep } from "node:path";
import type { Database } from "./sqlite.ts";
import { type Actor, requireAdmin, requireDeckOwner } from "./actor.ts";
import { readRequestBody } from "./http-body.ts";
import { databaseGate } from "./auth-database.ts";
import { consentHtml, consentScript } from "./consent.ts";
import { createHostedAuth, type HostedAuth } from "./auth.ts";
import { accountInfo, chooseHandle, createInvite, adminInfo, exportAccount, deleteAccount, connectedClients, disconnectClient } from "./accounts.ts";
import { createRateLimiter, RateLimitError } from "./ratelimit.ts";
import { clientIp, securityHeaders } from "./mode.ts";
import { allowedRequest, getMode, localActor, publicOrigin, type Mode } from "./mode.ts";
import { createToken, listTokens, revokeToken } from "./tokens.ts";
import { listActivity } from "./activity.ts";
import { cardsNamed, requireCardById, suggestNames } from "./cards.ts";
import { PKG_ROOT, SCHEMA_VERSION, withTransaction } from "./db.ts";
import { AppError, ServiceError } from "./errors.ts";
import { getFocus, setFocus } from "./focus.ts";
import { removeEngine, setEngine, updateBrief } from "./deck/brief.ts";
import { compareDecks, forkDeck } from "./deck/fork.ts";
import { applyImport, diffImport, exportDeck, exportPublicDeck } from "./deck/interop.ts";
import { cardHistory, hideLogEntry, listLog, unreviewedCount } from "./deck/log.ts";
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
import { priceStatus, priceResolver, getPriceSource, setPriceSource } from "./prices.ts";
import type { Jobs } from "./jobs.ts";
import { SearchError, search } from "./search/index.ts";
import { deckState, publicDeckState } from "./state.ts";
import { publicProfile, setPublishDecisions, setVisibility } from "./sharing.ts";
import { linkPreview, withPreview } from "./preview.ts";
import { getPlaytest, startPlaytest, changePlaytest, savePlaytestUi, saveObservation, tableCard } from "./playtest/service.ts";
import { playerStats } from "./stats.ts";
import { logging } from "./logging.ts";

export const APP_VERSION = "2.0.0";
const WEB_DIST = join(PKG_ROOT, "web", "dist");

export interface AppOptions {
  mode?: Mode;
  auth?: HostedAuth;
  /** Optional request-to-Actor override; production uses the hosted session resolver. */
  resolveUiActor?: (req: IncomingMessage, mode: Mode) => Actor | null | Promise<Actor | null>;
  /** The port the server listens on, for the connection details shown in Settings. */
  port?: number;
  jobs?: Jobs;
  /** Testable static root; production uses the Vite output. */
  webDist?: string;
}

type Handler = (actor: Actor, params: string[], body: any, url: URL) => unknown | Promise<unknown>;
/** A route anyone may call, signed in or not (spec §18.6); the capability takes the nullable viewer. */
type PublicHandler = (viewer: Actor | null, params: string[], body: any, url: URL) => unknown | Promise<unknown>;

/** The session seam preserves implicit user 1 in the local product. */
export function resolveUiActor(req: IncomingMessage, mode: Mode, auth?: HostedAuth): Actor | null | Promise<Actor | null> {
  return mode === "local" ? localActor() : auth?.resolveUiActor(req) ?? null;
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
  const auth = mode === "hosted" ? opts.auth ?? (process.env.BETTER_AUTH_SECRET ? createHostedAuth(db) : undefined) : undefined;
  const rateLimit = createRateLimiter();
  const resolveActor = opts.resolveUiActor ?? ((req:IncomingMessage, requestMode:Mode) => resolveUiActor(req,requestMode,auth));
  const routes: Array<{ method: string; pattern: RegExp; handler: PublicHandler; mutates: boolean; anonymous: boolean }> = [];
  const route = (method: string, pattern: RegExp, handler: Handler) =>
    routes.push({ method, pattern, handler: handler as PublicHandler, mutates: method !== "GET" && !pattern.source.includes("preview"), anonymous: false });
  const publicRoute = (pattern: RegExp, handler: PublicHandler) =>
    routes.push({ method: "GET", pattern, handler, mutates: false, anonymous: true });

  // ---------- live updates (spec §11) ----------
  // The stream says only "something changed"; the page refetches what it is
  // showing. A proposal made in a terminal appears without a reload.
  const listeners = new Map<number, Set<ServerResponse>>();
  const notify = (userId: number, change: Change) => {
    for (const res of listeners.get(userId) ?? []) res.write(`data: ${JSON.stringify(change)}\n\n`);
  };
  const notifyAll = (change: Change) => { for (const id of listeners.keys()) notify(id, change); };
  const agents = createAgentEndpoint(db, { mode, version: APP_VERSION, onChange: notify, verifyOAuth: auth?.verifyAccessToken, rateLimit, withDatabase: work => withDatabase(work) });

  // Envelope rule: a response carrying a domain composite names it — `state`,
  // `profile`, `memories` — and never spreads it at top level, so one
  // client-side applier can route every response.
  const state = (actor: Actor, id: number) => deckState(db, actor, id);

  if (mode === "hosted") {
    route("DELETE", /^\/api\/account\/oauth$/, (actor,_p,body) => disconnectClient(db,actor,String(body.client_id ?? "")));
    route("GET", /^\/api\/account$/, actor => accountInfo(db, actor));
    route("PATCH", /^\/api\/account\/handle$/, (actor,_p,body) => chooseHandle(db,actor,String(body.handle ?? "")));
    route("GET", /^\/api\/account\/export$/, actor => exportAccount(db,actor));
    route("DELETE", /^\/api\/account$/, (actor,_p,body) => {
      deleteAccount(db,actor,String(body.handle ?? ""));
      for (const stream of listeners.get(actor.user_id) ?? []) stream.end();
      listeners.delete(actor.user_id);
      return { ok: true };
    });
    route("GET", /^\/api\/admin$/, actor => ({ ...adminInfo(db,actor), jobs: opts.jobs?.status() ?? null }));
    route("POST", /^\/api\/admin\/invites$/, actor => createInvite(db,actor));
    route("POST", /^\/api\/admin\/jobs$/, (actor,_p,body) => {
      adminInfo(db,actor);
      if (!["backup","refresh"].includes(body.job)) throw new ServiceError("Unknown job");
      if (!opts.jobs) throw new ServiceError("Background jobs are not configured",503);
      opts.jobs.enqueue(body.job); return { ok:true };
    });
  }

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
    const other = url.searchParams.get("other"), slug = url.searchParams.get("slug");
    return compareDecks(db, actor, num(id), slug ? { slug } : other ? num(other) : undefined);
  });

  // ---------- sharing (spec §18) ----------
  route("PATCH", /^\/api\/decks\/(\d+)\/sharing$/, (actor, [id], body) => withTransaction(db, () => {
    if (body.visibility !== undefined) setVisibility(db, actor, num(id), body.visibility);
    if (body.publish_decisions !== undefined) setPublishDecisions(db, actor, num(id), body.publish_decisions);
    return { state: state(actor, num(id)) };
  }));
  route("PATCH", /^\/api\/decks\/(\d+)\/log\/(\d+)$/, (actor, [id, logId], body) => {
    // The deck first, and one answer for every miss, so log ids cannot be mapped to other users' decks.
    requireDeckOwner(db, actor, num(id), "change what this deck publishes");
    if (!db.prepare("SELECT 1 FROM log WHERE id = ? AND deck_id = ?").get(num(logId), num(id)))
      throw new ServiceError(`There is no deck ${num(id)}. List the decks to see which ids exist.`, 404);
    hideLogEntry(db, actor, num(logId), body.hidden);
    return { state: state(actor, num(id)) };
  });
  publicRoute(/^\/api\/public\/decks\/([0-9A-Za-z]+)$/, (viewer, [slug]) => ({ deck: publicDeckState(db, viewer, slug) }));
  publicRoute(/^\/api\/public\/decks\/([0-9A-Za-z]+)\/export$/, (viewer, [slug], _b, url) =>
    exportPublicDeck(db, viewer, slug, { categories: url.searchParams.get("categories") !== "off", includeMaybe: url.searchParams.get("include_maybe") === "on" }));
  publicRoute(/^\/api\/public\/users\/([a-z0-9_-]+)$/i, (viewer, [handle]) => ({ profile: publicProfile(db, viewer, handle) }));
  route("POST", /^\/api\/public\/decks\/([0-9A-Za-z]+)\/fork$/, (actor, [slug], body) => ({
    state: state(actor, forkDeck(db, actor, { slug }, String(body.name ?? ""))),
  }));

  publicRoute(/^\/api\/cards\/([0-9a-z-]+)\/table$/, (viewer, [id]) => ({ card: tableCard(db, viewer, id) }));
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
    if (ids.some(id => !belongs.get(id, deckId))) throw new ServiceError("Every proposal item must belong to this deck", 404);
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
    acceptProposals(db, actor, num(id), body.proposals, num(body.expected_revision, "expected_revision"), String(body.reason ?? ""));
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
      throw new ServiceError("Log entry does not belong to this deck", 404);
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
      price_source: getPriceSource(db, actor),
      cards: { count: cards.n, updated_at: ingested?.value ?? null, prices: priceStatus(db, priceResolver(db, actor).store) },
      agent: {
        url: `${mode === "hosted" ? publicOrigin() : `http://127.0.0.1:${opts.port ?? 8787}`}${MCP_PATH}`,
        mode,
        tokens: listTokens(db, actor),
        oauth_clients: mode === "hosted" ? connectedClients(db,actor) : [],
        oauth_usage: mode === "hosted" ? db.prepare("SELECT client_id,label,last_used_at FROM oauth_usage WHERE user_id=? ORDER BY last_used_at DESC").all(actor.user_id) : [],
        ...(mode === "local" ? { stdio_command: `node ${join(PKG_ROOT, "src", "mcp", "stdio.ts")}` } : {}),
      },
    };
  };
  route("GET", /^\/api\/settings$/, settings);
  route("PUT", /^\/api\/settings$/, (actor, _p, body) => withTransaction(db, () => {
    if (body.price_source) setPriceSource(db, actor, body.price_source);
    if (body.budgets) setBudgets(db, actor, body.budgets);
    return settings(actor);
  }));
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
  publicRoute(/^\/api\/resolve$/, (_viewer, _p, _b, url) => {
    const name = url.searchParams.get("name") ?? "";
    const matches = priceResolver(db, actor).resolve(cardsNamed(db, name));
    return { matches, suggestions: matches.length ? [] : suggestNames(db, name) };
  });
  publicRoute(/^\/api\/search$/, (actor, _p, _b, url) => {
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

  function serveStatic(pathname: string, res: ServerResponse, viewer: Actor | null, req: IncomingMessage): boolean {
    const root = resolve(opts.webDist ?? WEB_DIST);
    if (mode === "local" && ["/privacy","/terms"].includes(pathname)) return false;
    if (pathname.startsWith("/mcp") || /^\/(api|\.well-known)(\/|$)/.test(pathname)) return false;
    const file = resolve(root, pathname === "/" ? "index.html" : pathname.slice(1));
    if (!file.startsWith(root + sep)) return false;
    const appRoute = /^\/(?:decks|inbox|memories|profile|activity|settings|design|signin|consent|privacy|terms|deck\/[^/]+(?:\/(?:review|playtest))?|[du]\/[^/]+)?$/.test(pathname);
    const target = existsSync(file) && statSync(file).isFile() ? file :
      appRoute && !extname(pathname) ? join(root, "index.html") : null;
    if (!target || !existsSync(target)) return false;
    try {
      // Shared pages carry their own link-preview tags (§18.5), and say 404
      // when this requester cannot read what they name.
      const preview = target.endsWith("index.html")
        ? linkPreview(db, viewer, pathname, mode === "hosted" ? publicOrigin() : `http://${req.headers.host ?? "127.0.0.1"}`)
        : null;
      if (preview) {
        res.writeHead(preview.status, { "Content-Type": "text/html; charset=utf-8", "Cache-Control": mode === "hosted" ? "no-store" : "no-cache" });
        res.end(withPreview(readFileSync(target, "utf8"), preview));
        return true;
      }
      const content = readFileSync(target);
      res.writeHead(200, {
        "Content-Type": MIME[extname(target)] ?? "application/octet-stream",
        "Cache-Control": pathname.startsWith("/assets/") ? "public, max-age=31536000, immutable" : mode === "hosted" ? "no-store" : "no-cache",
      });
      res.end(content);
      return true;
    } catch { return false; }
  }

  async function readJson(req: IncomingMessage): Promise<Record<string, any>> {
    const body=await readRequestBody(req);
    if(!body.length)return {};
    if(req.headers["content-type"]?.split(";")[0].trim().toLowerCase()!=="application/json") throw new ServiceError("Content-Type must be application/json",415);
    try {
      const value:unknown=JSON.parse(body.toString("utf8"));
      if(!value || typeof value!=="object" || Array.isArray(value))throw Error();
      return value as Record<string,any>;
    }catch{throw new ServiceError("JSON body must be a valid object");}
  }

  const gate=databaseGate(db);
  const withDatabase = <T>(work:()=>T|Promise<T>):Promise<T> => mode === "hosted" ? gate.run(work) : Promise.resolve(work());
  const server: Server = createServer((req, res) => logging.request(req, res, async () => {
    securityHeaders(res,mode);
    if(mode === "hosted")res.setHeader("Cache-Control","no-store");
    const url = new URL(req.url ?? "/", "http://127.0.0.1");
    const json = (status: number, data: unknown) => {
      res.writeHead(status, { "Content-Type": "application/json" });
      res.end(JSON.stringify(data));
    };

    try {
      // Railway health probes use their own Host. This public response has no auth or tenant data.
      if(mode === "hosted" && req.method === "GET" && url.pathname === "/api/health")return json(200,{ok:true,schema_version:SCHEMA_VERSION});
      if(mode === "hosted") {
        if(!allowedRequest(req,mode))return json(403,{error:"Host or Origin is not allowed"});
        await readRequestBody(req);
      }
      await auth?.ready;
      if (url.pathname === MCP_PATH || url.pathname.startsWith(`${MCP_PATH}/`))
        return await agents.handle(req, res);

      if (!allowedRequest(req, mode)) return json(403, { error: "This app only answers requests from this machine." });
      if (mode === "hosted" && /^\/(api\/(auth|signup)|\.well-known)(\/|$)/.test(url.pathname)) {
        if(url.pathname === "/api/auth/oauth2/token") {
          const raw=(await readRequestBody(req)).toString("utf8");
          let id="";
          try {id=req.headers["content-type"]?.startsWith("application/json")?String(JSON.parse(raw).client_id ?? ""):new URLSearchParams(raw).get("client_id") ?? "";}catch{}
          if(!id && req.headers.authorization?.startsWith("Basic "))try{id=decodeURIComponent(Buffer.from(req.headers.authorization.slice(6),"base64").toString("utf8").split(":")[0]);}catch{}
          const exists=await withDatabase(()=>db.prepare('SELECT 1 FROM oauthClient WHERE clientId=? AND (disabled IS NULL OR disabled=0)').get(id));
          rateLimit(exists?"token":"auth",exists?id:clientIp(req));
        } else rateLimit("auth",clientIp(req));
      }
      if (mode === "hosted" && req.method === "POST" && url.pathname === "/api/signup") {
        if (!auth) throw new ServiceError("Authentication is not configured",503);
        if (req.headers.origin !== publicOrigin()) throw new ServiceError("Origin is required",403);
        const body = await readJson(req); await withDatabase(()=>{
          const code=String(body.code ?? "");
          if(auth.claimsEnabled && code && !db.prepare("SELECT 1 FROM invites WHERE code=? AND used_by IS NULL").get(code))rateLimit("claim",clientIp(req));
          auth.signup(req,res,code);
        }); return json(200,{ok:true});
      }
      if (mode === "hosted" && auth && (url.pathname.startsWith("/api/auth/") || url.pathname.startsWith("/.well-known/"))) {
        if (url.pathname === "/.well-known/oauth-authorization-server") req.url = "/api/auth/.well-known/oauth-authorization-server";
        // toNodeHandler uses the fixed configured issuer; forwarded headers
        // never get a chance to select a different auth origin.
        return await auth.handle(req,res);
      }
      return await withDatabase(async () => {
      if (req.method === "GET" && url.pathname === "/api/session") {
        const sessionActor = await resolveActor(req,mode);
        logging.actor(sessionActor);
        const providers = [process.env.DISCORD_CLIENT_ID && process.env.DISCORD_CLIENT_SECRET ? "discord" : null, process.env.GOOGLE_CLIENT_ID && process.env.GOOGLE_CLIENT_SECRET ? "google" : null].filter(Boolean);
        if (mode === "hosted" && !sessionActor) rateLimit("unauthenticated",clientIp(req));
        return json(200,{mode,providers,legal:auth?.legal ?? null,invites_required:auth?.invitesRequired ?? true,user:sessionActor ? accountInfo(db,sessionActor) : null});
      }
      if (mode === "hosted" && req.method === "GET" && url.pathname === "/consent.js") {
        res.writeHead(200,{"Content-Type":"text/javascript"}); return void res.end(consentScript);
      }
      // Health contains shared facts only; every other API/event request needs a principal.
      const actor = url.pathname === "/api/health" ? localActor() :
        await resolveActor(req,mode);
      logging.actor(actor);
      if (mode === "hosted") {
        if (!actor || url.pathname === "/api/health") {
          rateLimit("unauthenticated",clientIp(req));
          // Anonymous search (§18.6) is as expensive as anyone's, so it has its own, lower budget per IP (/64 for IPv6).
          if (url.pathname === "/api/search") rateLimit("anonymous_search",clientIp(req));
        } else {
          rateLimit("ui",String(actor.user_id));
          if (url.pathname === "/api/search") rateLimit("search",String(actor.user_id));
          if (url.pathname.endsWith("/compare")) rateLimit("compare",String(actor.user_id));
        }
        if (actor && url.pathname.startsWith("/api/") && !["GET","HEAD","OPTIONS"].includes(req.method ?? "") && req.headers.origin !== publicOrigin()) return json(403,{error:"Origin is required"});
      }
      // HEAD reads a public route or page like GET (link unfurlers probe with it); Node sends no body.
      const method = req.method === "HEAD" ? "GET" : req.method;
      const anonymous = routes.some(r => r.anonymous && r.method === method && r.pattern.test(url.pathname));
      if (url.pathname.startsWith("/api/") && !actor && !anonymous) return json(401, { error: "Sign in is required" });

      if (mode === "hosted" && req.method === "GET" && url.pathname === "/consent") {
        if (!actor) { res.writeHead(302,{Location:`/signin${url.search}`});return void res.end(); }
        if (!auth) return json(503,{error:"Authentication is not configured"});
        const client = await auth.consentIdentity(url.search.slice(1));
        const index = join(opts.webDist ?? WEB_DIST,"index.html");
        const css = existsSync(index) ? [...readFileSync(index,"utf8").matchAll(/href="([^" ]+\.css)"/g)].map(m=>m[1]) : [];
        res.writeHead(200,{"Content-Type":"text/html; charset=utf-8"});return void res.end(consentHtml(client,css));
      }

      if (req.method === "GET" && url.pathname === "/api/events") {
        res.writeHead(200, {
          "Content-Type": "text/event-stream",
          "Cache-Control": mode === "hosted" ? "no-store" : "no-cache",
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
        if (r.method !== req.method && !(req.method === "HEAD" && r.method === "GET")) continue;
        const m = url.pathname.match(r.pattern);
        if (!m) continue;
        const body = req.method === "GET" ? {} : await readJson(req);
        validateNumbers(body);
        const result = await r.handler(r.anonymous ? actor : actor!, m.slice(1), body, url);
        if (r.mutates && url.pathname !== "/api/account" && !quiet.has(url.pathname)) notify(actor!.user_id, { kind: url.pathname.split("/")[2] ?? "change", ...(url.pathname.match(/^\/api\/decks\/(\d+)/) ? { deck_id: Number(url.pathname.match(/^\/api\/decks\/(\d+)/)![1]) } : {}) });
        return json(200, result);
      }
      if ((req.method === "GET" || req.method === "HEAD") && !url.pathname.startsWith("/api/") && serveStatic(url.pathname, res, actor, req))
        return;
      json(404, { error: `Not found: ${req.method} ${url.pathname}` });
      });
    } catch (e: any) {
      logging.error(e);
      if (res.headersSent) return void res.end();
      if (e instanceof RateLimitError) res.setHeader("Retry-After",String(e.retryAfter));
      if (e instanceof AppError) return json(e.status, { error: e.message });
      json(500, { error: "Internal error" });
    }
  }));

  return {
    server,
    routes: routes.map(({ method, pattern, mutates }) => ({ method, pattern, mutates })),
    notify, notifyAll, withDatabase,
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
