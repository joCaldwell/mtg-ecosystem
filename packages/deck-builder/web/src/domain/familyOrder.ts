import type { DeckSummary } from "../api/client.ts";

/** List every descendant immediately after its parent, preserving roster order. */
export function familyOrder(decks: DeckSummary[]): Array<{ deck: DeckSummary; fork: boolean }> {
  const ids = new Set(decks.map(d => d.id)), seen = new Set<number>();
  const result: Array<{ deck: DeckSummary; fork: boolean }> = [];
  const visit = (deck: DeckSummary, fork: boolean) => {
    if (seen.has(deck.id)) return;
    seen.add(deck.id); result.push({ deck, fork });
    for (const child of decks.filter(d => d.parent_id === deck.id)) visit(child, true);
  };
  for (const deck of decks.filter(d => d.parent_id == null || !ids.has(d.parent_id))) visit(deck, false);
  for (const deck of decks) visit(deck, false);
  return result;
}
