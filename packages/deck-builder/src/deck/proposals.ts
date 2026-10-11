// Proposals and rulings (spec §8). On an active deck an agent changes
// membership only by proposing; the owner rules per item, per group, or on
// the whole proposal, with an optional free-text reason. There are no reason
// types and no cap on items.

import type { DatabaseSync } from "node:sqlite";
import { ServiceError } from "../errors.ts";
import { withTransaction } from "../db.ts";
import { type Actor, authorOf, requireOwner } from "../actor.ts";
import { lintProse, requireCardById } from "../cards.ts";
import { type LogCardInput, writeLog } from "./log.ts";
import { type Change, applyChanges } from "./membership.ts";
import { copyLimit, findDeckCard, requireDeck } from "./service.ts";

export interface ProposalItemInput {
  action: "add" | "cut";
  oracle_id: string;
  /** For an add: the slot to file the card in. Optional; slots never are required. */
  slot_id?: number | null;
  /** For a cut: if accepted, the card goes to the maybe list instead of out of the deck. */
  to_maybe?: boolean;
  reason: string;
  /** Items sharing a group are ruled on as a unit (a swap with one argument). */
  group?: string | null;
}

// ---------- creation ----------

export function createProposal(
  db: DatabaseSync,
  actor: Actor,
  deckId: number,
  input: { summary?: string; items: ProposalItemInput[] },
): number {
  requireDeck(db, deckId);
  const items = input.items ?? [];
  if (!items.length) throw new ServiceError("A proposal needs at least one item");

  const seen = new Set<string>();
  for (const item of items) {
    const card = requireCardById(db, item.oracle_id);
    if (item.action !== "add" && item.action !== "cut")
      throw new ServiceError(`An item's action is 'add' or 'cut' (got '${item.action}')`);
    if (!item.reason?.trim())
      throw new ServiceError(`The ${item.action} of ${card.name} needs a reason`);
    lintProse(db, item.reason);

    const key = `${item.action}:${item.oracle_id}`;
    if (seen.has(key))
      throw new ServiceError(`${card.name} appears twice as '${item.action}' in this proposal`);
    seen.add(key);

    const row = findDeckCard(db, deckId, item.oracle_id);
    const inDeck = !!row && !row.maybe;
    if (item.action === "cut" && !inDeck)
      throw new ServiceError(`${card.name} is not in the deck, so it cannot be proposed as a cut`);
    if (item.action === "add" && inDeck && row!.quantity >= copyLimit(card))
      throw new ServiceError(
        `${card.name} is already in the deck, and a Commander deck holds one copy of it`,
      );
    if (item.to_maybe) {
      if (item.action !== "cut")
        throw new ServiceError(
          `Only a cut can move a card to the maybe list; to shortlist ${card.name} without adding it, use the maybe list directly`,
        );
      // The maybe list holds a card once, so parking takes every copy out.
      if (row!.quantity > 1)
        throw new ServiceError(
          `${card.name} has ${row!.quantity} copies in the deck, and moving it to the maybe list would take all of them out. ` +
            `Propose the cut without to_maybe.`,
        );
    }
    if (item.slot_id != null) {
      if (item.action !== "add")
        throw new ServiceError("Only an add can name a slot; a cut leaves the card's slot as it is");
      if (!db.prepare("SELECT 1 FROM slots WHERE id = ? AND deck_id = ?").get(item.slot_id, deckId))
        throw new ServiceError(`Slot ${item.slot_id} not found in deck ${deckId}`, 404);
    }

    const pending = db
      .prepare(
        `SELECT p.id FROM proposal_items pi JOIN proposals p ON p.id = pi.proposal_id
         WHERE p.deck_id = ? AND pi.oracle_id = ? AND pi.action = ? AND pi.status = 'pending'`,
      )
      .get(deckId, item.oracle_id, item.action) as { id: number } | undefined;
    if (pending)
      throw new ServiceError(
        `Proposal #${pending.id} already has a pending ${item.action} of ${card.name}. ` +
          `Wait for the owner's ruling on it.`,
      );
  }
  if (input.summary) lintProse(db, input.summary);

  return withTransaction(db, () => {
    const r = db
      .prepare("INSERT INTO proposals (deck_id, summary, author) VALUES (?, ?, ?)")
      .run(deckId, (input.summary ?? "").trim(), authorOf(actor));
    const proposalId = Number(r.lastInsertRowid);
    const ins = db.prepare(
      `INSERT INTO proposal_items (proposal_id, action, oracle_id, slot_id, to_maybe, reason, group_id)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
    );
    for (const item of items)
      ins.run(
        proposalId,
        item.action,
        item.oracle_id,
        item.slot_id ?? null,
        item.to_maybe ? 1 : 0,
        item.reason.trim(),
        item.group?.trim() || null,
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
  power: string | null;
  toughness: string | null;
  loyalty: string | null;
  usd_cents: number | null;
  slot_id: number | null;
  slot_name: string | null;
  /** 1 on a cut that, if accepted, moves the card to the maybe list. */
  to_maybe: number;
  reason: string;
  group_id: string | null;
  status: "pending" | "accepted" | "rejected";
}

export interface ProposalView {
  id: number;
  deck_id: number;
  deck_name: string;
  summary: string;
  author: string;
  status: "open" | "resolved";
  created_at: string;
  items: ProposalItemView[];
}

const ITEM_SELECT = `SELECT pi.id, pi.proposal_id, pi.action, pi.oracle_id, c.name AS card_name,
       c.mana_cost, c.type_line, c.oracle_text, c.power, c.toughness, c.loyalty, pr.usd_cents,
       pi.slot_id, s.name AS slot_name, pi.to_maybe, pi.reason, pi.group_id, pi.status
  FROM proposal_items pi JOIN cards c ON c.oracle_id = pi.oracle_id
  LEFT JOIN card_prices pr ON pr.oracle_id = pi.oracle_id
  LEFT JOIN slots s ON s.id = pi.slot_id
  WHERE pi.proposal_id = ? ORDER BY pi.id`;

const PROPOSAL_SELECT = `SELECT p.id, p.deck_id, d.name AS deck_name, p.summary, p.author, p.status, p.created_at
  FROM proposals p JOIN decks d ON d.id = p.deck_id`;

function withItems(db: DatabaseSync, rows: Array<Omit<ProposalView, "items">>): ProposalView[] {
  const stmt = db.prepare(ITEM_SELECT);
  return rows.map((p) => ({ ...p, items: stmt.all(p.id) as unknown as ProposalItemView[] }));
}

/** Proposals for one deck, or across every deck when deckId is omitted (the inbox). */
export function listProposals(
  db: DatabaseSync,
  opts: { deckId?: number; status?: "open" | "resolved" } = {},
): ProposalView[] {
  const where: string[] = [];
  const params: Array<string | number> = [];
  if (opts.deckId !== undefined) {
    where.push("p.deck_id = ?");
    params.push(opts.deckId);
  }
  if (opts.status) {
    where.push("p.status = ?");
    params.push(opts.status);
  }
  const rows = db
    .prepare(
      `${PROPOSAL_SELECT} ${where.length ? `WHERE ${where.join(" AND ")}` : ""} ORDER BY p.id DESC LIMIT 200`,
    )
    .all(...params) as unknown as Array<Omit<ProposalView, "items">>;
  return withItems(db, rows);
}

export function getProposal(db: DatabaseSync, proposalId: number): ProposalView | null {
  const row = db.prepare(`${PROPOSAL_SELECT} WHERE p.id = ?`).get(proposalId) as
    | Omit<ProposalView, "items">
    | undefined;
  return row ? withItems(db, [row])[0] : null;
}

// ---------- ruling ----------

interface ItemRow {
  id: number;
  proposal_id: number;
  deck_id: number;
  action: "add" | "cut";
  oracle_id: string;
  slot_id: number | null;
  to_maybe: number;
  reason: string;
  group_id: string | null;
  status: string;
}

const ITEM_ROW = `SELECT pi.id, pi.proposal_id, p.deck_id, pi.action, pi.oracle_id, pi.slot_id,
       pi.to_maybe, pi.reason, pi.group_id, pi.status
  FROM proposal_items pi JOIN proposals p ON p.id = pi.proposal_id`;

// The items a ruling covers: the ones named, widened to whole groups — a
// group is ruled on as a unit so a swap cannot be half-accepted (spec §8.1).
function itemsToRule(db: DatabaseSync, itemIds: number[]): ItemRow[] {
  if (!itemIds.length) throw new ServiceError("Name at least one item to rule on");
  const byId = new Map<number, ItemRow>();
  for (const id of itemIds) {
    const item = db.prepare(`${ITEM_ROW} WHERE pi.id = ?`).get(id) as ItemRow | undefined;
    if (!item) throw new ServiceError(`Proposal item ${id} not found`, 404);
    const group = item.group_id
      ? (db
          .prepare(`${ITEM_ROW} WHERE pi.proposal_id = ? AND pi.group_id = ?`)
          .all(item.proposal_id, item.group_id) as unknown as ItemRow[])
      : [item];
    for (const g of group) byId.set(g.id, g);
  }
  const items = [...byId.values()].sort((a, b) => a.id - b.id);
  if (new Set(items.map((i) => i.proposal_id)).size > 1)
    throw new ServiceError("Rule on one proposal at a time");
  for (const i of items)
    if (i.status !== "pending")
      throw new ServiceError(
        `Item ${i.id} is already ${i.status}` +
          (i.group_id ? "; its group must be ruled on as a unit" : ""),
      );
  return items;
}

function resolveProposalIfDone(db: DatabaseSync, proposalId: number) {
  const { n } = db
    .prepare("SELECT COUNT(*) n FROM proposal_items WHERE proposal_id = ? AND status = 'pending'")
    .get(proposalId) as { n: number };
  if (n === 0)
    db.prepare("UPDATE proposals SET status = 'resolved' WHERE id = ?").run(proposalId);
}

function markItems(db: DatabaseSync, items: ItemRow[], status: "accepted" | "rejected") {
  const stmt = db.prepare(
    "UPDATE proposal_items SET status = ?, resolved_at = datetime('now') WHERE id = ?",
  );
  for (const i of items) stmt.run(status, i.id);
}

/** Accept items: the changes are applied and logged as one act. Returns the log id. */
export function acceptItems(
  db: DatabaseSync,
  actor: Actor,
  itemIds: number[],
  reason = "",
): number {
  requireOwner(actor, "rule on a proposal");
  const items = itemsToRule(db, itemIds);
  const deckId = items[0].deck_id;

  return withTransaction(db, () => {
    // Cuts first: a swap that replaces a commander needs the seat free.
    const changes: Change[] = [...items]
      .sort((a, b) => Number(a.action === "add") - Number(b.action === "add"))
      .map((i) =>
        i.action === "add"
          ? {
              action: "add" as const,
              oracle_id: i.oracle_id,
              // A slot deleted since the proposal was made must not fail the accept.
              slot_id:
                i.slot_id != null &&
                db.prepare("SELECT 1 FROM slots WHERE id = ? AND deck_id = ?").get(i.slot_id, deckId)
                  ? i.slot_id
                  : undefined,
              item_id: i.id,
              reason: i.reason,
            }
          : { action: "cut" as const, oracle_id: i.oracle_id, park: !!i.to_maybe, item_id: i.id, reason: i.reason },
      );
    const logId = applyChanges(db, actor, deckId, changes, {
      kind: "accept",
      ruling_reason: reason,
      proposal_id: items[0].proposal_id,
    });
    markItems(db, items, "accepted");
    resolveProposalIfDone(db, items[0].proposal_id);
    return logId!;
  });
}

/** Reject items. Nothing changes; the ruling and its optional reason are logged. */
export function rejectItems(
  db: DatabaseSync,
  actor: Actor,
  itemIds: number[],
  reason = "",
): number {
  requireOwner(actor, "rule on a proposal");
  const items = itemsToRule(db, itemIds);
  const deckId = items[0].deck_id;

  return withTransaction(db, () => {
    const cards: LogCardInput[] = items.map((i) => ({
      oracle_id: i.oracle_id,
      card_name: requireCardById(db, i.oracle_id).name,
      action: i.action,
      item_id: i.id,
      reason: i.reason,
    }));
    const logId = writeLog(
      db,
      actor,
      {
        deck_id: deckId,
        kind: "reject",
        summary:
          cards.length === 1
            ? `Rejected ${cards[0].action === "add" ? "adding" : "cutting"} ${cards[0].card_name}`
            : `Rejected ${cards.length} proposed changes`,
        ruling_reason: reason,
        proposal_id: items[0].proposal_id,
      },
      cards,
    );
    markItems(db, items, "rejected");
    resolveProposalIfDone(db, items[0].proposal_id);
    return logId;
  });
}

/** Every item of a proposal that is still pending. */
export function pendingItemIds(db: DatabaseSync, proposalId: number): number[] {
  return (
    db
      .prepare("SELECT id FROM proposal_items WHERE proposal_id = ? AND status = 'pending' ORDER BY id")
      .all(proposalId) as unknown as { id: number }[]
  ).map((r) => r.id);
}
