// The approval gate (spec §7): the agent proposes, the owner rules. Creation
// and ruling live here; the decision log itself — every ruling's paper trail,
// rejection routing, and undo — is deck/log.ts.

import type { DatabaseSync } from "node:sqlite";
import { ServiceError } from "../errors.ts";
import { withTransaction } from "../db.ts";
import { addCard, removeCard, requireCard, requireDeck } from "./service.ts";
import {
  REJECTION_TYPES,
  type RejectionType,
  isHardFiltered,
  logEntry,
  recordRejection,
  snapshotCard,
} from "./log.ts";

export type ProposalSource = "manual" | "agent" | "audit" | "import";

export interface ProposalItemInput {
  action: "add" | "cut";
  oracle_id: string;
  slot_id?: number | null;
  rationale: string;
  group_id?: string | null;
}

// The 3–5 cap forces the agent to rank (spec §7.1). Nothing is exempt any
// more: imports used to be, but they apply directly now (§9) instead of
// arriving here as a 99-item proposal. "import" stays in ProposalSource so
// rows written before that change still read back.
export const MAX_ITEMS = 5;

// ---------- creation ----------

export function createProposal(
  db: DatabaseSync,
  deckId: number,
  items: ProposalItemInput[],
  opts: { source?: ProposalSource; note?: string } = {},
): number {
  requireDeck(db, deckId);
  const source = opts.source ?? "manual";
  if (!items.length) throw new ServiceError("A proposal needs at least one item");
  if (items.length > MAX_ITEMS)
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
    const name = requireCard(db, item.oracle_id);
    if (item.action === "cut" && !inDeck.has(item.oracle_id))
      throw new ServiceError(`Cannot propose cutting ${name} — it is not in the deck`);
    if (item.action === "add" && inDeck.has(item.oracle_id))
      throw new ServiceError(`Cannot propose adding ${name} — it is already in the deck`);
    if (item.action === "add" && isHardFiltered(db, deckId, item.oracle_id))
      throw new ServiceError(
        `${name} is hard-filtered for this deck (remove the filter first to re-propose it)`,
      );
  }

  return withTransaction(db, () => {
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
    return proposalId;
  });
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

const ITEM_SELECT = `SELECT pi.id, pi.proposal_id, pi.action, pi.oracle_id, c.name AS card_name,
       c.mana_cost, c.type_line, c.oracle_text,
       pi.slot_id, pi.rationale, pi.group_id, pi.status
  FROM proposal_items pi JOIN cards c ON c.oracle_id = pi.oracle_id
  WHERE pi.proposal_id = ? ORDER BY pi.id`;

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
  const itemStmt = db.prepare(ITEM_SELECT);
  for (const p of proposals) p.items = itemStmt.all(p.id) as unknown as ProposalItemView[];
  return proposals;
}

// One proposal by id, open or resolved. The chat transcript reads by id and
// wants proposals it has already outlived — a turn's proposal stays in the
// conversation after it is ruled on, showing what became of it.
export function getProposal(db: DatabaseSync, proposalId: number) {
  const p = db
    .prepare("SELECT id, source, note, status, created_at FROM proposals WHERE id = ?")
    .get(proposalId) as unknown as
    | { id: number; source: ProposalSource; note: string; status: string; created_at: string }
    | undefined;
  if (!p) return null;
  const items = db.prepare(ITEM_SELECT).all(proposalId) as unknown as ProposalItemView[];
  return { ...p, items };
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

function resolveProposalIfDone(db: DatabaseSync, proposalId: number) {
  const { n } = db
    .prepare("SELECT COUNT(*) n FROM proposal_items WHERE proposal_id = ? AND status = 'pending'")
    .get(proposalId) as { n: number };
  if (n === 0)
    db.prepare("UPDATE proposals SET status = 'resolved' WHERE id = ?").run(proposalId);
}

function applyAccept(db: DatabaseSync, item: ItemRow): void {
  const name = requireCard(db, item.oracle_id);
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
      requireDeck(db, item.deck_id).revision,
    );
  } else {
    const snapshot = snapshotCard(db, item.deck_id, item.oracle_id);
    if (!snapshot) throw new ServiceError(`${name} is no longer in the deck; cannot accept the cut`);
    removeCard(db, item.deck_id, item.oracle_id);
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
      requireDeck(db, item.deck_id).revision,
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
  withTransaction(db, () => {
    for (const g of group) applyAccept(db, g);
    resolveProposalIfDone(db, item.proposal_id);
  });
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

  withTransaction(db, () => {
    const revision = requireDeck(db, item.deck_id).revision; // rejections don't change the deck
    for (const g of group) {
      const name = requireCard(db, g.oracle_id);
      db.prepare(
        "UPDATE proposal_items SET status = 'rejected', resolved_at = datetime('now') WHERE id = ?",
      ).run(g.id);
      recordRejection(
        db,
        g.deck_id,
        {
          type,
          reason: reason.trim(),
          rationale: g.rationale,
          action: g.action,
          oracle_id: g.oracle_id,
          card_name: name,
          proposal_id: g.proposal_id,
          item_id: g.id,
        },
        revision,
      );
    }
    resolveProposalIfDone(db, item.proposal_id);
  });
}
