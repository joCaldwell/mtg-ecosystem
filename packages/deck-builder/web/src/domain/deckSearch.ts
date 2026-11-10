import type { DeckCard, ProposalItem } from "../api/client.ts";
import { quantityOf } from "./cards.ts";
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
}

/** Identify matches. The layout itself is never changed: totals, targets and
 *  checks keep describing the whole deck; the list hides what does not match. */
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
  const visitCards = (list: DeckCard[]) => {
    for (const card of list) if (matches(card)) result.cardIds.add(card.oracle_id);
  };
  visitCards(layout.commanders);
  for (const group of layout.groups) visitCards(group.cards);
  visitCards(layout.companions);
  visitCards(layout.maybe);
  for (const adds of addsByGroup.values()) {
    for (const item of adds) {
      if (item.card_name.toLowerCase().includes(query) && matchesTags(tagsByCard.get(item.oracle_id) ?? [], tags)) result.addIds.add(item.id);
    }
  }
  return result;
}

/** How many rows a filter leaves in the list: each matching card once, and
 *  each matching proposed add while reviewing. */
export function matchCount(matches: DeckFilterMatches): number {
  return matches.cardIds.size + matches.addIds.size;
}

export interface GroupRows {
  cards: DeckCard[];
  adds: ProposalItem[];
}

/** What one group draws while a filter is active: only its matching rows, and
 *  `matched`, the copies among them, for the header's "2 of 14". While
 *  reviewing, the list shows the deck as it would be, so a matching add
 *  counts (one copy, as in reviewOverlay) and a matching cut does not.
 *  Secondary and command-zone contributors are left out; each of those cards
 *  is drawn in its own primary group (or the commander's) when it matches.
 *  `null` means the group has nothing to show and is hidden. */
export function filterGroup(rows: GroupRows, matches: DeckFilterMatches, cuts?: ReadonlySet<string>): (GroupRows & { matched: number }) | null {
  const cards = rows.cards.filter((card) => matches.cardIds.has(card.oracle_id));
  const adds = rows.adds.filter((item) => matches.addIds.has(item.id));
  if (!cards.length && !adds.length) return null;
  return { cards, adds, matched: quantityOf(cards.filter((card) => !cuts?.has(card.oracle_id))) + adds.length };
}

/** The list's one line when a filter leaves nothing. */
export function noMatchesLine(searchTerm: string, tagsActive: boolean): string {
  const query = searchTerm.trim();
  if (!query) return "No cards in this deck match these tags.";
  return `No cards in this deck match “${query}”${tagsActive ? " with these tags" : ""}.`;
}
