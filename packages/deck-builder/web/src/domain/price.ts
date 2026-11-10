import type { Currency, Store } from "../api/client.ts";

// Prices arrive as whole cents (spec §4.4) so that totals are exact; this is
// the one place they become text.

export function formatPrice(cents: number, currency: Currency = "USD"): string {
  return `${currency === "EUR" ? "€" : "$"}${(cents / 100).toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

/** The sentence that goes with a total, for a tooltip or a caption. */
export function costCaption(cost: { unpriced: number; as_of: string | null; store?: Store; currency?: Currency }): string {
  const parts = ["Every card at its cheapest printing"];
  if (cost.store) parts.push(`${cost.store === "tcgplayer" ? "TCGplayer" : "Cardmarket"} (${cost.currency})`);
  if (cost.as_of) parts.push(`Scryfall prices of ${cost.as_of.slice(0, 10)}`);
  if (cost.unpriced)
    parts.push(`${cost.unpriced} ${cost.unpriced === 1 ? "card has" : "cards have"} no price and ${cost.unpriced === 1 ? "is" : "are"} not counted`);
  return `${parts.join(". ")}.`;
}

export interface GroupCost {
  currency?: Currency;
  cents: number;
  /** Cards (not copies) that have a price, and that do not. */
  priced: number;
  unpriced: number;
}

/** What a group of rows costs, copies counted. Cards with no price are left out and counted. */
export function costOf(cards: Array<{ cents?: number | null; quantity: number; currency?: Currency }>): GroupCost {
  const cost: GroupCost = { currency: cards[0]?.currency, cents: 0, priced: 0, unpriced: 0 };
  for (const c of cards) {
    if (c.cents == null) cost.unpriced++;
    else {
      cost.priced++;
      cost.cents += c.cents * c.quantity;
    }
  }
  return cost;
}

/** The tooltip for a group's total. */
export function groupCostCaption(cost: GroupCost): string {
  const base = "This group, every card at its cheapest printing";
  if (!cost.unpriced) return `${base}.`;
  return `${base}. ${cost.unpriced} ${cost.unpriced === 1 ? "card has" : "cards have"} no price and ${cost.unpriced === 1 ? "is" : "are"} not counted.`;
}
