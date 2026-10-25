import { displayAuthor } from "../actor.ts";
import { requireDeckOwner } from "../actor.ts";
// Proposals and rulings (spec §8). On a Paper deck an agent changes
// membership only by proposing; the owner rules per item, per group, or on
// the whole proposal, with an optional free-text reason. There are no reason
// types and no cap on items.

import type { Database } from "../sqlite.ts";
import { ServiceError } from "../errors.ts";
import { withTransaction } from "../db.ts";
import { type Actor, authorOf, isAuthor, requireUi, PermissionError } from "../actor.ts";
import { lintProse, requireCardById } from "../cards.ts";
import { recordActivity } from "../activity.ts";
import { type LogCardInput, writeLog } from "./log.ts";
import { type Change, applyChanges } from "./membership.ts";
import { copyLimit, findDeckCard, requireDeck, requireEditableDeck, requireRevision, listSlots, validateSlotAssignments } from "./service.ts";

export interface ProposalItemInput {
  action: "add" | "cut";
  oracle_id: string;
  /** For an add: the slot to file the card in. Optional; slots never are required. */
  slot_id?: number | null;
  /** Additional purposes for an add. Omit to retain a maybe card's purposes. */
  secondary_slot_ids?: number[];
  /** For a cut: if accepted, the card goes to the maybe list instead of out of the deck. */
  to_maybe?: boolean;
  reason: string;
  /** Items sharing a group are ruled on as a unit (a swap with one argument). */
  group?: string | null;
}

// ---------- creation ----------

export function createProposal(
  db: Database,
  actor: Actor,
  deckId: number,
  input: { summary?: string; items: ProposalItemInput[]; expected_revision?: number },
): number {
  requireEditableDeck(db, actor, deckId);
  requireRevision(db, actor, deckId, input.expected_revision);
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

    const row = findDeckCard(db, actor, deckId, item.oracle_id);
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
    if (item.secondary_slot_ids !== undefined && item.action !== "add")
      throw new ServiceError("Only an add can name secondary slots; a cut keeps the card's purposes");
    if (item.action === "add") {
      const primary = item.slot_id ?? (row?.maybe ? row.slot_id : null);
      validateSlotAssignments(db, actor, deckId, primary, item.secondary_slot_ids !== undefined ? item.secondary_slot_ids : (row?.maybe ? row.secondary_slot_ids.filter(id => id !== primary) : []));
    }

    const pending = db
      .prepare(
        `SELECT p.id FROM proposal_items pi JOIN proposals p ON p.id = pi.proposal_id
         WHERE p.deck_id = ? AND pi.oracle_id = ? AND pi.action = ? AND pi.status = 'pending' AND p.status = 'open'`,
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
      .prepare("INSERT INTO proposals (deck_id, summary, author, base_revision, author_user_id, author_via, author_label, author_token_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
      .run(deckId, (input.summary ?? "").trim(), authorOf(actor), requireDeck(db, actor, deckId).revision, actor.user_id, actor.via, actor.label, actor.token_id ?? null);
    const proposalId = Number(r.lastInsertRowid);
    const ins = db.prepare(
      `INSERT INTO proposal_items (proposal_id, action, oracle_id, slot_id, secondary_slot_ids_json, to_maybe, reason, group_id)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
    );
    for (const item of items)
      ins.run(
        proposalId,
        item.action,
        item.oracle_id,
        item.slot_id ?? null,
        item.secondary_slot_ids === undefined ? null : JSON.stringify([...new Set(item.secondary_slot_ids)]),
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
  image_small: string | null;
  image_normal: string | null;
  image_art_crop: string | null;
  slot_id: number | null;
  slot_name: string | null;
  secondary_slot_ids: number[];
  secondary_slot_names: string[];
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
  author_user_id: number;
  author_via: "ui" | "agent";
  author_label: string;
  author_display?: string;
  author_token_id: number | null;
  status: "open" | "resolved";
  created_at: string;
  items: ProposalItemView[];
  base_revision: number | null;
  current_revision: number;
  stale: boolean;
  disposition: "current" | "withdrawn" | "superseded";
  superseded_by: number | null;
  supersedes: number[];
  is_current: boolean;
}

const ITEM_SELECT = `SELECT pi.id, pi.proposal_id, pi.action, pi.oracle_id, c.name AS card_name,
       c.mana_cost, c.type_line, c.oracle_text, c.power, c.toughness, c.loyalty, pr.usd_cents, c.image_small, c.image_normal, c.image_art_crop,
       pi.slot_id, s.name AS slot_name, pi.secondary_slot_ids_json, pi.to_maybe, pi.reason, pi.group_id, pi.status
  FROM proposal_items pi JOIN cards c ON c.oracle_id = pi.oracle_id
  LEFT JOIN card_prices pr ON pr.oracle_id = pi.oracle_id
  LEFT JOIN slots s ON s.id = pi.slot_id
  WHERE pi.proposal_id = ? ORDER BY pi.id`;

const PROPOSAL_SELECT = `SELECT p.id, p.deck_id, d.name AS deck_name, p.summary, p.author, p.author_user_id, p.author_via, p.author_label, p.author_token_id, p.status, p.created_at, p.base_revision, p.disposition, p.superseded_by
  FROM proposals p JOIN decks d ON d.id = p.deck_id`;

function withItems(db: Database, actor: Actor, rows: Array<Omit<ProposalView, "items">>): ProposalView[] {
  const stmt = db.prepare(ITEM_SELECT);
  return rows.map(p => {
    const slots = new Map(listSlots(db, actor, p.deck_id).map(s => [s.id, s.name]));
    const items = (stmt.all(p.id) as unknown as (ProposalItemView & { secondary_slot_ids_json: string | null })[]).map(raw => {
      const { secondary_slot_ids_json: json, ...item } = raw;
      const current = findDeckCard(db, actor, p.deck_id, item.oracle_id);
      const proposedPrimary = item.slot_id != null && slots.has(item.slot_id) ? item.slot_id : null;
      const primary = item.action === "add" && item.status === "pending"
        ? (current && !current.maybe ? current.slot_id : proposedPrimary ?? (current?.maybe ? current.slot_id : null))
        : proposedPrimary;
      const ids: number[] = json == null ? (item.action === "add" && current?.maybe ? current.secondary_slot_ids : []) : JSON.parse(json);
      const secondary_slot_ids = ids.filter(id => slots.has(id) && id !== primary);
      return { ...item, slot_id: primary, slot_name: primary == null ? null : slots.get(primary)!, secondary_slot_ids, secondary_slot_names: secondary_slot_ids.map(id => slots.get(id)!) };
    });
    const currentRevision = requireDeck(db, actor, p.deck_id).revision;
    return { ...p, author_display: displayAuthor(db, p), items, current_revision: currentRevision, stale: p.base_revision !== currentRevision,
      is_current: p.status === "open" && p.disposition === "current",
      supersedes: (db.prepare("SELECT p.id FROM proposals p JOIN decks d ON d.id = p.deck_id WHERE p.superseded_by = ? AND d.user_id = ? ORDER BY p.id").all(p.id, actor.user_id) as unknown as { id: number }[]).map(r => r.id) };
  });
}

/** Proposals for one deck, or across every deck when deckId is omitted (the inbox). */
export function listProposals(
  db: Database, actor: Actor,
  opts: { deckId?: number; status?: "open" | "resolved" } = {},
): ProposalView[] {
  if (opts.deckId !== undefined) requireDeck(db, actor, opts.deckId);
  const where: string[] = ["d.user_id = ?"];
  const params: Array<string | number> = [actor.user_id];
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
  return withItems(db, actor, rows);
}

export function getProposal(db: Database, actor: Actor, proposalId: number, forWrite = false): ProposalView | null {
  const row = db.prepare(`${PROPOSAL_SELECT} WHERE p.id = ? AND d.user_id = ?`).get(proposalId, actor.user_id) as
    | Omit<ProposalView, "items">
    | undefined;
  if (!row) throw new ServiceError(`Proposal ${proposalId} not found`, 404);
  if (forWrite) requireDeckOwner(db, actor, row.deck_id);
  return withItems(db, actor, [row])[0];
}

// ---------- ruling ----------

interface ItemRow {
  id: number;
  proposal_id: number;
  deck_id: number;
  action: "add" | "cut";
  oracle_id: string;
  slot_id: number | null;
  secondary_slot_ids_json: string | null;
  to_maybe: number;
  reason: string;
  group_id: string | null;
  status: string;
}

const ITEM_ROW = `SELECT pi.id, pi.proposal_id, p.deck_id, pi.action, pi.oracle_id, pi.slot_id,
       pi.secondary_slot_ids_json, pi.to_maybe, pi.reason, pi.group_id, pi.status
  FROM proposal_items pi JOIN proposals p ON p.id = pi.proposal_id`;

// The items a ruling covers: the ones named, widened to whole groups — a
// group is ruled on as a unit so a swap cannot be half-accepted (spec §8.1).
function itemsToRule(db: Database, actor: Actor, itemIds: number[]): ItemRow[] {
  if (!itemIds.length) throw new ServiceError("Name at least one item to rule on");
  const byId = new Map<number, ItemRow>();
  for (const id of itemIds) {
    const item = db.prepare(`${ITEM_ROW} WHERE pi.id = ?`).get(id) as ItemRow | undefined;
    if (!item) throw new ServiceError(`Proposal item ${id} not found`, 404);
    requireDeckOwner(db, actor, item.deck_id, "rule on this proposal");
    const proposal = getProposal(db, actor, item.proposal_id)!;
    if (!proposal.is_current) throw new ServiceError(`Proposal #${proposal.id} is ${proposal.disposition === "current" ? proposal.status : proposal.disposition}`);
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

function resolveProposalIfDone(db: Database, actor: Actor, proposalId: number) {
  const { n } = db
    .prepare("SELECT COUNT(*) n FROM proposal_items WHERE proposal_id = ? AND status = 'pending'")
    .get(proposalId) as { n: number };
  if (n === 0)
    db.prepare("UPDATE proposals SET status = 'resolved' WHERE id = ?").run(proposalId);
}

function markItems(db: Database, actor: Actor, items: ItemRow[], status: "accepted" | "rejected") {
  const stmt = db.prepare(
    "UPDATE proposal_items SET status = ?, resolved_at = datetime('now') WHERE id = ? AND proposal_id = ?",
  );
  for (const i of items) stmt.run(status, i.id, i.proposal_id);
}

/** Accept items: the changes are applied and logged as one act. Returns the log id. */
export function acceptItems(
  db: Database,
  actor: Actor,
  itemIds: number[],
  reason = "",
  options: { expected_revision?: number; acknowledge_stale?: boolean } = {},
): number {
  const items = itemsToRule(db, actor, itemIds);
  requireUi(actor, "rule on a proposal");
  const deckId = items[0].deck_id;
  requireEditableDeck(db, actor, deckId);
  requireRevision(db, actor, deckId, options.expected_revision);
  if (getProposal(db, actor, items[0].proposal_id)!.stale && (options.acknowledge_stale !== true || options.expected_revision === undefined))
    throw new ServiceError("Proposal is stale; review changes and acknowledge_stale with expected_revision, or amend it", 409);

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
              secondary_slot_ids: i.secondary_slot_ids_json == null ? undefined : (JSON.parse(i.secondary_slot_ids_json) as number[])
                .filter(id => id !== i.slot_id && db.prepare("SELECT 1 FROM slots WHERE id = ? AND deck_id = ?").get(id, deckId)),
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
    markItems(db, actor, items, "accepted");
    resolveProposalIfDone(db, actor, items[0].proposal_id);
    return logId!;
  });
}

/** Validate the click's entire revision/acknowledgement snapshot before any
 * membership changes. Subsequent revisions are produced only by this batch. */
export function acceptProposals(
  db: Database, actor: Actor, deckId: number,
  approvals: Array<{ proposal_id: number; acknowledge_stale?: boolean }>, expectedRevision: number,
): void {
  requireEditableDeck(db, actor, deckId);
  requireUi(actor, "rule on proposals");
  withTransaction(db, () => {
    requireRevision(db, actor, deckId, expectedRevision);
    if (!approvals.length || new Set(approvals.map(a => a.proposal_id)).size !== approvals.length)
      throw new ServiceError("Select each open proposal once");
    const pending = approvals.map(approval => {
      const proposal = getProposal(db, actor, approval.proposal_id);
      if (!proposal || proposal.deck_id !== deckId) throw new ServiceError("Every proposal must belong to this deck");
      if (proposal.stale && approval.acknowledge_stale !== true)
        throw new ServiceError("Proposal is stale; review changes and acknowledge_stale with expected_revision, or amend it", 409);
      const ids = pendingItemIds(db, actor, proposal.id);
      if (!ids.length) throw new ServiceError("Proposal has no pending items", 409);
      return ids;
    });
    for (const ids of pending) acceptItems(db, actor, ids, "", {
      expected_revision: requireDeck(db, actor, deckId).revision,
      // All original bases were checked above, within this transaction. Only
      // this batch can have advanced the revision since that check.
      acknowledge_stale: true,
    });
  });
}

/** Reject items. Nothing changes; the ruling and its optional reason are logged. */
export function rejectItems(
  db: Database,
  actor: Actor,
  itemIds: number[],
  reason = "",
): number {
  const items = itemsToRule(db, actor, itemIds);
  requireUi(actor, "rule on a proposal");
  const deckId = items[0].deck_id;
  requireEditableDeck(db, actor, deckId);

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
    markItems(db, actor, items, "rejected");
    resolveProposalIfDone(db, actor, items[0].proposal_id);
    return logId;
  });
}

/** Every item of a proposal that is still pending. */
export function pendingItemIds(db: Database, actor: Actor, proposalId: number): number[] {
  if (!getProposal(db, actor, proposalId)) throw new ServiceError(`Proposal ${proposalId} not found`, 404);
  return (
    db
      .prepare("SELECT id FROM proposal_items WHERE proposal_id = ? AND status = 'pending' ORDER BY id")
      .all(proposalId) as unknown as { id: number }[]
  ).map((r) => r.id);
}

/** Only current pending work can be revised. Items already ruled on remain
 * immutable in the previous proposal; the replacement describes remaining work. */
function requireOwnPending(db: Database, actor: Actor, id: number): ProposalView {
  const proposal = getProposal(db, actor, id, true);
  if (!proposal) throw new ServiceError(`Proposal ${id} not found`, 404);
  requireEditableDeck(db, actor, proposal.deck_id);
  if (actor.via !== "ui" && !isAuthor(actor, proposal))
    throw new PermissionError("Agents can amend or withdraw only their own pending proposals");
  if (!proposal.is_current || !proposal.items.some(i => i.status === "pending"))
    throw new ServiceError("Only a current proposal with pending items can be amended or withdrawn");
  return proposal;
}

export function withdrawProposal(db: Database, actor: Actor, id: number, reason: string, expectedRevision?: number): void {
  withTransaction(db, () => {
    const proposal = requireOwnPending(db, actor, id);
    requireRevision(db, actor, proposal.deck_id, expectedRevision);
    if (!reason?.trim()) throw new ServiceError("Withdrawing a proposal needs a reason");
    lintProse(db, reason);
    db.prepare("UPDATE proposals SET status = 'resolved', disposition = 'withdrawn' WHERE id = ?").run(id);
    recordActivity(db, actor, "withdraw_proposal", proposal.deck_id, `Withdrew proposal #${id}: ${reason}`);
  });
}

export function amendProposal(db: Database, actor: Actor, id: number,
  input: { summary?: string; items: ProposalItemInput[]; expected_revision?: number }): number {
  return withTransaction(db, () => {
    const proposal = requireOwnPending(db, actor, id);
    requireRevision(db, actor, proposal.deck_id, input.expected_revision);
    db.prepare("UPDATE proposals SET status = 'resolved', disposition = 'superseded' WHERE id = ?").run(id);
    const replacement = createProposal(db, actor, proposal.deck_id, input);
    db.prepare("UPDATE proposals SET superseded_by = ? WHERE id = ?").run(replacement, id);
    recordActivity(db, actor, "amend_proposal", proposal.deck_id, `Proposal #${replacement} supersedes #${id}`);
    return replacement;
  });
}
