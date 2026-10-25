import { requireDeckOwner } from "../actor.ts";
// Forks and comparison (spec §5.3). A fork copies a deck's list,
// organization, brief and deck memories into a new Theorycrafted deck and records the
// parent. The copies then diverge; the link exists so the two can be compared.
// Forks replace named checkpoints: there is no snapshot system and no DAG.
//
// Another user's shared deck is forked by its slug (spec §18.4) and copies
// less: never memories, ownership evidence, the power target, assessments,
// proposals or log entries, and the maybe list arrives without its notes.

import type { Database } from "../sqlite.ts";
import { ServiceError } from "../errors.ts";
import { withTransaction } from "../db.ts";
import type { Actor } from "../actor.ts";
import { copyPowerTarget } from "./power.ts";
import { copyDeckMemories } from "../memory.ts";
import { writeLog } from "./log.ts";
import { publicDeckState } from "../state.ts";
import { type DeckRow, createDeck, getDeck, recomputeIdentity, requireDeck, requireViewableDeck } from "./service.ts";

/** A deck to fork or compare against: one of the principal's own by id, or a
 * shared deck by its slug. */
export type DeckSource = number | { slug: string };

export function forkDeck(db: Database, actor: Actor, source: DeckSource, name: string): number {
  const parent: DeckRow = typeof source === "number" ? requireDeckOwner(db, actor, source) : requireViewableDeck(db, actor, source.slug);
  const deckId = parent.id;
  // Forking one's own deck by its link is the ordinary fork.
  const foreign = parent.user_id !== actor.user_id;
  return withTransaction(db, () => {
    const forkId = createDeck(db, actor, name, foreign ? {} : { parentId: deckId });
    if (foreign) {
      const handle = (db.prepare("SELECT handle FROM users WHERE id = ?").get(parent.user_id) as { handle: string }).handle;
      // Lineage by snapshot only: the other user's numeric id is theirs (§18.3)
      // and never stored where the forker's own reads or export could show it.
      db.prepare("UPDATE decks SET parent_owner_handle = ?, parent_slug = ?, parent_name = ? WHERE id = ?")
        .run(handle, parent.slug, parent.name, forkId);
    }

    const slotMap = new Map<number, number>();
    const insSlot = db.prepare(
      "INSERT INTO slots (deck_id, name, target_min, target_max, position) VALUES (?, ?, ?, ?, ?)",
    );
    for (const s of db
      .prepare("SELECT id, name, target_min, target_max, position FROM slots WHERE deck_id = ?")
      .all(deckId) as unknown as Array<{
      id: number;
      name: string;
      target_min: number | null;
      target_max: number | null;
      position: number;
    }>)
      slotMap.set(
        s.id,
        Number(insSlot.run(forkId, s.name, s.target_min, s.target_max, s.position).lastInsertRowid),
      );

    const tagMap = new Map<number, number>();
    const insTag = db.prepare("INSERT INTO tags (deck_id, name) VALUES (?, ?)");
    for (const t of db.prepare("SELECT id, name FROM tags WHERE deck_id = ?").all(deckId) as unknown as Array<{
      id: number;
      name: string;
    }>)
      tagMap.set(t.id, Number(insTag.run(forkId, t.name).lastInsertRowid));

    const insCard = db.prepare(
      `INSERT INTO deck_cards (deck_id, oracle_id, slot_id, role, maybe, maybe_note, quantity)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
    );
    for (const c of db
      .prepare(
        "SELECT oracle_id, slot_id, role, maybe, maybe_note, quantity FROM deck_cards WHERE deck_id = ?",
      )
      .all(deckId) as unknown as Array<{
      oracle_id: string;
      slot_id: number | null;
      role: string;
      maybe: number;
      maybe_note: string;
      quantity: number;
    }>)
      insCard.run(
        forkId,
        c.oracle_id,
        c.slot_id == null ? null : (slotMap.get(c.slot_id) ?? null),
        c.role,
        c.maybe,
        foreign ? "" : c.maybe_note,
        c.quantity,
      );

    const insSecondary = db.prepare("INSERT INTO deck_card_secondary_slots (deck_id, oracle_id, slot_id) VALUES (?, ?, ?)");
    for (const c of db.prepare("SELECT oracle_id, slot_id FROM deck_card_secondary_slots WHERE deck_id = ?").all(deckId) as unknown as { oracle_id: string; slot_id: number }[]) {
      const slotId = slotMap.get(c.slot_id);
      if (slotId != null) insSecondary.run(forkId, c.oracle_id, slotId);
    }

    const insCardTag = db.prepare(
      "INSERT INTO deck_card_tags (deck_id, oracle_id, tag_id) VALUES (?, ?, ?)",
    );
    for (const ct of db
      .prepare("SELECT oracle_id, tag_id FROM deck_card_tags WHERE deck_id = ?")
      .all(deckId) as unknown as Array<{ oracle_id: string; tag_id: number }>) {
      const tagId = tagMap.get(ct.tag_id);
      if (tagId != null) insCardTag.run(forkId, ct.oracle_id, tagId);
    }

    db.prepare(
      `INSERT INTO briefs (deck_id, thesis, constraints_md)
       SELECT ?, thesis, constraints_md FROM briefs WHERE deck_id = ?`,
    ).run(forkId, deckId);
    const insEngine = db.prepare("INSERT INTO engines (deck_id, name, description) VALUES (?, ?, ?)");
    const insPiece = db.prepare(
      "INSERT INTO engine_pieces (engine_id, oracle_id, note) SELECT ?, oracle_id, note FROM engine_pieces WHERE engine_id = ?",
    );
    for (const e of db
      .prepare("SELECT id, name, description FROM engines WHERE deck_id = ?")
      .all(deckId) as unknown as Array<{ id: number; name: string; description: string }>)
      insPiece.run(Number(insEngine.run(forkId, e.name, e.description).lastInsertRowid), e.id);

    recomputeIdentity(db, actor, forkId);
    if (foreign) {
      const upstream = getDeck(db, actor, forkId).deck.upstream!;
      writeLog(db, actor, { deck_id: forkId, kind: "edit", summary: `Forked from ${upstream.owner_handle}/${upstream.name}` });
      return forkId;
    }
    copyPowerTarget(db, actor, deckId, forkId);
    const memories = copyDeckMemories(db, actor, deckId, forkId);
    writeLog(db, actor, {
      deck_id: forkId,
      kind: "edit",
      summary:
        `Forked from "${parent.name}" at its revision ${parent.revision}` +
        (memories ? `, with ${memories} deck memor${memories === 1 ? "y" : "ies"}` : ""),
    });
    return forkId;
  });
}

export interface CompareCard {
  oracle_id: string;
  name: string;
  mana_cost: string | null;
  type_line: string;
  quantity: number;
  role: string;
}

export interface DeckComparison {
  a: { id: number; name: string; revision: number; card_count: number };
  /** A shared deck by its slug has no id or revision for the caller to see. */
  b: { id: number | null; slug: string | null; owner_handle: string | null; name: string; revision: number | null; card_count: number };
  /** True when b was chosen because it is a's parent. */
  b_is_parent: boolean;
  only_in_a: CompareCard[];
  only_in_b: CompareCard[];
  quantity_differs: Array<{ name: string; in_a: number; in_b: number }>;
  role_differs: Array<{ name: string; in_a: string; in_b: string }>;
  shared_count: number;
}

type CompareSide = { cards: Array<CompareCard & { maybe: number }> };

/** Compare two decks' lists (maybe lists excluded). With no second deck, compares against the parent:
 * one's own, or the shared deck a cross-owner fork came from while it can still be read (§18.4). */
export function compareDecks(db: Database, actor: Actor, aId: number, other?: DeckSource): DeckComparison {
  const a = getDeck(db, actor, aId);
  let target = other;
  if (target === undefined) {
    if (a.deck.parent) target = a.deck.parent.id;
    else if (a.deck.upstream) target = { slug: a.deck.upstream.slug };
    else
      throw new ServiceError(
        `"${a.deck.name}" is not a fork, so there is no parent to compare with. Name a second deck.`,
      );
  }
  let b: DeckComparison["b"];
  let bSide: CompareSide;
  if (typeof target === "number") {
    if (target === aId) throw new ServiceError("Compare a deck with a different deck");
    const view = getDeck(db, actor, target);
    b = { id: view.deck.id, slug: null, owner_handle: null, name: view.deck.name, revision: view.deck.revision, card_count: view.computed.card_count };
    bSide = view;
  } else {
    let shared;
    try {
      if (requireViewableDeck(db, actor, target.slug).id === aId) throw new ServiceError("Compare a deck with a different deck");
      shared = publicDeckState(db, actor, target.slug);
    }
    catch (e) {
      if (other === undefined && e instanceof ServiceError && e.status === 404)
        throw new ServiceError(`"${a.deck.name}" was forked from ${a.deck.upstream!.owner_handle}/${a.deck.upstream!.name}, which you can no longer read: it is private now or was deleted.`, 404);
      throw e;
    }
    b = { id: null, slug: shared.deck.slug, owner_handle: shared.deck.owner.handle, name: shared.deck.name, revision: null, card_count: shared.computed.card_count };
    bSide = shared;
  }

  const main = (view: CompareSide) => new Map(view.cards.filter((c) => !c.maybe).map((c) => [c.oracle_id, c]));
  const inA = main(a);
  const inB = main(bSide);
  const brief = (c: CompareCard): CompareCard => ({
    oracle_id: c.oracle_id,
    name: c.name,
    mana_cost: c.mana_cost,
    type_line: c.type_line,
    quantity: c.quantity,
    role: c.role,
  });

  const onlyA = [...inA.values()].filter((c) => !inB.has(c.oracle_id)).map(brief);
  const onlyB = [...inB.values()].filter((c) => !inA.has(c.oracle_id)).map(brief);
  const shared = [...inA.values()].filter((c) => inB.has(c.oracle_id));

  return {
    a: { id: a.deck.id, name: a.deck.name, revision: a.deck.revision, card_count: a.computed.card_count },
    b,
    b_is_parent: other === undefined,
    only_in_a: onlyA,
    only_in_b: onlyB,
    quantity_differs: shared
      .filter((c) => c.quantity !== inB.get(c.oracle_id)!.quantity)
      .map((c) => ({ name: c.name, in_a: c.quantity, in_b: inB.get(c.oracle_id)!.quantity })),
    role_differs: shared
      .filter((c) => c.role !== inB.get(c.oracle_id)!.role)
      .map((c) => ({ name: c.name, in_a: c.role, in_b: inB.get(c.oracle_id)!.role })),
    shared_count: shared.length,
  };
}
