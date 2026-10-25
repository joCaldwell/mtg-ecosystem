import { displayAuthor } from "./actor.ts";
// Memories (spec §7): short notes about preferences, reasons and outcomes.
// Agents write them without approval; the owner can always see, edit and
// retire them. They are rows, not files, so the app can attach one to a card
// or a deck and surface it without anyone going looking (§7.2).
//
// Nothing here calls a model. Search is SQLite full-text plus scope and card
// links; consolidation is the agent's work, prompted by the budget signal.

import type { Database } from "./sqlite.ts";
import { ServiceError } from "./errors.ts";
import { getSetting, setSetting, withTransaction } from "./db.ts";
import { type Actor, requireSelf, requireDeckOwner, PermissionError, authorOf } from "./actor.ts";
import { lintProse, requireCardById } from "./cards.ts";
import { requireDeck, requireEditableDeck } from "./deck/service.ts";

export type MemoryScope = "global" | "deck";
export type MemoryBasis = "stated" | "inferred";
export type MemoryStatus = "active" | "superseded" | "retired";

export interface MemoryView {
  user_id: number;
  author_user_id: number;
  author_via: "ui" | "agent";
  author_label: string;
  author_display?: string;
  id: number;
  text: string;
  scope: MemoryScope;
  deck_id: number | null;
  deck_name: string | null;
  read_only: boolean;
  basis: MemoryBasis;
  author: string;
  owner_touched: boolean;
  source_log_id: number | null;
  deck_revision: number | null;
  status: MemoryStatus;
  superseded_by: number | null;
  created_at: string;
  revised_at: string;
  cards: Array<{ oracle_id: string; name: string }>;
}

const SELECT = `SELECT m.user_id, m.author_user_id, m.author_via, m.author_label, m.id, m.text, m.scope, m.deck_id, d.name AS deck_name, (d.category = 'legacy') AS read_only, m.basis, m.author,
       m.owner_touched, m.source_log_id, m.deck_revision, m.status, m.superseded_by,
       m.created_at, m.revised_at
  FROM memories m LEFT JOIN decks d ON d.id = m.deck_id`;

type Row = Omit<MemoryView, "cards" | "owner_touched"> & { owner_touched: number };

function toViews(db: Database, actor: Actor, rows: Row[]): MemoryView[] {
  const cards = db.prepare(
    `SELECT mc.oracle_id, c.name FROM memory_cards mc JOIN cards c ON c.oracle_id = mc.oracle_id
     WHERE mc.memory_id = ? ORDER BY c.name`,
  );
  return rows.map((r) => ({
    ...r,
    author_display: displayAuthor(db, r),
    owner_touched: !!r.owner_touched,
    read_only: !!r.read_only,
    cards: cards.all(r.id) as unknown as Array<{ oracle_id: string; name: string }>,
  }));
}

export function getMemory(db: Database, actor: Actor, id: number): MemoryView {
  const row = db.prepare(`${SELECT} WHERE m.id = ? AND m.user_id = ?`).get(id, actor.user_id) as Row | undefined;
  if (!row) throw new ServiceError(`There is no memory ${id}`, 404);
  return toViews(db, actor, [row])[0];
}

// ---------- budgets (spec §7.4) ----------

export const DEFAULT_BUDGETS = { global: 1500, deck: 1000 };

export function getBudgets(db: Database, actor: Actor): { global: number; deck: number } {
  const read = (key: string, fallback: number) => {
    const n = Number(getSetting(db, actor, key));
    return Number.isFinite(n) && n > 0 ? n : fallback;
  };
  return {
    global: read("memory_budget_global", DEFAULT_BUDGETS.global),
    deck: read("memory_budget_deck", DEFAULT_BUDGETS.deck),
  };
}

export function setBudgets(db: Database, actor: Actor, budgets: { global?: number; deck?: number }): void {
  for (const [key, value] of Object.entries(budgets)) {
    if (value === undefined) continue;
    if (!Number.isFinite(value) || value < 100 || value > 20000)
      throw new ServiceError("A memory budget is between 100 and 20000 estimated tokens");
    setSetting(db, actor, `memory_budget_${key}`, String(Math.floor(value)));
  }
}

/** A rough size, good enough to budget with: about four characters a token. */
export function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

export interface LoadedScope {
  memories: MemoryView[];
  /** Active memories in this scope that did not fit the budget. */
  omitted: number;
  budget: number;
  used: number;
  /** True when something was left out: the cue to consolidate this scope. */
  over: boolean;
}

// What loads with a scope (spec §7.2, §7.4): the most recently revised active
// memories that fit the budget, and a count of what was left out.
export function loadScope(db: Database, actor: Actor, scope: MemoryScope, deckId?: number): LoadedScope {
  const all = listMemories(db, actor, { scope, deckId, limit: 1000 });
  const budget = getBudgets(db, actor)[scope];
  const memories: MemoryView[] = [];
  let used = 0;
  for (const m of all) {
    const cost = estimateTokens(m.text) + 8;
    if (used + cost > budget) continue;
    used += cost;
    memories.push(m);
  }
  const omitted = all.length - memories.length;
  return { memories, omitted, budget, used, over: omitted > 0 };
}

// ---------- reading ----------

export interface MemoryQuery {
  scope?: MemoryScope;
  deckId?: number;
  cardId?: string;
  /** Full-text query: any of the words, best matches first. */
  query?: string;
  status?: MemoryStatus | "any";
  limit?: number;
}

function ftsQuery(text: string, minLength = 2): string | null {
  const terms = [
    ...new Set(
      text
        .toLowerCase()
        .split(/[^\p{L}\p{N}]+/u)
        .filter((t) => t.length >= minLength && !STOPWORDS.has(t)),
    ),
  ];
  return terms.length ? terms.map((t) => `"${t}"*`).join(" OR ") : null;
}

const STOPWORDS = new Set([
  "the", "and", "for", "that", "this", "with", "not", "but", "are", "was", "have", "has",
  "they", "them", "their", "from", "into", "when", "than", "then", "its", "his", "her",
  "you", "your", "deck", "card", "cards", "owner", "likes", "like", "want", "wants",
]);

/** Most recently revised first; with a query, best match first. */
export function listMemories(db: Database, actor: Actor, q: MemoryQuery = {}): MemoryView[] {
  if (q.deckId !== undefined && db.prepare("SELECT 1 FROM decks WHERE id = ?").get(q.deckId)) requireDeck(db, actor, q.deckId);
  const where: string[] = ["m.user_id = ?"];
  const params: Array<string | number> = [actor.user_id];
  const status = q.status ?? "active";
  if (status !== "any") {
    where.push("m.status = ?");
    params.push(status);
  }
  if (q.scope) {
    where.push("m.scope = ?");
    params.push(q.scope);
  }
  if (q.deckId !== undefined) {
    where.push("m.deck_id = ?");
    params.push(q.deckId);
  }
  if (q.cardId) {
    where.push("m.id IN (SELECT memory_id FROM memory_cards WHERE oracle_id = ?)");
    params.push(q.cardId);
  }
  const limit = Math.min(Math.max(q.limit ?? 50, 1), 1000);

  if (q.query?.trim()) {
    const match = ftsQuery(q.query);
    if (!match) return [];
    const rows = db
      .prepare(
        `${SELECT} JOIN memories_fts ON memories_fts.memory_id = m.id
         WHERE memories_fts MATCH ? ${where.length ? `AND ${where.join(" AND ")}` : ""}
         ORDER BY bm25(memories_fts) LIMIT ?`,
      )
      .all(match, ...params, limit) as unknown as Row[];
    return toViews(db, actor, rows);
  }
  const rows = db
    .prepare(
      `${SELECT} ${where.length ? `WHERE ${where.join(" AND ")}` : ""}
       ORDER BY m.revised_at DESC, m.id DESC LIMIT ?`,
    )
    .all(...params, limit) as unknown as Row[];
  return toViews(db, actor, rows);
}

// Spec §7.2: memories linked to a card are attached wherever the card
// appears. A global one follows the card into every deck; a deck-scoped one
// surfaces only in its own deck.
export function memoriesForCards(
  db: Database, actor: Actor,
  oracleIds: string[],
  deckId?: number,
): Map<string, MemoryView[]> {
  if (deckId !== undefined) requireDeck(db, actor, deckId);
  const out = new Map<string, MemoryView[]>();
  const stmt = db.prepare(
    `${SELECT} JOIN memory_cards mc ON mc.memory_id = m.id
     WHERE mc.oracle_id = ? AND m.status = 'active' AND m.user_id = ?
       AND (m.scope = 'global' OR m.deck_id = ?)
     ORDER BY m.revised_at DESC LIMIT 5`,
  );
  for (const id of new Set(oracleIds)) {
    const rows = stmt.all(id, actor.user_id, deckId ?? -1) as unknown as Row[];
    if (rows.length) out.set(id, toViews(db, actor, rows));
  }
  return out;
}

// ---------- writing ----------

function indexText(db: Database, actor: Actor, id: number, text: string) {
  db.prepare("DELETE FROM memories_fts WHERE memory_id = ?").run(id);
  db.prepare("INSERT INTO memories_fts (text, memory_id) VALUES (?, ?)").run(text, id);
}

function setCards(db: Database, actor: Actor, id: number, oracleIds: string[]) {
  db.prepare("DELETE FROM memory_cards WHERE memory_id = ?").run(id);
  const ins = db.prepare("INSERT OR IGNORE INTO memory_cards (memory_id, oracle_id) VALUES (?, ?)");
  for (const oid of new Set(oracleIds)) ins.run(id, oid);
}

// Cards a memory is linked to: the ones named explicitly plus every
// [[Card Name]] in the text, so mentioning a card is enough to attach to it.
function linkedCards(db: Database, actor: Actor, text: string, explicit: string[] = []): string[] {
  for (const id of explicit) requireCardById(db, id);
  return [...new Set([...explicit, ...lintProse(db, text).map((c) => c.oracle_id)])];
}

const fold = (text: string) => text.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();

// Existing memories that resemble a new one: same words, or the same linked
// cards, within the scopes a reader of the new one would also see.
function findSimilar(
  db: Database, actor: Actor,
  text: string,
  scope: MemoryScope,
  deckId: number | null,
  cardIds: string[],
  excludeId?: number,
): MemoryView[] {
  const found = new Map<number, MemoryView>();
  const inReach = (m: MemoryView) =>
    m.id !== excludeId && (m.scope === "global" || (scope === "deck" && m.deck_id === deckId));
  const match = ftsQuery(text, 4);
  if (match)
    for (const m of toViews(
      db, actor,
      db
        .prepare(
          `${SELECT} JOIN memories_fts ON memories_fts.memory_id = m.id
           WHERE memories_fts MATCH ? AND m.status = 'active' AND m.user_id = ?
             AND (m.scope = 'global' OR (? = 'deck' AND m.deck_id = ?)) AND m.id != ?
           ORDER BY bm25(memories_fts) LIMIT 12`,
        )
        .all(match, actor.user_id, scope, deckId, excludeId ?? -1) as unknown as Row[],
    ))
      if (inReach(m) && found.size < 4) found.set(m.id, m);
  for (const [, linked] of memoriesForCards(db, actor, cardIds, deckId ?? undefined))
    for (const m of linked) if (inReach(m)) found.set(m.id, m);
  return [...found.values()].slice(0, 6);
}

export interface RememberInput {
  text: string;
  scope: MemoryScope;
  deck_id?: number | null;
  /** Cards to link, by oracle_id, in addition to any [[Card Name]] in the text. */
  cards?: string[];
  basis?: MemoryBasis;
  source_log_id?: number | null;
  /** The list tested; defaults to the current deck revision. */
  deck_revision?: number;
}

export interface RememberResult {
  memory: MemoryView;
  /** Existing memories that resemble this one — revise instead of piling up (spec §7.3). */
  similar: MemoryView[];
}

const MAX_MEMORY_CHARS = 1200;

function validateText(text: string): string {
  const trimmed = (text ?? "").trim();
  if (!trimmed) throw new ServiceError("A memory needs text");
  if (trimmed.length > MAX_MEMORY_CHARS)
    throw new ServiceError(
      `A memory is one statement and its reason, a few sentences at most ` +
        `(${trimmed.length} characters; the limit is ${MAX_MEMORY_CHARS}). Split it.`,
    );
  return trimmed;
}

/** Write a memory. No approval; always visible to the owner (spec §7.3, §7.5). */
export function remember(db: Database, actor: Actor, input: RememberInput): RememberResult {
  if (input.deck_id != null) requireEditableDeck(db, actor, input.deck_id);
  if (input.source_log_id != null && !db.prepare("SELECT 1 FROM log WHERE id = ? AND user_id = ?").get(input.source_log_id, actor.user_id))
    throw new PermissionError("A memory source must be one of your own rulings");
  const text = validateText(input.text);
  if (input.scope !== "global" && input.scope !== "deck")
    throw new ServiceError("A memory's scope is 'global' or 'deck'");
  const deckId = input.scope === "deck" ? (input.deck_id ?? null) : null;
  if (input.scope === "deck" && deckId == null)
    throw new ServiceError("A deck-scoped memory needs the deck it belongs to");
  const deck = deckId == null ? null : requireEditableDeck(db, actor, deckId);
  if (input.deck_revision !== undefined && (!deck || !Number.isInteger(input.deck_revision) || input.deck_revision < 0 || input.deck_revision > deck.revision))
    throw new ServiceError("The tested revision must be a revision of this deck");
  const cardIds = linkedCards(db, actor, text, input.cards);
  // What an agent concludes is a hypothesis until the owner says so.
  const basis: MemoryBasis = input.basis ?? (actor.via === "ui" ? "stated" : "inferred");
  if (basis !== "stated" && basis !== "inferred")
    throw new ServiceError("A memory's basis is 'stated' or 'inferred'");

  const similar = findSimilar(db, actor, text, input.scope, deckId, cardIds);
  const duplicate = similar.find(
    (m) => fold(m.text) === fold(text) && m.scope === input.scope && m.deck_id === deckId,
  );
  if (duplicate)
    throw new ServiceError(
      `Memory #${duplicate.id} already says exactly this. Nothing was written.`,
      409,
    );

  return withTransaction(db, () => {
    const r = db
      .prepare(
        `INSERT INTO memories (text, scope, deck_id, basis, author, owner_touched, source_log_id, deck_revision, user_id, author_user_id, author_via, author_label)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        text,
        input.scope,
        deckId,
        basis,
        authorOf(actor),
        actor.via === "ui" ? 1 : 0,
        input.source_log_id ?? null,
        input.deck_revision ?? deck?.revision ?? null,
        actor.user_id, actor.user_id, actor.via, actor.label,
      );
    const id = Number(r.lastInsertRowid);
    setCards(db, actor, id, cardIds);
    indexText(db, actor, id, text);
    return { memory: getMemory(db, actor, id), similar };
  });
}

// Spec §7.3: the owner outranks the agent. Once the owner has written or
// edited a memory, an agent can no longer change it.
function assertMayChange(actor: Actor, memory: MemoryView) {
  if (actor.via === "agent" && memory.owner_touched)
    throw new PermissionError(
      `Memory #${memory.id} was written or edited by the owner, so it cannot be changed by an agent. ` +
        `If you believe it is out of date, write a new memory that says so and tell the owner.`,
    );
}

export type Revision =
  | { action: "update"; text?: string; cards?: string[]; basis?: MemoryBasis }
  | {
      action: "supersede";
      text: string;
      cards?: string[];
      basis?: MemoryBasis;
      /** Move the replacement to another scope, e.g. promoting a deck memory to global. */
      scope?: MemoryScope;
      deck_id?: number | null;
    }
  | { action: "retire" }
  | { action: "restore" }
  | { action: "confirm" };

/** Revise a memory. Nothing is deleted: superseding and retiring both keep the row. */
export function reviseMemory(
  db: Database,
  actor: Actor,
  id: number,
  revision: Revision,
): MemoryView {
  const tenant = db.prepare("SELECT user_id FROM memories WHERE id = ?").get(id) as { user_id: number } | undefined;
  if (tenant) requireSelf(actor, tenant.user_id);
  const memory = getMemory(db, actor, id);
  if (memory.deck_id != null) requireEditableDeck(db, actor, memory.deck_id);
  const touched = actor.via === "ui" ? 1 : memory.owner_touched ? 1 : 0;

  switch (revision.action) {
    case "update": {
      assertMayChange(actor, memory);
      if (memory.status !== "active")
        throw new ServiceError(`Memory #${id} is ${memory.status}; restore it before editing`);
      const text = revision.text !== undefined ? validateText(revision.text) : memory.text;
      const cards = linkedCards(
        db, actor,
        text,
        revision.cards ?? memory.cards.map((c) => c.oracle_id),
      );
      return withTransaction(db, () => {
        db.prepare(
          `UPDATE memories SET text = ?, basis = ?, owner_touched = ?, revised_at = datetime('now')
           WHERE id = ?`,
        ).run(text, revision.basis ?? memory.basis, touched, id);
        setCards(db, actor, id, cards);
        indexText(db, actor, id, text);
        return getMemory(db, actor, id);
      });
    }
    case "supersede": {
      assertMayChange(actor, memory);
      if (memory.status !== "active")
        throw new ServiceError(`Memory #${id} is already ${memory.status}`);
      const scope = revision.scope ?? memory.scope;
      return withTransaction(db, () => {
        // Out of the way first, so the replacement is not refused as its duplicate.
        db.prepare("UPDATE memories SET status = 'superseded' WHERE id = ?").run(id);
        const { memory: replacement } = remember(db, actor, {
          text: revision.text,
          scope,
          deck_id: scope === "deck" ? (revision.deck_id ?? memory.deck_id) : null,
          cards: revision.cards ?? memory.cards.map((c) => c.oracle_id),
          basis: revision.basis ?? memory.basis,
          source_log_id: memory.source_log_id,
        });
        db.prepare("UPDATE memories SET superseded_by = ? WHERE id = ?").run(replacement.id, id);
        return replacement;
      });
    }
    case "retire":
      assertMayChange(actor, memory);
      db.prepare(
        "UPDATE memories SET status = 'retired', owner_touched = ?, revised_at = datetime('now') WHERE id = ?",
      ).run(touched, id);
      return getMemory(db, actor, id);
    case "restore":
      assertMayChange(actor, memory);
      db.prepare(
        `UPDATE memories SET status = 'active', superseded_by = NULL, owner_touched = ?,
           revised_at = datetime('now') WHERE id = ?`,
      ).run(touched, id);
      return getMemory(db, actor, id);
    case "confirm":
      assertMayChange(actor, memory);
      // The owner said so, in the app or in conversation: a hypothesis
      // becomes a stated fact. Confirming does not hand the memory to the
      // owner; only their own edit does that.
      db.prepare(
        "UPDATE memories SET basis = 'stated', revised_at = datetime('now') WHERE id = ?",
      ).run(id);
      return getMemory(db, actor, id);
    default:
      throw new ServiceError(
        "A revision's action is one of: update, supersede, retire, restore, confirm",
      );
  }
}

// A fork starts with its parent's deck memories (spec §5.3). They are copies:
// the two decks diverge, and so does what is known about each.
export function copyDeckMemories(db: Database, actor: Actor, fromDeckId: number, toDeckId: number): number {
  requireEditableDeck(db, actor, toDeckId);
  requireDeckOwner(db, actor, fromDeckId);
  const source = listMemories(db, actor, { scope: "deck", deckId: fromDeckId, limit: 1000 });
  const revision = requireEditableDeck(db, actor, toDeckId).revision;
  const ins = db.prepare(
    `INSERT INTO memories (text, scope, deck_id, basis, author, owner_touched, source_log_id,
                           deck_revision, created_at, revised_at, user_id, author_user_id, author_via, author_label)
     VALUES (?, 'deck', ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?)`,
  );
  // Oldest first, so the copies keep the originals' relative order.
  for (const m of source.reverse()) {
    const r = ins.run(
      m.text,
      toDeckId,
      m.basis,
      m.author,
      m.owner_touched ? 1 : 0,
      revision,
      m.created_at,
      m.revised_at, actor.user_id, m.author_user_id, m.author_via, m.author_label,
    );
    const id = Number(r.lastInsertRowid);
    setCards(db, actor, id, m.cards.map((c) => c.oracle_id));
    indexText(db, actor, id, m.text);
  }
  return source.length;
}
