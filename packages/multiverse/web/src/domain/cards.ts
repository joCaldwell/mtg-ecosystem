// Small facts about a card, read off its type line and stats. Nothing here
// knows what a card is called or what it says (ARCHITECTURE.md: card facts
// come from the API).

// Display order for card-type grouping. `primaryType` walks this list, so a
// card with several types files under the first one that matches — an Artifact
// Creature is a Creature. Land is the exception and wins outright, because the
// header's land count is "any type line containing Land" (the server's
// computeState) and the two numbers have to agree.
export const TYPE_GROUPS: readonly string[] = [
  "Creature",
  "Planeswalker",
  "Battle",
  "Instant",
  "Sorcery",
  "Artifact",
  "Enchantment",
  "Land",
];

export function isLand(card: { type_line: string }): boolean {
  return /\bLand\b/.test(card.type_line);
}

export function primaryType(typeLine: string): string {
  if (isLand({ type_line: typeLine })) return "Land";
  return TYPE_GROUPS.find((t) => new RegExp(`\\b${t}\\b`).test(typeLine)) ?? "Other";
}

/** The corner stat, printed the way the card prints it: power/toughness, or
 *  loyalty in brackets. Empty for cards that have neither. */
export function ptString(card: {
  power: string | null;
  toughness: string | null;
  loyalty: string | null;
}): string {
  if (card.power != null && card.toughness != null) return `${card.power}/${card.toughness}`;
  if (card.loyalty != null) return `[${card.loyalty}]`;
  return "";
}

/** Mana value, then name: the order a list reads as a curve. */
export function byCurve<T extends { cmc: number; name: string }>(a: T, b: T): number {
  return a.cmc - b.cmc || a.name.localeCompare(b.name);
}

/** A to Z. */
export function byName<T extends { name: string }>(a: T, b: T): number {
  return a.name.localeCompare(b.name);
}

/** Most expensive first, then name. A card with no price has no place in the order, so it goes last. */
export function byPrice<T extends { cents?: number | null; name: string }>(a: T, b: T): number {
  if (a.cents == null || b.cents == null)
    return Number(a.cents == null) - Number(b.cents == null) || byName(a, b);
  return b.cents - a.cents || byName(a, b);
}

/** How the cards inside a group are ordered. The owner picks; "curve" is the default. */
export type SortBy = "curve" | "name" | "price";

export const CARD_ORDER: Record<SortBy, (a: SortableCard, b: SortableCard) => number> = {
  curve: byCurve,
  name: byName,
  price: byPrice,
};

interface SortableCard {
  cmc: number;
  name: string;
  cents?: number | null;
}

/** Copies, not rows: a group holding six of one basic reads 6. */
export function quantityOf(cards: Array<{ quantity: number }>): number {
  return cards.reduce((n, c) => n + c.quantity, 0);
}
