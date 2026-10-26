import { displayAuthor, requireActor } from "../actor.ts";
import { requireDeck } from "./service.ts";
// The decisions log (spec §8.2): the deck's version history and the record of
// what the owner decided and why. Every act lands here through writeLog —
// decisions on proposals and document edits, direct edits, imports, undos — and
// the cards an act touched are queryable per card (§8.3).

import type { Database } from "../sqlite.ts";
import { ServiceError } from "../errors.ts";
import { type Actor, authorOf, requireSelf, isDeckOwner, requireDeckOwner, requireUi } from "../actor.ts";

export type LogKind = "accept" | "reject" | "edit" | "import" | "undo";

/** The longest reason a decision keeps (spec §8.2): a sentence or two, read by agents. */
export const MAX_RULING_REASON = 500;
/** A decision's optional reason, trimmed; blank is no reason. */
export function decisionReason(reason: unknown): string {
  const text = typeof reason === "string" ? reason.trim() : "";
  if (text.length > MAX_RULING_REASON)
    throw new ServiceError(`A reason can be at most ${MAX_RULING_REASON} characters; this one is ${text.length}.`);
  return text;
}

export interface LogCardInput {
  oracle_id: string;
  card_name: string;
  action: "add" | "cut" | "role";
  quantity?: number;
  item_id?: number | null;
  reason?: string;
  /** Undo payload; see deck/membership.ts. */
  before?: unknown;
}

export interface LogInput {
  /** NULL only for a decision on a profile edit. */
  deck_id: number | null;
  kind: LogKind;
  summary?: string;
  decision_reason?: string;
  proposal_id?: number | null;
  document_edit_id?: number | null;
  undo_of?: number | null;
}

// Spec §8.4. An entry waits for an agent to read it when it is something the
// owner did that an agent could learn from: every decision, and every direct
// change to a Paper deck. What an agent did itself needs no review, and
// neither does the owner assembling a Theorycrafted deck card by card.
function awaitsReview(actor: Actor, kind: LogKind, deckCategory: string | null): boolean {
  if (actor.via !== "ui") return false;
  if (kind === "accept" || kind === "reject") return true;
  return deckCategory === "paper";
}

export function writeLog(
  db: Database,
  actor: Actor,
  entry: LogInput,
  cards: LogCardInput[] = [],
): number {
  const deck = entry.deck_id == null ? null : requireDeckOwner(db, actor, entry.deck_id);
  for (const [table, id] of [["proposals", entry.proposal_id], ["document_edits", entry.document_edit_id], ["log", entry.undo_of]] as const) {
    if (id == null) continue;
    const row = db.prepare(`SELECT ${table === "proposals" ? "(SELECT user_id FROM decks WHERE id = deck_id)" : "user_id"} AS user_id FROM ${table} WHERE id = ?`).get(id) as { user_id: number } | undefined;
    if (!row) throw new ServiceError("The linked record does not exist", 404);
    requireSelf(actor, row.user_id);
  }

  const r = db
    .prepare(
      `INSERT INTO log (deck_id, revision, kind, summary, ruling_reason, proposal_id,
                        document_edit_id, author, undo_of, user_id, author_user_id, author_via, author_label, reviewed_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ${
         awaitsReview(actor, entry.kind, deck?.category ?? null) ? "NULL" : "datetime('now')"
       })`,
    )
    .run(
      entry.deck_id,
      deck?.revision ?? 0,
      entry.kind,
      entry.summary ?? "",
      (entry.decision_reason ?? "").trim(),
      entry.proposal_id ?? null,
      entry.document_edit_id ?? null,
      authorOf(actor),
      entry.undo_of ?? null, actor.user_id, actor.user_id, actor.via, actor.label,
    );
  const logId = Number(r.lastInsertRowid);

  if (cards.length) {
    const ins = db.prepare(
      `INSERT INTO log_cards (log_id, deck_id, oracle_id, card_name, action, quantity, item_id, reason, before_json)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    );
    for (const c of cards)
      ins.run(
        logId,
        entry.deck_id,
        c.oracle_id,
        c.card_name,
        c.action,
        c.quantity ?? 1,
        c.item_id ?? null,
        c.reason ?? "",
        c.before === undefined ? null : JSON.stringify(c.before),
      );
  }
  return logId;
}

// ---------- reading ----------

export interface LogCard {
  oracle_id: string;
  card_name: string;
  action: "add" | "cut" | "role";
  quantity: number;
  item_id: number | null;
  reason: string;
}

export interface LogEntry {
  id: number;
  deck_id: number | null;
  read_only: boolean;
  deck_name: string | null;
  revision: number;
  kind: LogKind;
  summary: string;
  decision_reason: string;
  proposal_id: number | null;
  document_edit_id: number | null;
  author: string;
  author_user_id: number;
  author_via: "ui" | "agent";
  author_label: string;
  author_display?: string;
  undo_of: number | null;
  undone_by: number | null;
  reviewed_at: string | null;
  ts: string;
  /** Left off the public page's decisions log by the owner (§18.2). */
  hidden: number;
  cards: LogCard[];
}

const ENTRY_SELECT = `SELECT l.id, l.deck_id, (d.category = 'archived') AS read_only, d.name AS deck_name, l.revision, l.kind, l.summary,
       l.ruling_reason AS decision_reason, l.proposal_id, l.document_edit_id, l.author, l.author_user_id, l.author_via, l.author_label, l.undo_of, l.undone_by,
       l.reviewed_at, l.ts, l.hidden
  FROM log l LEFT JOIN decks d ON d.id = l.deck_id`;

function withCards(db: Database, actor: Actor, rows: Array<Omit<LogEntry, "cards">>): LogEntry[] {
  const stmt = db.prepare(
    "SELECT oracle_id, card_name, action, quantity, item_id, reason FROM log_cards WHERE log_id = ? ORDER BY id",
  );
  return rows.map((r) => ({ ...r, author_display: displayAuthor(db, r), read_only: !!r.read_only, cards: stmt.all(r.id) as unknown as LogCard[] }));
}

export interface LogQuery {
  deckId?: number;
  unreviewedOnly?: boolean;
  /** ISO date or datetime; entries at or after it. */
  since?: string;
  limit?: number;
}

/** Newest first. */
export function listLog(db: Database, actor: Actor, query: LogQuery = {}): LogEntry[] {
  if (query.deckId !== undefined) requireDeck(db, actor, query.deckId);
  const where: string[] = ["l.user_id = ?"];
  const params: Array<string | number> = [actor.user_id];
  if (query.deckId !== undefined) {
    where.push("l.deck_id = ?");
    params.push(query.deckId);
  }
  if (query.unreviewedOnly) where.push("l.reviewed_at IS NULL");
  if (query.since) {
    where.push("l.ts >= ?");
    params.push(query.since);
  }
  const rows = db
    .prepare(
      `${ENTRY_SELECT} ${where.length ? `WHERE ${where.join(" AND ")}` : ""} ORDER BY l.id DESC LIMIT ?`,
    )
    .all(...params, Math.min(Math.max(query.limit ?? 50, 1), 500)) as unknown as Array<
    Omit<LogEntry, "cards">
  >;
  return withCards(db, actor, rows);
}

/** Hide one entry from the public decisions log, or show it again (§18.2).
 * The owner's, in the app; the entry itself and its undo are untouched. */
export function hideLogEntry(db: Database, actor: Actor, logId: number, hidden: boolean): LogEntry {
  const entry = getLogEntry(db, actor, logId)!;
  if (entry.deck_id == null) throw new ServiceError(`Log entry ${logId} is not about a deck`, 404);
  requireDeckOwner(db, actor, entry.deck_id, "change what this deck publishes");
  requireUi(actor, "change what a deck publishes");
  if (typeof hidden !== "boolean") throw new ServiceError("hidden must be true or false");
  db.prepare("UPDATE log SET hidden = ? WHERE id = ? AND user_id = ?").run(hidden ? 1 : 0, logId, actor.user_id);
  return getLogEntry(db, actor, logId)!;
}

export function getLogEntry(db: Database, actor: Actor, logId: number): LogEntry | null {
  requireActor(actor);
  const row = db.prepare(`${ENTRY_SELECT} WHERE l.id = ? AND l.user_id = ?`).get(logId, actor.user_id) as
    | Omit<LogEntry, "cards">
    | undefined;
  if (!row) throw new ServiceError(`Log entry ${logId} not found`, 404);
  return withCards(db, actor, [row])[0];
}

export interface CardDecision {
  log_id: number;
  kind: LogKind;
  action: "add" | "cut" | "role";
  revision: number;
  /** The proposer's reason for the item, if it came from a proposal. */
  reason: string;
  /** The owner's reason for the decision, if they gave one. */
  decision_reason: string;
  author: string;
  author_user_id: number;
  author_via: "ui" | "agent";
  author_label: string;
  author_display?: string;
  undone: boolean;
  ts: string;
  /** Set when the decision was made in a deck this one was forked from. */
  from_deck: string | null;
}

// A deck and the decks it was forked from, nearest first.
function lineage(db: Database, actor: Actor, deckId: number): number[] {
  const chain: number[] = [];
  const parentOf = db.prepare("SELECT parent_id FROM decks WHERE id = ?");
  for (let cur: number | null = deckId; cur != null && !chain.includes(cur); ) {
    if (!isDeckOwner(db, actor, cur)) break;
    chain.push(cur);
    cur = (parentOf.get(cur) as { parent_id: number | null } | undefined)?.parent_id ?? null;
  }
  return chain;
}

// Spec §8.3: what has been decided about this card in this deck, newest
// first. Attached by the app wherever the card appears, so nobody has to
// remember to ask.
//
// A fork inherits its parents' history: "rejected from the original as too
// slow" is exactly what someone reworking the fork needs to see, and a fork
// starts with no decisions of its own. Those entries are labelled with the deck
// they were made in. (As built: the first real-agent run on a fork had no
// decisions to read, and said so.)
export function cardHistory(
  db: Database, actor: Actor,
  deckId: number,
  oracleId: string,
  limit = 5,
): CardDecision[] {
  requireDeck(db, actor, deckId);
  const decks = lineage(db, actor, deckId);
  return (
    db
      .prepare(
        `SELECT l.id AS log_id, l.kind, lc.action, l.revision, lc.reason, l.ruling_reason AS decision_reason,
                l.author, l.author_user_id, l.author_via, l.author_label, l.undone_by, l.ts, lc.deck_id, d.name AS deck_name
         FROM log_cards lc JOIN log l ON l.id = lc.log_id JOIN decks d ON d.id = lc.deck_id
         WHERE lc.deck_id IN (${decks.map(() => "?").join(", ")}) AND lc.oracle_id = ?
         ORDER BY l.id DESC LIMIT ?`,
      )
      .all(...decks, oracleId, limit) as unknown as Array<
      Omit<CardDecision, "undone" | "from_deck"> & {
        undone_by: number | null;
        deck_id: number;
        deck_name: string;
      }
    >
  ).map(({ undone_by, deck_id, deck_name, ...r }) => ({
    ...r,
    author_display: displayAuthor(db, r),
    undone: undone_by != null,
    from_deck: deck_id === deckId ? null : deck_name,
  }));
}

/** The same lookup for many cards at once, keyed by oracle_id; cards with no history are absent. */
export function cardHistories(
  db: Database, actor: Actor,
  deckId: number,
  oracleIds: string[],
  limit = 3,
): Map<string, CardDecision[]> {
  requireDeck(db, actor, deckId);
  const out = new Map<string, CardDecision[]>();
  for (const id of new Set(oracleIds)) {
    const history = cardHistory(db, actor, deckId, id, limit);
    if (history.length) out.set(id, history);
  }
  return out;
}

// ---------- the review queue (spec §8.4) ----------

export function unreviewedCount(db: Database, actor: Actor): number {
  return (db.prepare("SELECT COUNT(*) n FROM log WHERE reviewed_at IS NULL AND user_id = ?").get(actor.user_id) as { n: number })
    .n;
}

// Mark entries read. `through` clears that entry's deck up to and including its id,
// which is what an agent that has just read the whole queue wants.
export function markReviewed(
  db: Database, actor: Actor,
  which: { ids?: number[]; through?: number },
): number {
  requireActor(actor);
  if (which.through != null) {
    if (which.through === 0) return 0;
    const entry = db.prepare("SELECT deck_id FROM log WHERE id = ? AND user_id = ?").get(which.through, actor.user_id) as { deck_id: number | null } | undefined;
    if (!entry) throw new ServiceError("The decision to review through does not exist", 404);
    const r = db
      .prepare("UPDATE log SET reviewed_at = datetime('now') WHERE reviewed_at IS NULL AND id <= ? AND deck_id IS ? AND user_id = ?")
      .run(which.through, entry.deck_id, actor.user_id);
    return Number(r.changes);
  }
  const ids = which.ids ?? [];
  if (!ids.length) throw new ServiceError("Give the ids of the decisions you have read, or 'through'");
  for (const id of ids) {
    const tenant = db.prepare("SELECT user_id FROM log WHERE id = ? AND user_id = ?").get(id, actor.user_id) as { user_id: number } | undefined;
    if (!tenant) throw new ServiceError(`Log entry ${id} not found`, 404);
  }
  const stmt = db.prepare(
    "UPDATE log SET reviewed_at = datetime('now') WHERE reviewed_at IS NULL AND id = ? AND user_id = ?",
  );
  let n = 0;
  for (const id of ids) n += Number(stmt.run(id, actor.user_id).changes);
  return n;
}
