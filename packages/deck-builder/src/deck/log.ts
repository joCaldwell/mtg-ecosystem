// The rulings log (spec §8.2): the deck's version history and the record of
// what the owner decided and why. Every act lands here through writeLog —
// rulings on proposals and document edits, direct edits, imports, undos — and
// the cards an act touched are queryable per card (§8.3).

import type { DatabaseSync } from "node:sqlite";
import { ServiceError } from "../errors.ts";
import { type Actor, authorOf } from "../actor.ts";

export type LogKind = "accept" | "reject" | "edit" | "import" | "undo";

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
  /** NULL only for a ruling on a profile edit. */
  deck_id: number | null;
  kind: LogKind;
  summary?: string;
  ruling_reason?: string;
  proposal_id?: number | null;
  document_edit_id?: number | null;
  undo_of?: number | null;
}

// Spec §8.4. An entry waits for an agent to read it when it is something the
// owner did that an agent could learn from: every ruling, and every direct
// change to a Paper deck. What an agent did itself needs no review, and
// neither does the owner assembling a Theorycrafted deck card by card.
function awaitsReview(actor: Actor, kind: LogKind, deckCategory: string | null): boolean {
  if (actor.kind !== "owner") return false;
  if (kind === "accept" || kind === "reject") return true;
  return deckCategory === "paper";
}

export function writeLog(
  db: DatabaseSync,
  actor: Actor,
  entry: LogInput,
  cards: LogCardInput[] = [],
): number {
  const deck =
    entry.deck_id == null
      ? null
      : (db.prepare("SELECT revision, category FROM decks WHERE id = ?").get(entry.deck_id) as
          | { revision: number; category: string }
          | undefined);
  if (entry.deck_id != null && !deck) throw new ServiceError(`Deck ${entry.deck_id} not found`, 404);

  const r = db
    .prepare(
      `INSERT INTO log (deck_id, revision, kind, summary, ruling_reason, proposal_id,
                        document_edit_id, author, undo_of, reviewed_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ${
         awaitsReview(actor, entry.kind, deck?.category ?? null) ? "NULL" : "datetime('now')"
       })`,
    )
    .run(
      entry.deck_id,
      deck?.revision ?? 0,
      entry.kind,
      entry.summary ?? "",
      (entry.ruling_reason ?? "").trim(),
      entry.proposal_id ?? null,
      entry.document_edit_id ?? null,
      authorOf(actor),
      entry.undo_of ?? null,
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
  ruling_reason: string;
  proposal_id: number | null;
  document_edit_id: number | null;
  author: string;
  undo_of: number | null;
  undone_by: number | null;
  reviewed_at: string | null;
  ts: string;
  cards: LogCard[];
}

const ENTRY_SELECT = `SELECT l.id, l.deck_id, (d.category = 'legacy') AS read_only, d.name AS deck_name, l.revision, l.kind, l.summary,
       l.ruling_reason, l.proposal_id, l.document_edit_id, l.author, l.undo_of, l.undone_by,
       l.reviewed_at, l.ts
  FROM log l LEFT JOIN decks d ON d.id = l.deck_id`;

function withCards(db: DatabaseSync, rows: Array<Omit<LogEntry, "cards">>): LogEntry[] {
  const stmt = db.prepare(
    "SELECT oracle_id, card_name, action, quantity, item_id, reason FROM log_cards WHERE log_id = ? ORDER BY id",
  );
  return rows.map((r) => ({ ...r, read_only: !!r.read_only, cards: stmt.all(r.id) as unknown as LogCard[] }));
}

export interface LogQuery {
  deckId?: number;
  unreviewedOnly?: boolean;
  /** ISO date or datetime; entries at or after it. */
  since?: string;
  limit?: number;
}

/** Newest first. */
export function listLog(db: DatabaseSync, query: LogQuery = {}): LogEntry[] {
  const where: string[] = [];
  const params: Array<string | number> = [];
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
  return withCards(db, rows);
}

export function getLogEntry(db: DatabaseSync, logId: number): LogEntry | null {
  const row = db.prepare(`${ENTRY_SELECT} WHERE l.id = ?`).get(logId) as
    | Omit<LogEntry, "cards">
    | undefined;
  return row ? withCards(db, [row])[0] : null;
}

export interface CardRuling {
  log_id: number;
  kind: LogKind;
  action: "add" | "cut" | "role";
  revision: number;
  /** The proposer's reason for the item, if it came from a proposal. */
  reason: string;
  /** The owner's reason for the ruling, if they gave one. */
  ruling_reason: string;
  author: string;
  undone: boolean;
  ts: string;
  /** Set when the ruling was made in a deck this one was forked from. */
  from_deck: string | null;
}

// A deck and the decks it was forked from, nearest first.
function lineage(db: DatabaseSync, deckId: number): number[] {
  const chain: number[] = [];
  const parentOf = db.prepare("SELECT parent_id FROM decks WHERE id = ?");
  for (let cur: number | null = deckId; cur != null && !chain.includes(cur); ) {
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
// starts with no rulings of its own. Those entries are labelled with the deck
// they were made in. (As built: the first real-agent run on a fork had no
// rulings to read, and said so.)
export function cardHistory(
  db: DatabaseSync,
  deckId: number,
  oracleId: string,
  limit = 5,
): CardRuling[] {
  const decks = lineage(db, deckId);
  return (
    db
      .prepare(
        `SELECT l.id AS log_id, l.kind, lc.action, l.revision, lc.reason, l.ruling_reason,
                l.author, l.undone_by, l.ts, lc.deck_id, d.name AS deck_name
         FROM log_cards lc JOIN log l ON l.id = lc.log_id JOIN decks d ON d.id = lc.deck_id
         WHERE lc.deck_id IN (${decks.map(() => "?").join(", ")}) AND lc.oracle_id = ?
         ORDER BY l.id DESC LIMIT ?`,
      )
      .all(...decks, oracleId, limit) as unknown as Array<
      Omit<CardRuling, "undone" | "from_deck"> & {
        undone_by: number | null;
        deck_id: number;
        deck_name: string;
      }
    >
  ).map(({ undone_by, deck_id, deck_name, ...r }) => ({
    ...r,
    undone: undone_by != null,
    from_deck: deck_id === deckId ? null : deck_name,
  }));
}

/** The same lookup for many cards at once, keyed by oracle_id; cards with no history are absent. */
export function cardHistories(
  db: DatabaseSync,
  deckId: number,
  oracleIds: string[],
  limit = 3,
): Map<string, CardRuling[]> {
  const out = new Map<string, CardRuling[]>();
  for (const id of new Set(oracleIds)) {
    const history = cardHistory(db, deckId, id, limit);
    if (history.length) out.set(id, history);
  }
  return out;
}

// ---------- the review queue (spec §8.4) ----------

export function unreviewedCount(db: DatabaseSync): number {
  return (db.prepare("SELECT COUNT(*) n FROM log WHERE reviewed_at IS NULL").get() as { n: number })
    .n;
}

// Mark entries read. `through` clears everything up to and including an id,
// which is what an agent that has just read the whole queue wants.
export function markReviewed(
  db: DatabaseSync,
  which: { ids?: number[]; through?: number },
): number {
  if (which.through != null) {
    const r = db
      .prepare("UPDATE log SET reviewed_at = datetime('now') WHERE reviewed_at IS NULL AND id <= ?")
      .run(which.through);
    return Number(r.changes);
  }
  const ids = which.ids ?? [];
  if (!ids.length) throw new ServiceError("Give the ids of the rulings you have read, or 'through'");
  const stmt = db.prepare(
    "UPDATE log SET reviewed_at = datetime('now') WHERE reviewed_at IS NULL AND id = ?",
  );
  let n = 0;
  for (const id of ids) n += Number(stmt.run(id).changes);
  return n;
}
