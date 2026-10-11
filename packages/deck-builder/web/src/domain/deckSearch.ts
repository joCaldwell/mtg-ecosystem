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

/** Filter visible rows after grouping so deck totals and review decisions stay intact. */
export function filterDeckList(
  layout: DeckLayout,
  addsByGroup: Map<string, ProposalItem[]>,
  searchTerm: string,
  tags: TagFilter = { ids: [], match: "any" },
  /** Proposed adds can inherit tags from a parked card when accepted. */
  cards: DeckCard[] = [],
): { layout: DeckLayout; addsByGroup: Map<string, ProposalItem[]> } {
  const query = searchTerm.trim().toLowerCase();
  if (!query && tags.ids.length === 0) return { layout, addsByGroup };

  const matches = (card: DeckCard) => card.name.toLowerCase().includes(query) && matchesTags(card.tag_ids, tags);
  const tagsByCard = new Map(cards.map((card) => [card.oracle_id, card.tag_ids]));
  const filteredAdds = new Map<string, ProposalItem[]>();
  for (const [key, adds] of addsByGroup) {
    const matching = adds.filter((item) => item.card_name.toLowerCase().includes(query) && matchesTags(tagsByCard.get(item.oracle_id) ?? [], tags));
    if (matching.length) filteredAdds.set(key, matching);
  }

  return {
    layout: {
      commanders: layout.commanders.filter(matches),
      companions: layout.companions.filter(matches),
      groups: layout.groups
        .map((group) => ({ ...group, cards: group.cards.filter(matches) }))
        .filter((group) => group.cards.length > 0 || filteredAdds.has(group.key)),
      maybe: layout.maybe.filter(matches),
    },
    addsByGroup: filteredAdds,
  };
}
