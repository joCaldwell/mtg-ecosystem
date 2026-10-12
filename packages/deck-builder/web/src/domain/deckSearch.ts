import type { DeckCard, ProposalItem } from "../api/client.ts";
import type { DeckLayout } from "./grouping.ts";

export type TagMatch = "any" | "all";

export interface TagFilter {
  ids: number[];
  match: TagMatch;
}

export function matchesTags(tagIds: number[], filter: TagFilter): boolean {
  if (filter.ids.length === 0) return true;
  const has = (id: number) => tagIds.includes(id);
  return filter.match === "all" ? filter.ids.every(has) : filter.ids.some(has);
}

export interface DeckFilterMatches {
  active: boolean;
  cardIds: Set<string>;
  addIds: Set<number>;
}

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
  const result: DeckFilterMatches = {
    active: !!query || tags.ids.length > 0,
    cardIds: new Set(),
    addIds: new Set(),
  };
  if (!result.active) return result;

  const matches = (card: DeckCard) => card.name.toLowerCase().includes(query) && matchesTags(card.tag_ids, tags);
  const tagsByCard = new Map(cards.map((card) => [card.oracle_id, card.tag_ids]));
  for (const card of [...layout.commanders, ...layout.companions, ...layout.groups.flatMap((group) => group.cards), ...layout.maybe]) {
    if (matches(card)) result.cardIds.add(card.oracle_id);
  }
  for (const adds of addsByGroup.values()) {
    for (const item of adds) {
      if (item.card_name.toLowerCase().includes(query) && matchesTags(tagsByCard.get(item.oracle_id) ?? [], tags)) {
        result.addIds.add(item.id);
      }
    }
  }
  return result;
}
