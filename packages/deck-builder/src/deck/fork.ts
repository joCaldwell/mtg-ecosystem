import { requireDeckOwner } from "../actor.ts";
// Forks and comparison (spec §5.3). A fork copies a deck's list,
// organization, brief and deck memories into a new Theorycrafted deck and records the
// parent. The copies then diverge; the link exists so the two can be compared.
// Forks replace named checkpoints: there is no snapshot system and no DAG.

import type { Database } from "../sqlite.ts";
import { ServiceError } from "../errors.ts";
import { withTransaction } from "../db.ts";
import type { Actor } from "../actor.ts";
import { copyPowerTarget } from "./power.ts";
import { copyDeckMemories } from "../memory.ts";
import { writeLog } from "./log.ts";
import { createDeck, getDeck, recomputeIdentity, requireDeck } from "./service.ts";

export function forkDeck(db: Database, actor: Actor, deckId: number, name: string): number {
  const parent = requireDeckOwner(db, actor, deckId);
  return withTransaction(db, () => {
    const forkId = createDeck(db, actor, name, { parentId: deckId });

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
        c.maybe_note,
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

    copyPowerTarget(db, actor, deckId, forkId);
    recomputeIdentity(db, actor, forkId);
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
  b: { id: number; name: string; revision: number; card_count: number };
  /** True when b was chosen because it is a's parent. */
  b_is_parent: boolean;
  only_in_a: CompareCard[];
  only_in_b: CompareCard[];
  quantity_differs: Array<{ name: string; in_a: number; in_b: number }>;
  role_differs: Array<{ name: string; in_a: string; in_b: string }>;
  shared_count: number;
}

/** Compare two decks' lists (maybe lists excluded). With no second deck, compares against the parent. */
export function compareDecks(db: Database, actor: Actor, aId: number, bId?: number): DeckComparison {
  const a = getDeck(db, actor, aId);
  let other = bId;
  if (other === undefined) {
    if (!a.deck.parent)
      throw new ServiceError(
        `"${a.deck.name}" is not a fork, so there is no parent to compare with. Name a second deck.`,
      );
    other = a.deck.parent.id;
  }
  if (other === aId) throw new ServiceError("Compare a deck with a different deck");
  const b = getDeck(db, actor, other);

  const main = (view: typeof a) => new Map(view.cards.filter((c) => !c.maybe).map((c) => [c.oracle_id, c]));
  const inA = main(a);
  const inB = main(b);
  const brief = (c: (typeof a.cards)[number]): CompareCard => ({
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
    b: { id: b.deck.id, name: b.deck.name, revision: b.deck.revision, card_count: b.computed.card_count },
    b_is_parent: bId === undefined,
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
