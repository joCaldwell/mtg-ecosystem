// Card prices (spec §4.4): one estimate per card, in USD — what its cheapest
// playable paper printing costs.
//
// A deck holds cards, not printings (spec §4.1), so "what does this card
// cost" needs a rule for which printing is meant. Scryfall's oracle-cards
// file prices one representative printing per card, chosen for recognisability
// rather than cost, so the price comes from the default-cards file instead:
// every printing, reduced here to the cheapest one per card.
//
// Sources, read 2026-10-11:
//   https://scryfall.com/docs/api/cards — `prices` holds "daily price
//     information … including usd, usd_foil, usd_etched, eur, eur_foil,
//     eur_etched, and tix prices, as strings"; `games`, `oversized`,
//     `set_type` and `finishes` are per printing.
//   https://scryfall.com/docs/api/bulk-data — "prices should be considered
//     dangerously stale after 24 hours. Only use bulk price information to
//     track trends or provide a general estimate of card value."
//   https://scryfall.com/docs/api/sets — set_type `memorabilia`: "A set made
//     up of gold-bordered, oversize, or trophy cards that are not legal".

import type { DatabaseSync } from "node:sqlite";
import { withTransaction } from "./db.ts";

export type Finish = "nonfoil" | "foil" | "etched";

/** The fields of a Scryfall card object (one printing) that pricing reads. */
export interface ScryfallPrinting {
  /** Absent on reversible cards, which carry it on each face instead. */
  oracle_id?: string;
  set: string;
  set_type?: string;
  collector_number: string;
  games?: string[];
  oversized?: boolean;
  prices?: { usd?: string | null; usd_foil?: string | null; usd_etched?: string | null };
  card_faces?: Array<{ oracle_id?: string }>;
}

export interface CheapestPrice {
  usd_cents: number;
  set_code: string;
  collector_number: string;
  finish: Finish;
}

const PRICE_FIELDS: Array<[keyof NonNullable<ScryfallPrinting["prices"]>, Finish]> = [
  ["usd", "nonfoil"],
  ["usd_foil", "foil"],
  ["usd_etched", "etched"],
];

// A printing counts only if a deck could play it: a paper card, of normal
// size, from a set of legal cards. Without the last two the "cheapest
// printing" of a Reserved List card is its gold-bordered World Championship
// copy or its Collectors' Edition one, at a fraction of the real price.
// Scryfall reports legality per card, not per printing, so `legalities` says
// "legal" on those too; the set type is what tells them apart.
function isPlayablePaper(p: ScryfallPrinting): boolean {
  return (p.games ?? []).includes("paper") && !p.oversized && p.set_type !== "memorabilia";
}

function toCents(price: string | null | undefined): number | null {
  if (price == null || !/^\d+(\.\d+)?$/.test(price)) return null;
  return Math.round(Number(price) * 100);
}

/** Reduce every printing to the cheapest one per card, whatever its finish. */
export async function cheapestPrintings(
  printings: Iterable<ScryfallPrinting> | AsyncIterable<ScryfallPrinting>,
): Promise<Map<string, CheapestPrice>> {
  const cheapest = new Map<string, CheapestPrice>();
  for await (const p of printings) {
    if (!isPlayablePaper(p)) continue;
    let best: CheapestPrice | null = null;
    for (const [field, finish] of PRICE_FIELDS) {
      const cents = toCents(p.prices?.[field]);
      if (cents != null && (best == null || cents < best.usd_cents))
        best = { usd_cents: cents, set_code: p.set, collector_number: p.collector_number, finish };
    }
    if (!best) continue;
    // One physical reversible card is a copy of the card on each of its faces.
    const ids = new Set([p.oracle_id, ...(p.card_faces ?? []).map((f) => f.oracle_id)]);
    for (const id of ids) {
      if (!id) continue;
      const held = cheapest.get(id);
      if (!held || isCheaper(best, held)) cheapest.set(id, best);
    }
  }
  return cheapest;
}

// Ties go to the same printing every time, so a refresh that changed no price
// changes no row.
function isCheaper(a: CheapestPrice, b: CheapestPrice): boolean {
  if (a.usd_cents !== b.usd_cents) return a.usd_cents < b.usd_cents;
  if (a.set_code !== b.set_code) return a.set_code < b.set_code;
  return a.collector_number < b.collector_number;
}

// Prices are replaced wholesale: a card that has lost its price is unpriced,
// not stuck at the last one seen. The table deliberately does not reference
// `cards` — it keeps a price for every card Scryfall prices, so a card that
// enters the database later is priced without downloading the printings again.
export function ingestPrices(
  db: DatabaseSync,
  prices: Map<string, CheapestPrice>,
  asOf: string,
): { priced: number; unpriced: number } {
  withTransaction(db, () => {
    db.exec("DELETE FROM card_prices");
    const insert = db.prepare(
      "INSERT INTO card_prices (oracle_id, usd_cents, set_code, collector_number, finish) VALUES (?, ?, ?, ?, ?)",
    );
    for (const [oracleId, p] of prices)
      insert.run(oracleId, p.usd_cents, p.set_code, p.collector_number, p.finish);
    db.prepare(
      "INSERT INTO meta (key, value) VALUES ('prices_as_of', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    ).run(asOf);
  });
  const status = priceStatus(db);
  return { priced: status.priced, unpriced: status.unpriced };
}

export interface PriceStatus {
  /** When Scryfall generated the prices in use; null until the first refresh. */
  as_of: string | null;
  /** Cards in the database with a price, and without one. */
  priced: number;
  unpriced: number;
}

export function pricesAsOf(db: DatabaseSync): string | null {
  const row = db.prepare("SELECT value FROM meta WHERE key = 'prices_as_of'").get() as
    | { value: string }
    | undefined;
  return row?.value ?? null;
}

export function priceStatus(db: DatabaseSync): PriceStatus {
  const { total, priced } = db
    .prepare(
      `SELECT COUNT(*) AS total, COUNT(p.oracle_id) AS priced
       FROM cards c LEFT JOIN card_prices p ON p.oracle_id = c.oracle_id`,
    )
    .get() as { total: number; priced: number };
  return { as_of: pricesAsOf(db), priced, unpriced: total - priced };
}

/** The price column for a query that selects straight from `cards`. */
export const USD_CENTS_COLUMN =
  "(SELECT usd_cents FROM card_prices WHERE card_prices.oracle_id = cards.oracle_id) AS usd_cents";

export function formatUsd(cents: number): string {
  return `$${(cents / 100).toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

export interface CostEstimate {
  usd_cents: number;
  /** Cards (not copies) with no price, which the total leaves out. */
  unpriced: number;
}

/** What a list of cards costs, copies counted. */
export function estimateCost(cards: Array<{ usd_cents: number | null; quantity: number }>): CostEstimate {
  let cents = 0;
  let unpriced = 0;
  for (const c of cards) {
    if (c.usd_cents == null) unpriced++;
    else cents += c.usd_cents * c.quantity;
  }
  return { usd_cents: cents, unpriced };
}
