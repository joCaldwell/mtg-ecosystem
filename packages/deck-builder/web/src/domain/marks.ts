import type { DeckState, Memory } from "../api/client.ts";

export interface CardMark {
  memories: number;
  rulings: number;
}

/**
 * Which cards carry context worth a marker in the list: active memories
 * linked to the card (global ones, and this deck's), and rulings on it in the
 * deck's recent log (accept or reject entries that have not been undone).
 * Keyed by oracle_id; only cards with at least one of either appear.
 */
export function cardMarks(state: DeckState, memories: Memory[]): Map<string, CardMark> {
  const marks = new Map<string, CardMark>();
  const bump = (oracleId: string, field: keyof CardMark) => {
    const mark = marks.get(oracleId) ?? { memories: 0, rulings: 0 };
    mark[field]++;
    marks.set(oracleId, mark);
  };

  for (const m of memories) {
    if (m.status !== "active") continue;
    if (m.scope === "deck" && m.deck_id !== state.deck.id) continue;
    for (const c of new Set(m.cards.map((x) => x.oracle_id))) bump(c, "memories");
  }

  for (const entry of state.log) {
    if (entry.kind !== "accept" && entry.kind !== "reject") continue;
    if (entry.undone_by != null) continue;
    for (const c of new Set(entry.cards.map((x) => x.oracle_id))) bump(c, "rulings");
  }

  return marks;
}

/**
 * Whether per-card "not in paper" markers say anything. The paper flag works
 * exception-first (spec §5.5): build the deck, then mark the few cards that
 * are missing. Until most of the deck is in paper, nearly every row would
 * carry the marker and it would mark nothing.
 */
export function paperMarksUseful(state: DeckState): boolean {
  let total = 0;
  let inPaper = 0;
  for (const c of state.cards) {
    if (c.maybe) continue;
    total += c.quantity;
    if (c.in_paper) inPaper += c.quantity;
  }
  return total > 0 && inPaper * 2 >= total;
}
