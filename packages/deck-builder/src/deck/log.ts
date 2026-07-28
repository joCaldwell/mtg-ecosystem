// The decision log is the deck's version history (spec §7.4): every ruling —
// proposal item, audit dismissal, brief edit — lands here through logEntry,
// and typed rejections route their side effects (§7.2) through
// recordRejection so no surface can drift from the others. Undo reads the
// log back; it lives here with the writers.

import type { DatabaseSync } from "node:sqlite";
import { ServiceError } from "../errors.ts";
import { withTransaction } from "../db.ts";
import { addCard, getDeck, removeCard, requireCard, requireDeck, updateCard } from "./service.ts";

export type RejectionType = "hard_filter" | "thesis_change" | "playtest_finding" | "soft";

export const REJECTION_TYPES: RejectionType[] = [
  "hard_filter",
  "thesis_change",
  "playtest_finding",
  "soft",
];

export function logEntry(
  db: DatabaseSync,
  deckId: number,
  fields: Partial<{
    kind: string;
    action: string | null;
    oracle_id: string | null;
    card_name: string | null;
    rationale: string | null;
    rejection_type: string | null;
    rejection_reason: string | null;
    proposal_id: number | null;
    item_id: number | null;
    undo_of: number | null;
    snapshot_json: string | null;
    brief_flag: number;
  }>,
  revision: number,
): number {
  const r = db
    .prepare(
      `INSERT INTO decision_log
       (deck_id, revision, kind, action, oracle_id, card_name, rationale, rejection_type,
        rejection_reason, proposal_id, item_id, undo_of, snapshot_json, brief_flag)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      deckId,
      revision,
      fields.kind ?? "accept",
      fields.action ?? null,
      fields.oracle_id ?? null,
      fields.card_name ?? null,
      fields.rationale ?? null,
      fields.rejection_type ?? null,
      fields.rejection_reason ?? null,
      fields.proposal_id ?? null,
      fields.item_id ?? null,
      fields.undo_of ?? null,
      fields.snapshot_json ?? null,
      fields.brief_flag ?? 0,
    );
  return Number(r.lastInsertRowid);
}

// One writer for every typed rejection (spec §7.2): logs it and routes the
// side effects — hard_filter on a rejected *add* removes the card from future
// searches, playtest_finding becomes a durable card note, thesis_change flags
// the brief. Proposal rejections and audit dismissals both come through here.
export function recordRejection(
  db: DatabaseSync,
  deckId: number,
  fields: {
    type: RejectionType;
    reason: string;
    rationale: string;
    action?: "add" | "cut" | null;
    oracle_id?: string | null;
    card_name?: string | null;
    proposal_id?: number | null;
    item_id?: number | null;
  },
  revision: number,
): number {
  const id = logEntry(
    db,
    deckId,
    {
      kind: "reject",
      action: fields.action ?? null,
      oracle_id: fields.oracle_id ?? null,
      card_name: fields.card_name ?? null,
      rationale: fields.rationale,
      rejection_type: fields.type,
      rejection_reason: fields.reason,
      proposal_id: fields.proposal_id ?? null,
      item_id: fields.item_id ?? null,
      brief_flag: fields.type === "thesis_change" ? 1 : 0,
    },
    revision,
  );
  // Only a rejected add hard-filters: the card is not in the deck and should
  // never be searched up again. A cut rejection means "keep it", which is not
  // a filter.
  if (fields.type === "hard_filter" && fields.action === "add" && fields.oracle_id) {
    db.prepare(
      "INSERT OR IGNORE INTO hard_filters (deck_id, oracle_id, card_name, reason) VALUES (?, ?, ?, ?)",
    ).run(deckId, fields.oracle_id, fields.card_name ?? "", fields.reason);
  }
  if (fields.type === "playtest_finding" && fields.oracle_id) {
    db.prepare(
      "INSERT INTO card_notes (deck_id, oracle_id, card_name, note) VALUES (?, ?, ?, ?)",
    ).run(deckId, fields.oracle_id, fields.card_name ?? "", fields.reason);
  }
  return id;
}

// ---------- reading ----------

export interface LogEntry {
  id: number;
  deck_id: number;
  revision: number;
  kind: "accept" | "reject" | "undo" | "filter_removed";
  action: "add" | "cut" | "import" | null;
  oracle_id: string | null;
  card_name: string | null;
  rationale: string | null;
  rejection_type: RejectionType | null;
  rejection_reason: string | null;
  proposal_id: number | null;
  item_id: number | null;
  undo_of: number | null;
  undone_by: number | null;
  snapshot_json: string | null;
  brief_flag: number;
  ts: string;
}

export function getLog(db: DatabaseSync, deckId: number, limit = 50): LogEntry[] {
  return db
    .prepare("SELECT * FROM decision_log WHERE deck_id = ? ORDER BY id DESC LIMIT ?")
    .all(deckId, limit) as unknown as LogEntry[];
}

export function getCardHistory(db: DatabaseSync, deckId: number, oracleId: string): LogEntry[] {
  return db
    .prepare("SELECT * FROM decision_log WHERE deck_id = ? AND oracle_id = ? ORDER BY id DESC")
    .all(deckId, oracleId) as unknown as LogEntry[];
}

// ---------- snapshots (undo payloads) ----------

export interface CardSnapshot {
  oracle_id: string;
  slot_id: number | null;
  role: string;
  owned: boolean | number;
  quantity: number;
  tag_ids: number[];
}

export function snapshotCard(db: DatabaseSync, deckId: number, oracleId: string) {
  const row = db
    .prepare("SELECT slot_id, role, owned, quantity FROM deck_cards WHERE deck_id = ? AND oracle_id = ?")
    .get(deckId, oracleId) as
    | { slot_id: number | null; role: string; owned: number; quantity: number }
    | undefined;
  if (!row) return null;
  const tagIds = (
    db
      .prepare("SELECT tag_id FROM deck_card_tags WHERE deck_id = ? AND oracle_id = ?")
      .all(deckId, oracleId) as unknown as { tag_id: number }[]
  ).map((t) => t.tag_id);
  return { ...row, tag_ids: tagIds };
}

// One log row for a whole applied import, carrying the pre-import list so the
// row's ordinary "undo" button reverts the entire import. Kind stays 'accept'
// so it reads and undoes like any other applied change; action 'import' is
// what tells undoDecision to restore wholesale rather than per card. Callers
// run this inside their own transaction.
export function logImport(
  db: DatabaseSync,
  deckId: number,
  summary: string,
  before: CardSnapshot[],
): number {
  return logEntry(
    db,
    deckId,
    {
      kind: "accept",
      action: "import",
      rationale: summary,
      snapshot_json: JSON.stringify({ cards: before }),
    },
    requireDeck(db, deckId).revision,
  );
}

// Put the deck back exactly as the snapshot found it. Wiping first sidesteps
// the command-zone capacity check, which would otherwise trip while a
// commander from the snapshot and one from the import both exist.
function restoreCards(db: DatabaseSync, deckId: number, snapshot: CardSnapshot[]): void {
  for (const row of db
    .prepare("SELECT oracle_id FROM deck_cards WHERE deck_id = ?")
    .all(deckId) as unknown as { oracle_id: string }[])
    removeCard(db, deckId, row.oracle_id);

  const existingTags = new Set(
    (db.prepare("SELECT id FROM tags WHERE deck_id = ?").all(deckId) as unknown as {
      id: number;
    }[]).map((t) => t.id),
  );
  const existingSlots = new Set(
    (db.prepare("SELECT id FROM slots WHERE deck_id = ?").all(deckId) as unknown as {
      id: number;
    }[]).map((s) => s.id),
  );

  for (const c of snapshot) {
    const slotId = c.slot_id != null && existingSlots.has(c.slot_id) ? c.slot_id : null;
    addCard(db, deckId, c.oracle_id, {
      slotId,
      role: (c.role as "card" | "commander" | "companion") ?? "card",
    });
    const patch: Parameters<typeof updateCard>[3] = {
      owned: !!c.owned,
      tagIds: (c.tag_ids ?? []).filter((t) => existingTags.has(t)),
    };
    if (c.quantity > 1) patch.quantity = c.quantity;
    updateCard(db, deckId, c.oracle_id, patch);
  }
}

// ---------- undo (log reversal, spec §7.4) ----------

export function undoDecision(db: DatabaseSync, deckId: number, logId: number): void {
  const entry = db
    .prepare("SELECT * FROM decision_log WHERE id = ? AND deck_id = ?")
    .get(logId, deckId) as
    | {
        id: number;
        kind: string;
        action: string | null;
        oracle_id: string | null;
        card_name: string | null;
        snapshot_json: string | null;
        undone_by: number | null;
      }
    | undefined;
  if (!entry) throw new ServiceError(`Log entry ${logId} not found`, 404);
  if (entry.kind !== "accept")
    throw new ServiceError("Only accepted (applied) decisions can be undone");
  if (entry.undone_by != null)
    throw new ServiceError("This decision has already been undone");

  withTransaction(db, () => {
    const undoId =
      entry.action === "import"
        ? undoImport(db, deckId, entry)
        : entry.action === "add"
          ? undoAdd(db, deckId, entry)
          : undoCut(db, deckId, entry);
    db.prepare("UPDATE decision_log SET undone_by = ? WHERE id = ?").run(undoId, entry.id);
  });
}

type LogRow = { id: number; oracle_id: string | null; card_name: string | null; snapshot_json: string | null };

// Bulk entry: the snapshot is the whole pre-import list, and undoing it
// means restoring that list rather than reversing one card.
function undoImport(db: DatabaseSync, deckId: number, entry: LogRow): number {
  const current = (getDeck(db, deckId).cards as unknown as CardSnapshot[]).map((c) => ({
    oracle_id: c.oracle_id,
    slot_id: c.slot_id,
    role: c.role,
    owned: c.owned,
    quantity: c.quantity,
    tag_ids: c.tag_ids,
  }));
  const before = (JSON.parse(entry.snapshot_json ?? "{}").cards ?? []) as CardSnapshot[];
  restoreCards(db, deckId, before);
  return logEntry(
    db,
    deckId,
    {
      kind: "undo",
      action: "import",
      rationale: `Undo of decision #${entry.id} — the deck is back to its pre-import list`,
      undo_of: entry.id,
      snapshot_json: JSON.stringify({ cards: current }),
    },
    requireDeck(db, deckId).revision,
  );
}

function undoAdd(db: DatabaseSync, deckId: number, entry: LogRow): number {
  const snapshot = snapshotCard(db, deckId, entry.oracle_id!);
  if (!snapshot)
    throw new ServiceError(`${entry.card_name} is no longer in the deck; nothing to undo`);
  removeCard(db, deckId, entry.oracle_id!);
  return logEntry(
    db,
    deckId,
    {
      kind: "undo",
      action: "cut",
      oracle_id: entry.oracle_id,
      card_name: entry.card_name,
      rationale: `Undo of decision #${entry.id}`,
      undo_of: entry.id,
      snapshot_json: JSON.stringify(snapshot),
    },
    requireDeck(db, deckId).revision,
  );
}

function undoCut(db: DatabaseSync, deckId: number, entry: LogRow): number {
  if (snapshotCard(db, deckId, entry.oracle_id!))
    throw new ServiceError(`${entry.card_name} is already back in the deck`);
  const snapshot = entry.snapshot_json ? JSON.parse(entry.snapshot_json) : {};
  let slotId: number | null = snapshot.slot_id ?? null;
  if (
    slotId != null &&
    !db.prepare("SELECT 1 FROM slots WHERE id = ? AND deck_id = ?").get(slotId, deckId)
  )
    slotId = null;
  addCard(db, deckId, entry.oracle_id!, { slotId, role: snapshot.role ?? "card" });
  const existingTags = new Set(
    (
      db.prepare("SELECT id FROM tags WHERE deck_id = ?").all(deckId) as unknown as {
        id: number;
      }[]
    ).map((t) => t.id),
  );
  const patch: Parameters<typeof updateCard>[3] = {
    owned: !!snapshot.owned,
    tagIds: (snapshot.tag_ids ?? []).filter((t: number) => existingTags.has(t)),
  };
  if (snapshot.quantity > 1) patch.quantity = snapshot.quantity;
  updateCard(db, deckId, entry.oracle_id!, patch);
  return logEntry(
    db,
    deckId,
    {
      kind: "undo",
      action: "add",
      oracle_id: entry.oracle_id,
      card_name: entry.card_name,
      rationale: `Undo of decision #${entry.id}`,
      undo_of: entry.id,
      snapshot_json: null,
    },
    requireDeck(db, deckId).revision,
  );
}

// ---------- hard filters and card notes (rejection routing targets) ----------

export function isHardFiltered(db: DatabaseSync, deckId: number, oracleId: string): boolean {
  return !!db
    .prepare("SELECT 1 FROM hard_filters WHERE deck_id = ? AND oracle_id = ?")
    .get(deckId, oracleId);
}

export interface HardFilter {
  oracle_id: string;
  card_name: string;
  reason: string;
  created_at: string;
}

export function listHardFilters(db: DatabaseSync, deckId: number): HardFilter[] {
  return db
    .prepare("SELECT oracle_id, card_name, reason, created_at FROM hard_filters WHERE deck_id = ? ORDER BY card_name")
    .all(deckId) as unknown as HardFilter[];
}

export function removeHardFilter(db: DatabaseSync, deckId: number, oracleId: string): void {
  const row = db
    .prepare("SELECT card_name FROM hard_filters WHERE deck_id = ? AND oracle_id = ?")
    .get(deckId, oracleId) as { card_name: string } | undefined;
  if (!row) throw new ServiceError("No such hard filter", 404);
  withTransaction(db, () => {
    db.prepare("DELETE FROM hard_filters WHERE deck_id = ? AND oracle_id = ?").run(deckId, oracleId);
    logEntry(
      db,
      deckId,
      {
        kind: "filter_removed",
        oracle_id: oracleId,
        card_name: row.card_name,
        rationale: "Hard filter removed",
      },
      requireDeck(db, deckId).revision,
    );
  });
}

export interface CardNote {
  id: number;
  oracle_id: string;
  card_name: string;
  note: string;
  created_at: string;
}

export function listCardNotes(db: DatabaseSync, deckId: number): CardNote[] {
  return db
    .prepare("SELECT id, oracle_id, card_name, note, created_at FROM card_notes WHERE deck_id = ? ORDER BY id DESC")
    .all(deckId) as unknown as CardNote[];
}

export function addCardNote(db: DatabaseSync, deckId: number, oracleId: string, note: string): number {
  if (!note?.trim()) throw new ServiceError("Note cannot be empty");
  const name = requireCard(db, oracleId);
  const r = db
    .prepare("INSERT INTO card_notes (deck_id, oracle_id, card_name, note) VALUES (?, ?, ?, ?)")
    .run(deckId, oracleId, name, note.trim());
  return Number(r.lastInsertRowid);
}
