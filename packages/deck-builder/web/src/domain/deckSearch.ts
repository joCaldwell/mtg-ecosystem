import type { DeckCard, ProposalItem } from "../api/client.ts";
import type { DeckLayout } from "./grouping.ts";

export type TagMatch = "any" | "all";

export interface TagFilter {
  ids: number[];
  match: TagMatch;
  inverted?: boolean;
}

export function matchesTags(tagIds: number[], filter: TagFilter): boolean {
  if (filter.ids.length === 0) return true;
  const has = (id: number) => tagIds.includes(id);
  const matches = filter.match === "all" ? filter.ids.every(has) : filter.ids.some(has);
  return filter.inverted ? !matches : matches;
}

export interface DeckFilterMatches {
  active: boolean;
  cardIds: Set<string>;
  addIds: Set<number>;
  /** Every match once, in the order the list draws it: `card:<oracle_id>` or
   *  `add:<item id>`. Find-in-deck steps through these; a read-only view,
   *  which has no find controls, may leave it out. */
  ordered?: string[];
}

export const matchKey = {
  card: (oracleId: string) => `card:${oracleId}`,
  add: (itemId: number) => `add:${itemId}`,
};

/** Identify matches without removing or reordering any rows, groups or review decisions. */
export function matchDeckList(
  layout: DeckLayout,
  addsByGroup: Map<string, ProposalItem[]>,
  searchTerm: string,
  tags: TagFilter = { ids: [], match: "any" },
  /** Proposed adds can inherit tags from a parked card when accepted. */
  cards: DeckCard[] = [],
): DeckFilterMatches {
  const query = searchTerm.trim().toLowerCase();
  const result: DeckFilterMatches & { ordered: string[] } = {
    active: !!query || tags.ids.length > 0,
    cardIds: new Set(),
    addIds: new Set(),
    ordered: [],
  };
  if (!result.active) return result;

  const matches = (card: DeckCard) => card.name.toLowerCase().includes(query) && matchesTags(card.tag_ids, tags);
  const tagsByCard = new Map(cards.map((card) => [card.oracle_id, card.tag_ids]));
  const visitCards = (list: DeckCard[]) => {
    for (const card of list) {
      if (!result.cardIds.has(card.oracle_id) && matches(card)) {
        result.cardIds.add(card.oracle_id);
        result.ordered.push(matchKey.card(card.oracle_id));
      }
    }
  };
  const visitAdds = (adds: ProposalItem[] = []) => {
    for (const item of adds) {
      if (!result.addIds.has(item.id) && item.card_name.toLowerCase().includes(query) && matchesTags(tagsByCard.get(item.oracle_id) ?? [], tags)) {
        result.addIds.add(item.id);
        result.ordered.push(matchKey.add(item.id));
      }
    }
  };
  // DeckList's order: commanders, each group's rows then its proposed adds,
  // the companion, then the maybe list.
  visitCards(layout.commanders);
  for (const group of layout.groups) {
    visitCards(group.cards);
    visitAdds(addsByGroup.get(group.key));
  }
  visitCards(layout.companions);
  visitCards(layout.maybe);
  for (const adds of addsByGroup.values()) visitAdds(adds);
  return result;
}
