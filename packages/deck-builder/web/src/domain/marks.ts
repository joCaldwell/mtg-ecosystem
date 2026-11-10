import type { DeckListState, Memory } from "../api/client.ts";

export interface CardMark {
  memories: number;
  decisions: number;
}

/**
 * Which cards carry context worth a marker in the list: active memories
 * linked to the card (global ones, and this deck's), and decisions on it in the
 * deck's recent log (accept or reject entries that have not been undone).
 * Keyed by oracle_id; only cards with at least one of either appear.
 */
export function cardMarks(state: DeckListState, memories: Memory[]): Map<string, CardMark> {
  const marks = new Map<string, CardMark>();
  const bump = (oracleId: string, field: keyof CardMark) => {
    const mark = marks.get(oracleId) ?? { memories: 0, decisions: 0 };
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
    for (const c of new Set(entry.cards.map((x) => x.oracle_id))) bump(c, "decisions");
  }

  return marks;
}
