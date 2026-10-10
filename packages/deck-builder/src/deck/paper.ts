// The paper flag, read side (spec §5.5). `in_paper` means the physical card
// is in the physical deck — not "owned". This is the one place an agent can
// read it, and only by asking: it appears in no default view, because ambient
// knowledge of what the owner has would bias recommendations toward it.

import type { DatabaseSync } from "node:sqlite";
import { getDeck } from "./service.ts";

export interface PaperStatus {
  deck: { id: number; name: string };
  /** Cards of the deck (maybe list excluded) and how many are in paper. */
  total_cards: number;
  in_paper_cards: number;
  /** Missing here, but another deck holds a physical copy. */
  pull: Array<{
    name: string;
    type_line: string;
    quantity: number;
    held_by: Array<{ deck_id: number; deck_name: string }>;
  }>;
  /** Missing here and in no other deck's paper. */
  buy: Array<{ name: string; type_line: string; quantity: number }>;
}

export function paperStatus(db: DatabaseSync, deckId: number): PaperStatus {
  const { deck, cards } = getDeck(db, deckId);
  const main = cards.filter((c) => !c.maybe);
  const holders = db.prepare(
    `SELECT d.id AS deck_id, d.name AS deck_name FROM deck_cards dc JOIN decks d ON d.id = dc.deck_id
     WHERE dc.oracle_id = ? AND dc.deck_id != ? AND dc.in_paper = 1 AND dc.maybe = 0 ORDER BY d.name`,
  );

  const pull: PaperStatus["pull"] = [];
  const buy: PaperStatus["buy"] = [];
  for (const c of main.filter((c) => !c.in_paper)) {
    const heldBy = (
      holders.all(c.oracle_id, deckId) as unknown as Array<{ deck_id: number; deck_name: string }>
    ).map((h) => ({ deck_id: h.deck_id, deck_name: h.deck_name }));
    // The type line rides along so a reader sorting the list by kind does not
    // have to recall what each card is.
    if (heldBy.length)
      pull.push({ name: c.name, type_line: c.type_line, quantity: c.quantity, held_by: heldBy });
    else buy.push({ name: c.name, type_line: c.type_line, quantity: c.quantity });
  }

  return {
    deck: { id: deck.id, name: deck.name },
    total_cards: main.reduce((n, c) => n + c.quantity, 0),
    in_paper_cards: main.filter((c) => c.in_paper).reduce((n, c) => n + c.quantity, 0),
    pull,
    buy,
  };
}
