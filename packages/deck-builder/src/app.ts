// The app's one HTTP server (spec §10.1): the UI, its JSON API, the live
// event stream, and the agent endpoint, all in the process that owns the
// database. The UI API acts as the owner; the agent endpoint acts as the
// calling agent; both call the same capability layer.

import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { existsSync, readFileSync } from "node:fs";
import { extname, join, normalize } from "node:path";
import type { DatabaseSync } from "node:sqlite";
import { OWNER } from "./actor.ts";
import { listActivity } from "./activity.ts";
import { cardsNamed, requireCardById, suggestNames } from "./cards.ts";
import { PKG_ROOT, withTransaction } from "./db.ts";
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
  renameTag,
  setDeckCategory,
  setDeckParent,
  setDeckStatus,
  shortlistCard,
  unshortlistCard,
  updateSlot,
} from "./deck/service.ts";
import { getProfile, listDocumentEdits, ruleDocumentEdit, setProfile } from "./documents.ts";
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
import { refreshCardData } from "./refresh.ts";
import { SearchError, search } from "./search/index.ts";
import { deckState } from "./state.ts";
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

export function createApp(db: DatabaseSync, opts: AppOptions) {
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
  route("GET", /^\/api\/decks$/, () => listDecks(db));
  route("POST", /^\/api\/decks$/, (_p, body) => withTransaction(db, () => {
    const id = createDeck(db, String(body.name ?? ""), { status: body.status });
    if (body.category !== undefined) setDeckCategory(db, OWNER, id, body.category);
    return { state: state(id) };
  }));
  route("GET", /^\/api\/decks\/(\d+)$/, ([id]) => ({ state: state(Number(id)) }));
  route("PATCH", /^\/api\/decks\/(\d+)$/, ([id], body) => withTransaction(db, () => {
    if (body.name !== undefined) renameDeck(db, OWNER, Number(id), String(body.name));
    if (body.status !== undefined) setDeckStatus(db, OWNER, Number(id), body.status);
    if (body.category !== undefined) setDeckCategory(db, OWNER, Number(id), body.category);
    if (body.parent_id !== undefined)
      setDeckParent(db, OWNER, Number(id), body.parent_id == null ? null : Number(body.parent_id));
    return { state: state(Number(id)) };
  }));
  route("DELETE", /^\/api\/decks\/(\d+)$/, ([id]) => {
    deleteDeck(db, OWNER, Number(id));
    return { ok: true };
  });
  route("POST", /^\/api\/decks\/(\d+)\/fork$/, ([id], body) => ({
    state: state(forkDeck(db, OWNER, Number(id), String(body.name ?? ""))),
  }));
  route("GET", /^\/api\/decks\/(\d+)\/compare$/, ([id], _b, url) => {
    const other = url.searchParams.get("other");
    return compareDecks(db, Number(id), other ? Number(other) : undefined);
  });

  // ---------- cards in a deck ----------
  route("POST", /^\/api\/decks\/(\d+)\/cards$/, ([id], body) => {
    const deckId = Number(id);
    const oracleId = String(body.oracle_id);
    if (body.maybe) shortlistCard(db, deckId, oracleId, String(body.maybe_note ?? ""));
    else
      applyChanges(
        db,
        OWNER,
        deckId,
        [{ action: "add", oracle_id: oracleId, slot_id: body.slot_id ?? undefined, role: body.role, quantity: body.quantity }],
        { kind: "edit" },
      );
    return { state: state(deckId) };
  });
  route("PATCH", /^\/api\/decks\/(\d+)\/cards\/([0-9a-z-]+)$/, ([id, oid], body) => {
    const deckId = Number(id);
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
    if (body.quantity !== undefined) setQuantity(db, OWNER, deckId, oid, Number(body.quantity));
    if (body.slot_id !== undefined || body.tag_ids !== undefined)
      organizeCard(db, deckId, oid, { slotId: body.slot_id, tagIds: body.tag_ids });
    return { state: state(deckId) };
  });
  route("DELETE", /^\/api\/decks\/(\d+)\/cards\/([0-9a-z-]+)$/, ([id, oid]) => {
    const deckId = Number(id);
    const row = findDeckCard(db, deckId, oid);
    if (row?.maybe) unshortlistCard(db, deckId, oid);
    else applyChanges(db, OWNER, deckId, [{ action: "cut", oracle_id: oid, quantity: "all" }], { kind: "edit" });
    return { state: state(deckId) };
  });
  route("GET", /^\/api\/decks\/(\d+)\/cards\/([0-9a-z-]+)\/history$/, ([id, oid]) => ({
    rulings: cardHistory(db, Number(id), oid, 20),
    memories: memoriesForCards(db, [oid], Number(id)).get(oid) ?? [],
  }));

  // ---------- slots and tags ----------
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

  // ---------- proposals and rulings (spec §8) ----------
  const inbox = () => ({
    proposals: listProposals(db, { status: "open" }),
    document_edits: listDocumentEdits(db, { status: "pending" }),
    unreviewed: unreviewedCount(db),
  });
  route("GET", /^\/api\/inbox$/, inbox);
  const rule = (ids: number[], body: any) =>
    body.outcome === "accept"
      ? acceptItems(db, OWNER, ids, String(body.reason ?? ""))
      : body.outcome === "reject"
        ? rejectItems(db, OWNER, ids, String(body.reason ?? ""))
        : (() => {
            throw new ServiceError("A ruling's outcome is 'accept' or 'reject'");
          })();
  route("POST", /^\/api\/decks\/(\d+)\/items\/rule$/, ([id], body) => {
    rule((body.item_ids ?? []).map(Number), body);
    return { state: state(Number(id)), inbox: inbox() };
  });
  route("POST", /^\/api\/decks\/(\d+)\/proposals\/(\d+)\/rule$/, ([id, pid], body) => {
    rule(pendingItemIds(db, Number(pid)), body);
    return { state: state(Number(id)), inbox: inbox() };
  });
  route("POST", /^\/api\/document-edits\/(\d+)\/rule$/, ([editId], body) => {
    if (body.outcome !== "accept" && body.outcome !== "reject")
      throw new ServiceError("A ruling's outcome is 'accept' or 'reject'");
    const edit = listDocumentEdits(db, {}).find((e) => e.id === Number(editId));
    ruleDocumentEdit(db, OWNER, Number(editId), body.outcome, String(body.reason ?? ""));
    return {
      ...(edit?.deck_id != null ? { state: state(edit.deck_id) } : { profile: getProfile(db) }),
      inbox: inbox(),
    };
  });
  route("POST", /^\/api\/decks\/(\d+)\/log\/(\d+)\/undo$/, ([id, logId]) => {
    const result = undo(db, OWNER, Number(logId));
    return { state: state(Number(id)), skipped: result.skipped };
  });
  route("GET", /^\/api\/log$/, (_p, _b, url) =>
    listLog(db, {
      deckId: url.searchParams.get("deck") ? Number(url.searchParams.get("deck")) : undefined,
      unreviewedOnly: url.searchParams.get("unreviewed") === "1",
      limit: Number(url.searchParams.get("limit") ?? 100),
    }),
  );

  // ---------- deck power (spec §5.7) ----------
  route("GET", /^\/api\/decks\/(\d+)\/power$/, ([id]) => getPower(db, Number(id)));
  route("PUT", /^\/api\/decks\/(\d+)\/power\/target$/, ([id], body) => {
    setPowerTarget(db, OWNER, Number(id), body.target);
    return { state: state(Number(id)) };
  });
  route("POST", /^\/api\/decks\/(\d+)\/power\/assessments$/, ([id], body) => {
    const assessmentId = recordPowerAssessment(db, OWNER, Number(id), body);
    return { assessment_id: assessmentId, state: state(Number(id)) };
  });

  // ---------- documents (spec §6) ----------
  route("PUT", /^\/api\/decks\/(\d+)\/brief$/, ([id], body) => {
    updateBrief(db, Number(id), { thesis: body.thesis, constraints_md: body.constraints_md });
    return { state: state(Number(id)) };
  });
  route("POST", /^\/api\/decks\/(\d+)\/engines$/, ([id], body) => {
    setEngine(db, Number(id), String(body.name ?? ""), String(body.description ?? ""), body.pieces ?? []);
    return { state: state(Number(id)) };
  });
  route("DELETE", /^\/api\/decks\/(\d+)\/engines\/(\d+)$/, ([id, engineId]) => {
    removeEngine(db, Number(id), Number(engineId));
    return { state: state(Number(id)) };
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
        deckId: q.get("deck") ? Number(q.get("deck")) : undefined,
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
    memory: reviseMemory(db, OWNER, Number(id), body),
  }));
  route("GET", /^\/api\/memories\/(\d+)$/, ([id]) => ({ memory: getMemory(db, Number(id)) }));

  // ---------- activity, stats, focus, settings ----------
  route("GET", /^\/api\/activity$/, (_p, _b, url) =>
    listActivity(db, Number(url.searchParams.get("limit") ?? 200)),
  );
  route("GET", /^\/api\/stats$/, () => playerStats(db));
  route("GET", /^\/api\/focus$/, () => ({ focus: getFocus(db) }));
  const settings = () => {
    const cards = db.prepare("SELECT COUNT(*) n FROM cards").get() as { n: number };
    const ingested = db.prepare("SELECT value FROM meta WHERE key = 'last_ingest'").get() as
      | { value: string }
      | undefined;
    return {
      budgets: getBudgets(db),
      cards: { count: cards.n, updated_at: ingested?.value ?? null, prices: priceStatus(db) },
      agent: {
        url: `http://127.0.0.1:${opts.port ?? 8787}${MCP_PATH}`,
        token,
        // The stdio shim, by absolute path, so it runs from anywhere.
        stdio_command: `node ${join(PKG_ROOT, "src", "mcp", "stdio.ts")}`,
      },
    };
  };
  route("GET", /^\/api\/settings$/, settings);
  route("PUT", /^\/api\/settings$/, (_p, body) => {
    if (body.budgets) setBudgets(db, body.budgets);
    return settings();
  });
  route("POST", /^\/api\/settings\/rotate-token$/, () => {
    if (!opts.rotateToken) throw new ServiceError("This server cannot rotate its token");
    token = opts.rotateToken();
    agents = createAgentEndpoint(db, { token, version: APP_VERSION, onChange: notify });
    return settings();
  });
  route("POST", /^\/api\/cards\/refresh$/, async () => {
    await refreshCardData(db, { force: true });
    return settings();
  });

  // ---------- Archidekt interop (spec §12) ----------
  route("GET", /^\/api\/decks\/(\d+)\/export$/, ([id], _b, url) =>
    exportDeck(db, Number(id), {
      categories: url.searchParams.get("categories") !== "off",
    }),
  );
  route("POST", /^\/api\/decks\/(\d+)\/import\/preview$/, ([id], body) =>
    diffImport(db, Number(id), String(body.text ?? "")),
  );
  route("POST", /^\/api\/decks\/(\d+)\/import$/, ([id], body) => {
    const r = applyImport(db, OWNER, Number(id), String(body.text ?? ""), body.note);
    return { import: r, state: state(Number(id)) };
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
        limit: Math.min(Number(url.searchParams.get("limit") ?? 30), 100),
        deckId: deckParam ? Number(deckParam) : undefined,
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

  async function readJson(req: IncomingMessage): Promise<any> {
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(chunk as Buffer);
    if (!chunks.length) return {};
    try {
      return JSON.parse(Buffer.concat(chunks).toString("utf-8"));
    } catch {
      throw new ServiceError("Invalid JSON body");
    }
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
        });
        res.write(`data: ${version}\n\n`);
        listeners.add(res);
        req.on("close", () => listeners.delete(res));
        return;
      }

      for (const r of routes) {
        if (r.method !== req.method) continue;
        const m = url.pathname.match(r.pattern);
        if (!m) continue;
        const body = req.method === "GET" ? {} : await readJson(req);
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
    close: () =>
      new Promise<void>((resolve) => {
        for (const res of listeners) res.end();
        server.close(() => resolve());
        server.closeAllConnections?.();
      }),
  };
}
