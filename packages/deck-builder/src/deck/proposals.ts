import type { DatabaseSync } from "node:sqlite";
import { ServiceError, addCard, removeCard, updateCard } from "./service.ts";

export type ProposalSource = "manual" | "agent" | "audit" | "import";
export type RejectionType = "hard_filter" | "thesis_change" | "playtest_finding" | "soft";

export const REJECTION_TYPES: RejectionType[] = [
  "hard_filter",
  "thesis_change",
  "playtest_finding",
  "soft",
];

export interface ProposalItemInput {
  action: "add" | "cut";
  oracle_id: string;
  slot_id?: number | null;
  rationale: string;
  group_id?: string | null;
}

// The 3–5 cap forces the agent to rank (spec §7.1). Import diffs are exempt
// (agreed deviation) — they're the user's own changes re-entering the gate.
const MAX_ITEMS = 5;

function currentRevision(db: DatabaseSync, deckId: number): number {
  const row = db.prepare("SELECT revision FROM decks WHERE id = ?").get(deckId) as
    | { revision: number }
    | undefined;
  if (!row) throw new ServiceError(`Deck ${deckId} not found`, 404);
  return row.revision;
}

function cardName(db: DatabaseSync, oracleId: string): string {
  const row = db.prepare("SELECT name FROM cards WHERE oracle_id = ?").get(oracleId) as
    | { name: string }
    | undefined;
  if (!row) throw new ServiceError(`Unknown oracle_id ${oracleId}`, 404);
  return row.name;
}

export function isHardFiltered(db: DatabaseSync, deckId: number, oracleId: string): boolean {
  return !!db
    .prepare("SELECT 1 FROM hard_filters WHERE deck_id = ? AND oracle_id = ?")
    .get(deckId, oracleId);
}

// ---------- creation ----------

export function createProposal(
  db: DatabaseSync,
  deckId: number,
  items: ProposalItemInput[],
  opts: { source?: ProposalSource; note?: string } = {},
): number {
  currentRevision(db, deckId); // 404 if deck missing
  const source = opts.source ?? "manual";
  if (!items.length) throw new ServiceError("A proposal needs at least one item");
  if (source !== "import" && items.length > MAX_ITEMS)
    throw new ServiceError(`A proposal contains at most ${MAX_ITEMS} items (got ${items.length})`);

  const inDeck = new Set(
    (
      db.prepare("SELECT oracle_id FROM deck_cards WHERE deck_id = ?").all(deckId) as unknown as {
        oracle_id: string;
      }[]
    ).map((r) => r.oracle_id),
  );

  for (const item of items) {
    if (!item.rationale?.trim())
      throw new ServiceError(`Item for ${item.oracle_id} needs a rationale — the reasons are the point`);
    const name = cardName(db, item.oracle_id);
    if (item.action === "cut" && !inDeck.has(item.oracle_id))
      throw new ServiceError(`Cannot propose cutting ${name} — it is not in the deck`);
    if (item.action === "add" && inDeck.has(item.oracle_id))
      throw new ServiceError(`Cannot propose adding ${name} — it is already in the deck`);
    if (item.action === "add" && isHardFiltered(db, deckId, item.oracle_id))
      throw new ServiceError(
        `${name} is hard-filtered for this deck (remove the filter first to re-propose it)`,
      );
  }

  db.exec("BEGIN");
  try {
    const r = db
      .prepare("INSERT INTO proposals (deck_id, source, note) VALUES (?, ?, ?)")
      .run(deckId, source, opts.note ?? "");
    const proposalId = Number(r.lastInsertRowid);
    const ins = db.prepare(
      `INSERT INTO proposal_items (proposal_id, action, oracle_id, slot_id, rationale, group_id)
       VALUES (?, ?, ?, ?, ?, ?)`,
    );
    for (const item of items)
      ins.run(
        proposalId,
        item.action,
        item.oracle_id,
        item.slot_id ?? null,
        item.rationale.trim(),
        item.group_id ?? null,
      );
    db.exec("COMMIT");
    return proposalId;
  } catch (e) {
    db.exec("ROLLBACK");
    throw e;
  }
}

// ---------- reading ----------

export interface ProposalItemView {
  id: number;
  proposal_id: number;
  action: "add" | "cut";
  oracle_id: string;
  card_name: string;
  mana_cost: string | null;
  type_line: string;
  oracle_text: string;
  slot_id: number | null;
  rationale: string;
  group_id: string | null;
  status: "pending" | "accepted" | "rejected";
}

export function listProposals(db: DatabaseSync, deckId: number, status?: "open" | "resolved") {
  const proposals = db
    .prepare(
      `SELECT id, source, note, status, created_at FROM proposals
       WHERE deck_id = ? ${status ? "AND status = ?" : ""} ORDER BY id DESC`,
    )
    .all(...(status ? [deckId, status] : [deckId])) as unknown as Array<{
    id: number;
    source: ProposalSource;
    note: string;
    status: string;
    created_at: string;
    items: ProposalItemView[];
  }>;
  const itemStmt = db.prepare(
    `SELECT pi.id, pi.proposal_id, pi.action, pi.oracle_id, c.name AS card_name,
            c.mana_cost, c.type_line, c.oracle_text,
            pi.slot_id, pi.rationale, pi.group_id, pi.status
     FROM proposal_items pi JOIN cards c ON c.oracle_id = pi.oracle_id
     WHERE pi.proposal_id = ? ORDER BY pi.id`,
  );
  for (const p of proposals) p.items = itemStmt.all(p.id) as unknown as ProposalItemView[];
  return proposals;
}

// Pending delta vs 100 and vs each slot target (spec §7.1).
export function pendingDelta(db: DatabaseSync, deckId: number) {
  const items = db
    .prepare(
      `SELECT pi.action, pi.slot_id, dc.slot_id AS current_slot_id
       FROM proposal_items pi
       JOIN proposals p ON p.id = pi.proposal_id
       LEFT JOIN deck_cards dc ON dc.deck_id = p.deck_id AND dc.oracle_id = pi.oracle_id
       WHERE p.deck_id = ? AND pi.status = 'pending'`,
    )
    .all(deckId) as unknown as Array<{
    action: "add" | "cut";
    slot_id: number | null;
    current_slot_id: number | null;
  }>;
  const adds = items.filter((i) => i.action === "add").length;
  const cuts = items.filter((i) => i.action === "cut").length;
  const bySlot: Record<number, number> = {};
  for (const i of items) {
    const slot = i.action === "add" ? i.slot_id : i.current_slot_id;
    if (slot != null) bySlot[slot] = (bySlot[slot] ?? 0) + (i.action === "add" ? 1 : -1);
  }
  return { pending_adds: adds, pending_cuts: cuts, pending_by_slot: bySlot };
}

// ---------- ruling ----------

interface ItemRow {
  id: number;
  proposal_id: number;
  deck_id: number;
  action: "add" | "cut";
  oracle_id: string;
  slot_id: number | null;
  rationale: string;
  group_id: string | null;
  status: string;
}

function getItem(db: DatabaseSync, itemId: number): ItemRow {
  const row = db
    .prepare(
      `SELECT pi.*, p.deck_id FROM proposal_items pi JOIN proposals p ON p.id = pi.proposal_id
       WHERE pi.id = ?`,
    )
    .get(itemId) as ItemRow | undefined;
  if (!row) throw new ServiceError(`Proposal item ${itemId} not found`, 404);
  return row;
}

// group_id marks atomic bundles: ruling on one item rules the whole group.
function groupOf(db: DatabaseSync, item: ItemRow): ItemRow[] {
  if (!item.group_id) return [item];
  return db
    .prepare(
      `SELECT pi.*, p.deck_id FROM proposal_items pi JOIN proposals p ON p.id = pi.proposal_id
       WHERE pi.proposal_id = ? AND pi.group_id = ?`,
    )
    .all(item.proposal_id, item.group_id) as unknown as ItemRow[];
}

function snapshotCard(db: DatabaseSync, deckId: number, oracleId: string) {
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

function logEntry(
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

function resolveProposalIfDone(db: DatabaseSync, proposalId: number) {
  const { n } = db
    .prepare("SELECT COUNT(*) n FROM proposal_items WHERE proposal_id = ? AND status = 'pending'")
    .get(proposalId) as { n: number };
  if (n === 0)
    db.prepare("UPDATE proposals SET status = 'resolved' WHERE id = ?").run(proposalId);
}

function applyAccept(db: DatabaseSync, item: ItemRow): void {
  const name = cardName(db, item.oracle_id);
  if (item.action === "add") {
    if (snapshotCard(db, item.deck_id, item.oracle_id))
      throw new ServiceError(`${name} is already in the deck; cannot accept the add`);
    // If the target slot was deleted since proposing, add unslotted.
    let slotId = item.slot_id;
    if (
      slotId != null &&
      !db.prepare("SELECT 1 FROM slots WHERE id = ? AND deck_id = ?").get(slotId, item.deck_id)
    )
      slotId = null;
    addCard(db, item.deck_id, item.oracle_id, { slotId });
    const revision = currentRevision(db, item.deck_id);
    logEntry(
      db,
      item.deck_id,
      {
        kind: "accept",
        action: "add",
        oracle_id: item.oracle_id,
        card_name: name,
        rationale: item.rationale,
        proposal_id: item.proposal_id,
        item_id: item.id,
      },
      revision,
    );
  } else {
    const snapshot = snapshotCard(db, item.deck_id, item.oracle_id);
    if (!snapshot) throw new ServiceError(`${name} is no longer in the deck; cannot accept the cut`);
    removeCard(db, item.deck_id, item.oracle_id);
    const revision = currentRevision(db, item.deck_id);
    logEntry(
      db,
      item.deck_id,
      {
        kind: "accept",
        action: "cut",
        oracle_id: item.oracle_id,
        card_name: name,
        rationale: item.rationale,
        proposal_id: item.proposal_id,
        item_id: item.id,
        snapshot_json: JSON.stringify(snapshot),
      },
      revision,
    );
  }
  db.prepare(
    "UPDATE proposal_items SET status = 'accepted', resolved_at = datetime('now') WHERE id = ?",
  ).run(item.id);
}

export function acceptItem(db: DatabaseSync, itemId: number): void {
  const item = getItem(db, itemId);
  const group = groupOf(db, item);
  for (const g of group)
    if (g.status !== "pending")
      throw new ServiceError(
        `Item ${g.id} in this group is already ${g.status}; the group must be ruled on as a unit`,
      );
  db.exec("BEGIN");
  try {
    for (const g of group) applyAccept(db, g);
    resolveProposalIfDone(db, item.proposal_id);
    db.exec("COMMIT");
  } catch (e) {
    db.exec("ROLLBACK");
    throw e;
  }
}

// Rejecting requires a typed reason (spec §7.2); the type routes it.
export function rejectItem(
  db: DatabaseSync,
  itemId: number,
  type: RejectionType,
  reason: string,
): void {
  if (!REJECTION_TYPES.includes(type))
    throw new ServiceError(`Rejection type must be one of: ${REJECTION_TYPES.join(", ")}`);
  if (!reason?.trim()) throw new ServiceError("A rejection requires a reason — the reasons are the point");

  const item = getItem(db, itemId);
  const group = groupOf(db, item);
  for (const g of group)
    if (g.status !== "pending")
      throw new ServiceError(
        `Item ${g.id} in this group is already ${g.status}; the group must be ruled on as a unit`,
      );

  db.exec("BEGIN");
  try {
    const revision = currentRevision(db, item.deck_id); // rejections don't change the deck
    for (const g of group) {
      const name = cardName(db, g.oracle_id);
      db.prepare(
        "UPDATE proposal_items SET status = 'rejected', resolved_at = datetime('now') WHERE id = ?",
      ).run(g.id);
      logEntry(
        db,
        g.deck_id,
        {
          kind: "reject",
          action: g.action,
          oracle_id: g.oracle_id,
          card_name: name,
          rationale: g.rationale,
          rejection_type: type,
          rejection_reason: reason.trim(),
          proposal_id: g.proposal_id,
          item_id: g.id,
          brief_flag: type === "thesis_change" ? 1 : 0,
        },
        revision,
      );
      if (type === "hard_filter" && g.action === "add") {
        db.prepare(
          "INSERT OR IGNORE INTO hard_filters (deck_id, oracle_id, card_name, reason) VALUES (?, ?, ?, ?)",
        ).run(g.deck_id, g.oracle_id, name, reason.trim());
      }
      if (type === "playtest_finding") {
        db.prepare(
          "INSERT INTO card_notes (deck_id, oracle_id, card_name, note) VALUES (?, ?, ?, ?)",
        ).run(g.deck_id, g.oracle_id, name, reason.trim());
      }
    }
    resolveProposalIfDone(db, item.proposal_id);
    db.exec("COMMIT");
  } catch (e) {
    db.exec("ROLLBACK");
    throw e;
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

  db.exec("BEGIN");
  try {
    let snapshotJson: string | null = null;
    if (entry.action === "add") {
      const snapshot = snapshotCard(db, deckId, entry.oracle_id!);
      if (!snapshot)
        throw new ServiceError(`${entry.card_name} is no longer in the deck; nothing to undo`);
      snapshotJson = JSON.stringify(snapshot);
      removeCard(db, deckId, entry.oracle_id!);
    } else {
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
    }
    const revision = currentRevision(db, deckId);
    const undoId = logEntry(
      db,
      deckId,
      {
        kind: "undo",
        action: entry.action === "add" ? "cut" : "add",
        oracle_id: entry.oracle_id,
        card_name: entry.card_name,
        rationale: `Undo of decision #${entry.id}`,
        undo_of: entry.id,
        snapshot_json: snapshotJson,
      },
      revision,
    );
    db.prepare("UPDATE decision_log SET undone_by = ? WHERE id = ?").run(undoId, entry.id);
    db.exec("COMMIT");
  } catch (e) {
    db.exec("ROLLBACK");
    throw e;
  }
}

// ---------- log, filters, notes ----------

export function getLog(db: DatabaseSync, deckId: number, limit = 50) {
  return db
    .prepare("SELECT * FROM decision_log WHERE deck_id = ? ORDER BY id DESC LIMIT ?")
    .all(deckId, limit);
}

export function getCardHistory(db: DatabaseSync, deckId: number, oracleId: string) {
  return db
    .prepare("SELECT * FROM decision_log WHERE deck_id = ? AND oracle_id = ? ORDER BY id DESC")
    .all(deckId, oracleId);
}

export function listHardFilters(db: DatabaseSync, deckId: number) {
  return db
    .prepare("SELECT oracle_id, card_name, reason, created_at FROM hard_filters WHERE deck_id = ? ORDER BY card_name")
    .all(deckId);
}

export function removeHardFilter(db: DatabaseSync, deckId: number, oracleId: string): void {
  const row = db
    .prepare("SELECT card_name FROM hard_filters WHERE deck_id = ? AND oracle_id = ?")
    .get(deckId, oracleId) as { card_name: string } | undefined;
  if (!row) throw new ServiceError("No such hard filter", 404);
  db.exec("BEGIN");
  try {
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
      currentRevision(db, deckId),
    );
    db.exec("COMMIT");
  } catch (e) {
    db.exec("ROLLBACK");
    throw e;
  }
}

export function listCardNotes(db: DatabaseSync, deckId: number) {
  return db
    .prepare("SELECT id, oracle_id, card_name, note, created_at FROM card_notes WHERE deck_id = ? ORDER BY id DESC")
    .all(deckId);
}

export function addCardNote(db: DatabaseSync, deckId: number, oracleId: string, note: string): number {
  if (!note?.trim()) throw new ServiceError("Note cannot be empty");
  const name = cardName(db, oracleId);
  const r = db
    .prepare("INSERT INTO card_notes (deck_id, oracle_id, card_name, note) VALUES (?, ?, ?, ?)")
    .run(deckId, oracleId, name, note.trim());
  return Number(r.lastInsertRowid);
}
