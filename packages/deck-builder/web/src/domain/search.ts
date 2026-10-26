import type { CardData, DeckState } from "../api/client.ts";
import type { CardMark } from "./marks.ts";
import { pendingOf } from "./grouping.ts";

export interface AnnotatedResult {
  card: CardData;
  inDeck: boolean;
  onMaybe: boolean;
  /** A pending proposal item names this card. */
  proposed: "add" | "cut" | null;
  /** The most recent "reject" log entry containing the card; reason is the owner's decision_reason. */
  lastRejection: { revision: number; reason: string } | null;
  memories: number;
  /** Legal in Commander. The card data also holds digital-only rebalanced cards, which are not. */
  legal: boolean;
}

/**
 * Search results with what the deck already knows about each card. Cards
 * that are legal in Commander come first, in the server's order; the rest
 * follow, so a not-legal card never pushes a playable one off the list.
 */
export function annotateResults(
  results: CardData[],
  state: DeckState,
  marks: Map<string, CardMark>,
): AnnotatedResult[] {
  const rows = new Map(state.cards.map((c) => [c.oracle_id, c]));
  const proposed = new Map<string, "add" | "cut">();
  for (const item of pendingOf(state)) if (!proposed.has(item.oracle_id)) proposed.set(item.oracle_id, item.action);

  // The log is newest first, so the first reject that names a card is its latest.
  const rejection = new Map<string, { revision: number; reason: string }>();
  for (const entry of state.log) {
    if (entry.kind !== "reject" || entry.undone_by != null) continue;
    for (const c of entry.cards)
      if (!rejection.has(c.oracle_id))
        rejection.set(c.oracle_id, { revision: entry.revision, reason: entry.decision_reason });
  }

  const annotated = results.map((card) => {
    const row = rows.get(card.oracle_id);
    return {
      card,
      inDeck: !!row && !row.maybe,
      onMaybe: !!row && !!row.maybe,
      proposed: proposed.get(card.oracle_id) ?? null,
      lastRejection: rejection.get(card.oracle_id) ?? null,
      memories: marks.get(card.oracle_id)?.memories ?? 0,
      legal: card.commander_legality === "legal",
    };
  });
  return [...annotated.filter((r) => r.legal), ...annotated.filter((r) => !r.legal)];
}
