import { requireDeckOwner } from "../actor.ts";
// Membership: the one place the contents of a deck change. Accepted
// proposals, direct edits, imports and undos all arrive here as a list of
// changes, are applied together, bump the revision once, and leave one log
// entry (spec §5.1, §8.2).
//
// The permission rule of the whole app lives in applyChanges (spec §5.2): an
// agent may change membership directly only on a Theorycrafted deck.

import type { Database } from "../sqlite.ts";
import { ServiceError } from "../errors.ts";
import { withTransaction } from "../db.ts";
import { type Actor, PermissionError, requireUi } from "../actor.ts";
import { requireCardById } from "../cards.ts";
import { type LogCardInput, type LogKind, getLogEntry, writeLog } from "./log.ts";
import {
  type DeckCardRow,
  type Role,
  findDeckCard,
  recomputeIdentity,
  requireEditableDeck,
  requireDirectCardEdit,
  organizeCard,
  validateSlotAssignments,
} from "./service.ts";

export type Change =
  | {
      action: "add";
      oracle_id: string;
      quantity?: number;
      slot_id?: number | null;
      secondary_slot_ids?: number[];
      role?: Role;
      item_id?: number;
      reason?: string;
      /** Restored by undo; never set by a caller adding a card fresh. */
      tag_ids?: number[];
    }
  | {
      action: "cut";
      oracle_id: string;
      /** Copies to remove; "all" removes the card. Defaults to 1. */
      quantity?: number | "all";
      /** Move the card to the maybe list instead of out of the deck. */
      park?: boolean;
      item_id?: number;
      reason?: string;
    }
  | { action: "role"; oracle_id: string; role: Role };

export interface ChangeEntry {
  kind: Exclude<LogKind, "reject">;
  summary?: string;
  decision_reason?: string;
  proposal_id?: number;
  undo_of?: number;
}

// What a cut removed, so undo can put it back as it was.
interface CutBefore {
  slot_id: number | null;
  secondary_slot_ids: number[];
  role: Role;
  tag_ids: number[];
  parked: boolean;
}

function tagIdsOf(db: Database, actor: Actor, deckId: number, oracleId: string): number[] {
  return (
    db
      .prepare("SELECT tag_id FROM deck_card_tags WHERE deck_id = ? AND oracle_id = ?")
      .all(deckId, oracleId) as unknown as { tag_id: number }[]
  ).map((t) => t.tag_id);
}

function slotExists(db: Database, actor: Actor, deckId: number, slotId: number): boolean {
  return !!db.prepare("SELECT 1 FROM slots WHERE id = ? AND deck_id = ?").get(slotId, deckId);
}

function assertRoleCapacity(db: Database, actor: Actor, deckId: number, role: "commander" | "companion") {
  const { n } = db
    .prepare("SELECT COUNT(*) n FROM deck_cards WHERE deck_id = ? AND role = ? AND maybe = 0")
    .get(deckId, role) as { n: number };
  const cap = role === "commander" ? 2 : 1;
  if (n >= cap)
    throw new ServiceError(
      role === "commander"
        ? "A deck supports at most two command-zone cards (commander plus partner or background). " +
          "Change the current commander's role first."
        : "A deck supports at most one companion",
    );
}

function setRole(db: Database, actor: Actor, deckId: number, oracleId: string, role: Role) {
  if (role !== "card") assertRoleCapacity(db, actor, deckId, role);
  // Zone and purpose are independent; becoming a commander keeps its roles.
  db.prepare("UPDATE deck_cards SET role = ? WHERE deck_id = ? AND oracle_id = ?")
    .run(role, deckId, oracleId);
}

function applyAdd(
  db: Database, actor: Actor,
  deckId: number,
  c: Extract<Change, { action: "add" }>,
): LogCardInput {
  const card = requireCardById(db, c.oracle_id);
  const quantity = c.quantity ?? 1;
  if (!Number.isInteger(quantity) || quantity < 1)
    throw new ServiceError(`Cannot add ${quantity} copies of ${card.name}`);
  if (c.slot_id != null && !slotExists(db, actor, deckId, c.slot_id))
    throw new ServiceError(`Slot ${c.slot_id} not found in deck ${deckId}`, 404);

  const existing = findDeckCard(db, actor, deckId, c.oracle_id);
  const primary = c.slot_id !== undefined ? c.slot_id : existing?.slot_id ?? null;
  const secondary = c.secondary_slot_ids !== undefined ? c.secondary_slot_ids : (existing?.secondary_slot_ids ?? []).filter(id => id !== primary);
  validateSlotAssignments(db, actor, deckId, primary, secondary);
  let wasMaybe = false;
  if (existing?.maybe) {
    // Promoting a shortlisted card keeps the slot and tags it was parked with.
    wasMaybe = true;
    db.prepare(
      "UPDATE deck_cards SET maybe = 0, maybe_note = '', quantity = ? WHERE deck_id = ? AND oracle_id = ?",
    ).run(quantity, deckId, c.oracle_id);
    organizeCard(db, actor, deckId, c.oracle_id, { slotId: primary, secondarySlotIds: secondary });
    if (c.role && c.role !== "card") setRole(db, actor, deckId, c.oracle_id, c.role);
  } else if (existing) {
    // A second add is another copy (basics need it). Whether the count is
    // legal is a check's job, not a gate here.
    db.prepare(
      "UPDATE deck_cards SET quantity = quantity + ? WHERE deck_id = ? AND oracle_id = ?",
    ).run(quantity, deckId, c.oracle_id);
  } else {
    const role = c.role ?? "card";
    if (role !== "card") assertRoleCapacity(db, actor, deckId, role);
    db.prepare(
      `INSERT INTO deck_cards (deck_id, oracle_id, slot_id, role, quantity)
       VALUES (?, ?, ?, ?, ?)`,
    ).run(
      deckId,
      c.oracle_id,
      primary,
      role,
      quantity,
    );
    organizeCard(db, actor, deckId, c.oracle_id, { secondarySlotIds: secondary });
    if (c.tag_ids?.length) {
      const live = new Set(
        (db.prepare("SELECT id FROM tags WHERE deck_id = ?").all(deckId) as unknown as {
          id: number;
        }[]).map((t) => t.id),
      );
      const ins = db.prepare(
        "INSERT OR IGNORE INTO deck_card_tags (deck_id, oracle_id, tag_id) VALUES (?, ?, ?)",
      );
      for (const id of c.tag_ids) if (live.has(id)) ins.run(deckId, c.oracle_id, id);
    }
  }
  return {
    oracle_id: c.oracle_id,
    card_name: card.name,
    action: "add",
    quantity,
    item_id: c.item_id ?? null,
    reason: c.reason ?? "",
    before: { was_maybe: wasMaybe },
  };
}

function applyCut(
  db: Database, actor: Actor,
  deckId: number,
  c: Extract<Change, { action: "cut" }>,
): LogCardInput {
  const card = requireCardById(db, c.oracle_id);
  const row = findDeckCard(db, actor, deckId, c.oracle_id);
  if (!row || row.maybe)
    throw new ServiceError(`${card.name} is not in the deck, so it cannot be cut`);

  const requested = c.quantity === "all" || c.park ? row.quantity : (c.quantity ?? 1);
  if (!Number.isInteger(requested) || requested < 1)
    throw new ServiceError(`Cannot cut ${requested} copies of ${card.name}`);
  const quantity = Math.min(requested, row.quantity);

  const before: CutBefore = {
    slot_id: row.slot_id,
    secondary_slot_ids: row.secondary_slot_ids,
    role: row.role,
    tag_ids: tagIdsOf(db, actor, deckId, c.oracle_id),
    parked: !!c.park,
  };

  if (c.park) {
    db.prepare(
      `UPDATE deck_cards SET maybe = 1, quantity = 1, role = 'card'
       WHERE deck_id = ? AND oracle_id = ?`,
    ).run(deckId, c.oracle_id);
  } else if (quantity >= row.quantity) {
    db.prepare("DELETE FROM deck_cards WHERE deck_id = ? AND oracle_id = ?").run(
      deckId,
      c.oracle_id,
    );
  } else {
    db.prepare(
      "UPDATE deck_cards SET quantity = quantity - ? WHERE deck_id = ? AND oracle_id = ?",
    ).run(quantity, deckId, c.oracle_id);
  }
  return {
    oracle_id: c.oracle_id,
    card_name: card.name,
    action: "cut",
    quantity,
    item_id: c.item_id ?? null,
    reason: c.reason ?? "",
    before,
  };
}

function applyRole(
  db: Database, actor: Actor,
  deckId: number,
  c: Extract<Change, { action: "role" }>,
): LogCardInput | null {
  const card = requireCardById(db, c.oracle_id);
  const row = findDeckCard(db, actor, deckId, c.oracle_id);
  if (!row || row.maybe)
    throw new ServiceError(`${card.name} is not in the deck, so its role cannot change`);
  if (row.role === c.role) return null;
  setRole(db, actor, deckId, c.oracle_id, c.role);
  return {
    oracle_id: c.oracle_id,
    card_name: card.name,
    action: "role",
    quantity: row.quantity,
    reason: `${row.role} → ${c.role}`,
    before: { from: row.role, slot_id: row.slot_id },
  };
}

/**
 * Apply a set of membership changes as one act. All of them land or none do.
 * Returns the id of the log entry, or null if nothing actually changed.
 */
export function applyChanges(
  db: Database,
  actor: Actor,
  deckId: number,
  changes: Change[],
  entry: ChangeEntry,
): number | null {
  requireDirectCardEdit(db, actor, deckId);
  if (actor.via === "agent" && entry.kind !== "edit")
    throw new PermissionError("Only the owner can rule on proposals, import over a deck, or undo.");

  return withTransaction(db, () => {
    const logged: LogCardInput[] = [];
    for (const c of changes) {
      const row =
        c.action === "add"
          ? applyAdd(db, actor, deckId, c)
          : c.action === "cut"
            ? applyCut(db, actor, deckId, c)
            : applyRole(db, actor, deckId, c);
      if (row) logged.push(row);
    }
    if (!logged.length) return null;

    // Any of these can move a card into or out of the command zone.
    recomputeIdentity(db, actor, deckId);
    db.prepare("UPDATE decks SET revision = revision + 1 WHERE id = ?").run(deckId);
    return writeLog(
      db,
      actor,
      {
        deck_id: deckId,
        kind: entry.kind,
        summary: entry.summary ?? summarize(logged),
        decision_reason: entry.decision_reason,
        proposal_id: entry.proposal_id,
        undo_of: entry.undo_of,
      },
      logged,
    );
  });
}

function summarize(cards: LogCardInput[]): string {
  if (cards.length === 1) {
    const c = cards[0];
    if (c.action === "role") return `${c.card_name}: ${c.reason}`;
    const copies = (c.quantity ?? 1) > 1 ? `${c.quantity}× ` : "";
    return `${c.action === "add" ? "Added" : "Cut"} ${copies}${c.card_name}`;
  }
  const count = (a: string) =>
    cards.filter((c) => c.action === a).reduce((n, c) => n + (c.quantity ?? 1), 0);
  const parts = [];
  if (count("add")) parts.push(`+${count("add")}`);
  if (count("cut")) parts.push(`−${count("cut")}`);
  const roles = cards.filter((c) => c.action === "role").length;
  if (roles) parts.push(`${roles} role change${roles > 1 ? "s" : ""}`);
  return `${parts.join(", ")} cards`;
}

// ---------- undo (spec §8.2: the reversal of a log entry) ----------

export interface UndoResult {
  log_id: number;
  /** Parts of the entry that could not be reversed because the deck has moved on. */
  skipped: string[];
}

export function undo(db: Database, actor: Actor, logId: number): UndoResult {
  const entry = getLogEntry(db, actor, logId);
  requireUi(actor, "undo a change");
  if (!entry || entry.deck_id == null) throw new ServiceError(`Log entry ${logId} not found`, 404);
  requireEditableDeck(db, actor, entry.deck_id);
  if (entry.kind === "reject")
    throw new ServiceError("A rejection changed nothing, so there is nothing to undo");
  if (entry.kind === "undo") throw new ServiceError("An undo cannot itself be undone");
  if (entry.undone_by != null) throw new ServiceError("This change has already been undone");
  const deckId = entry.deck_id;

  const rows = db
    .prepare(
      "SELECT oracle_id, card_name, action, quantity, before_json FROM log_cards WHERE log_id = ? ORDER BY id DESC",
    )
    .all(logId) as unknown as Array<{
    oracle_id: string;
    card_name: string;
    action: "add" | "cut" | "role";
    quantity: number;
    before_json: string | null;
  }>;
  if (!rows.length) throw new ServiceError("This entry changed no cards, so there is nothing to undo");

  const inverse: Change[] = [];
  const skipped: string[] = [];
  // Role reversals wait until the adds and cuts are planned: restoring a
  // commander needs the command zone to have room, which the reversal of a
  // later role change may be what frees.
  const roles: Change[] = [];

  for (const r of rows) {
    const before = r.before_json ? JSON.parse(r.before_json) : {};
    const current = findDeckCard(db, actor, deckId, r.oracle_id);
    if (r.action === "add") {
      if (!current || current.maybe) {
        skipped.push(`${r.card_name} is no longer in the deck`);
        continue;
      }
      inverse.push(
        before.was_maybe
          ? { action: "cut", oracle_id: r.oracle_id, park: true }
          : { action: "cut", oracle_id: r.oracle_id, quantity: r.quantity },
      );
    } else if (r.action === "cut") {
      const b = before as Partial<CutBefore>;
      if (current && !current.maybe) {
        if (b.parked) {
          skipped.push(`${r.card_name} is already back in the deck`);
          continue;
        }
        inverse.push({ action: "add", oracle_id: r.oracle_id, quantity: r.quantity });
      } else if (current?.maybe) {
        inverse.push({ action: "add", oracle_id: r.oracle_id, quantity: r.quantity });
        if (b.role && b.role !== "card")
          roles.push({ action: "role", oracle_id: r.oracle_id, role: b.role });
      } else {
        inverse.push({
          action: "add",
          oracle_id: r.oracle_id,
          quantity: r.quantity,
          slot_id: b.slot_id != null && slotExists(db, actor, deckId, b.slot_id) ? b.slot_id : null,
          secondary_slot_ids: (b.secondary_slot_ids ?? []).filter(id => slotExists(db, actor, deckId, id) && id !== b.slot_id),
          tag_ids: b.tag_ids ?? [],
        });
        if (b.role && b.role !== "card")
          roles.push({ action: "role", oracle_id: r.oracle_id, role: b.role });
      }
    } else {
      if (!current || current.maybe) {
        skipped.push(`${r.card_name} is no longer in the deck`);
        continue;
      }
      roles.push({ action: "role", oracle_id: r.oracle_id, role: before.from ?? "card" });
    }
  }

  // Demotions before promotions, so a swapped commander frees its seat first.
  roles.sort(
    (a, b) =>
      Number((a as { role: Role }).role !== "card") - Number((b as { role: Role }).role !== "card"),
  );

  return withTransaction(db, () => {
    const undoId = applyChanges(db, actor, deckId, [...inverse, ...roles], {
      kind: "undo",
      summary: `Undo of #${logId}: ${entry.summary}`,
      undo_of: logId,
    });
    if (undoId == null)
      throw new ServiceError(
        `Nothing left to undo: ${skipped.join("; ") || "the deck already matches"}`,
      );
    db.prepare("UPDATE log SET undone_by = ? WHERE id = ?").run(undoId, logId);
    return { log_id: undoId, skipped };
  });
}

// ---------- conveniences for the owner's direct edits ----------

/** Set a card's quantity as one logged act. */
export function setQuantity(
  db: Database,
  actor: Actor,
  deckId: number,
  oracleId: string,
  quantity: number,
): number | null {
  requireDeckOwner(db, actor, deckId);
  if (!Number.isInteger(quantity) || quantity < 1)
    throw new ServiceError("Quantity must be a whole number, at least 1");
  const row = findDeckCard(db, actor, deckId, oracleId) as DeckCardRow | undefined;
  if (!row || row.maybe)
    throw new ServiceError(`${requireCardById(db, oracleId).name} is not in the deck`);
  if (quantity === row.quantity) return null;
  return applyChanges(
    db,
    actor,
    deckId,
    [
      quantity > row.quantity
        ? { action: "add", oracle_id: oracleId, quantity: quantity - row.quantity }
        : { action: "cut", oracle_id: oracleId, quantity: row.quantity - quantity },
    ],
    { kind: "edit" },
  );
}
