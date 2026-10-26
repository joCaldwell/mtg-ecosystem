// Card prices (spec §4.4): one estimate per card and store — what its cheapest
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

import type { Database } from "./sqlite.ts";
import type { Actor } from "./actor.ts";
import { requireUi } from "./actor.ts";
import { ServiceError } from "./errors.ts";
import { getSetting, setSetting, withTransaction } from "./db.ts";

export const PRICE_STORES_VERSION = "1";

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
  prices?: { usd?: string | null; usd_foil?: string | null; usd_etched?: string | null; eur?: string | null; eur_foil?: string | null; eur_etched?: string | null };
  card_faces?: Array<{ oracle_id?: string }>;
}

export type Currency = "USD" | "EUR";
export type Store = "tcgplayer" | "cardmarket";
export interface PriceSource { currency: Currency; store: Store | "lowest" }
export interface PriceMetadata { currency: Currency; store: Store; as_of: string | null; source: "Scryfall" }
export interface ResolvedPrice extends PriceMetadata { cents: number | null }

export interface CheapestPrice {
  store: Store;
  currency: Currency;
  cents: number;
  set_code: string;
  collector_number: string;
  finish: Finish;
}

// Verified 2026-10-25 against https://scryfall.com/docs/api/cards and
// https://scryfall.com/docs/faqs/where-do-scryfall-prices-come-from-7 .
// USD finishes are TCGplayer market prices; EUR finishes are Cardmarket
// prices (trend/averages as available). eur_etched is optional in bulk data.
const STORES = {
  tcgplayer: { currency: "USD", fields: [["usd", "nonfoil"], ["usd_foil", "foil"], ["usd_etched", "etched"]] },
  cardmarket: { currency: "EUR", fields: [["eur", "nonfoil"], ["eur_foil", "foil"], ["eur_etched", "etched"]] },
} as const;

function validPriceSource(value: PriceSource): boolean {
  return !!value && ["USD", "EUR"].includes(value.currency) &&
    (value.store === "lowest" || (Object.hasOwn(STORES, value.store) && STORES[value.store].currency === value.currency));
}

export function getPriceSource(db: Database, actor: Actor | null): PriceSource {
  if (actor === null) return { currency: "USD", store: "lowest" };
  const source = { currency: getSetting(db, actor, "price_currency") ?? "USD",
    store: getSetting(db, actor, "price_store") ?? "lowest" } as PriceSource;
  return validPriceSource(source) ? source : { currency: "USD", store: "lowest" };
}

export function setPriceSource(db: Database, actor: Actor | null, value: PriceSource): void {
  if (actor === null) throw new ServiceError("Sign in to change the price source", 401);
  requireUi(actor, "change the price source");
  if (!validPriceSource(value))
    throw new ServiceError("Choose USD with TCGplayer or Lowest, or EUR with Cardmarket or Lowest");
  withTransaction(db, () => {
    setSetting(db, actor, "price_currency", value.currency);
    setSetting(db, actor, "price_store", value.store);
  });
}

/** One actor-scoped policy for row prices and currency search (§4.4).
 * Null means anonymous USD/lowest, without reading any user's preferences.
 * SQL column arguments are internal identifiers, never user input. */
export function priceResolver(db: Database, actor: Actor | null, currency?: Currency) {
  return resolver(db, getPriceSource(db, actor), currency);
}

function resolver(db: Database, choice: PriceSource, currency?: Currency) {
  const unit = currency ?? choice.currency;
  const selected = choice.currency === unit ? choice.store : "lowest";
  const stores = (Object.keys(STORES) as Store[]).filter(s => STORES[s].currency === unit && (selected === "lowest" || selected === s));
  const metadata: PriceMetadata = { currency: unit, store: stores[0], as_of: pricesAsOf(db), source: "Scryfall" };
  const sql = (column: string) => `(SELECT cents FROM card_prices WHERE oracle_id = ${column} AND currency = '${unit}' AND store IN (${stores.map(s => `'${s}'`).join(",")}) ORDER BY cents, store LIMIT 1)`;
  const stmt = db.prepare(`SELECT cents, store FROM card_prices WHERE oracle_id = ? AND currency = ? AND store IN (${stores.map(s => `'${s}'`).join(",")}) ORDER BY cents, store LIMIT 1`);
  return { ...metadata, sql, resolve<T extends { oracle_id: string }>(rows: T[]): Array<T & ResolvedPrice> {
    return rows.map(row => {
      const price = stmt.get(row.oracle_id, unit) as {cents: number; store: Store} | undefined;
      return { ...row, ...metadata, cents: price?.cents ?? null, store: price?.store ?? metadata.store };
    });
  } };
}

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
    for (const store of Object.keys(STORES) as Store[]) {
      let best: CheapestPrice | null = null;
      for (const [field, finish] of STORES[store].fields) {
        const cents = toCents(p.prices?.[field]);
        if (cents != null && (best == null || cents < best.cents))
          best = { store, currency: STORES[store].currency, cents, set_code: p.set, collector_number: p.collector_number, finish };
      }
      if (!best) continue;
      const ids = new Set([p.oracle_id, ...(p.card_faces ?? []).map(f => f.oracle_id)]);
      for (const id of ids) {
        if (!id) continue;
        const key = `${id}:${store}`;
        const held = cheapest.get(key);
        if (!held || isCheaper(best, held)) cheapest.set(key, best);
      }
    }
  }
  return cheapest;
}

// Ties go to the same printing every time, so a refresh that changed no price
// changes no row.
function isCheaper(a: CheapestPrice, b: CheapestPrice): boolean {
  if (a.cents !== b.cents) return a.cents < b.cents;
  if (a.set_code !== b.set_code) return a.set_code < b.set_code;
  return a.collector_number < b.collector_number;
}

// Prices are replaced wholesale: a card that has lost its price is unpriced,
// not stuck at the last one seen. The table deliberately does not reference
// `cards` — it keeps a price for every card Scryfall prices, so a card that
// enters the database later is priced without downloading the printings again.
export function ingestPrices(
  db: Database,
  prices: Map<string, CheapestPrice>,
  asOf: string,
): { priced: number; unpriced: number } {
  // Prepare in bounded transactions while readers keep seeing the old table.
  db.exec(`DROP TABLE IF EXISTS card_prices_staging;
    CREATE TABLE card_prices_staging (
      oracle_id TEXT NOT NULL, store TEXT NOT NULL CHECK(store IN ('tcgplayer','cardmarket')),
      currency TEXT NOT NULL CHECK((store = 'tcgplayer' AND currency = 'USD') OR (store = 'cardmarket' AND currency = 'EUR')),
      cents INTEGER NOT NULL CHECK(cents >= 0), set_code TEXT NOT NULL,
      collector_number TEXT NOT NULL, finish TEXT NOT NULL CHECK (finish IN ('nonfoil','foil','etched')), PRIMARY KEY(oracle_id, store)
    )`);
  try {
    const insert = db.prepare("INSERT INTO card_prices_staging VALUES (?, ?, ?, ?, ?, ?, ?)");
    const entries = prices.entries();
    for (;;) {
      const batch: [string, CheapestPrice][] = [];
      for (let i = 0; i < 500; i++) { const next = entries.next(); if (next.done) break; batch.push(next.value); }
      if (!batch.length) break;
      withTransaction(db, () => {
        for (const [oracleId, p] of batch) insert.run(oracleId.slice(0, oracleId.lastIndexOf(":")), p.store, p.currency, p.cents, p.set_code, p.collector_number, p.finish);
      });
    }
    withTransaction(db, () => {
      db.exec(`DROP TABLE card_prices; ALTER TABLE card_prices_staging RENAME TO card_prices;
        CREATE INDEX idx_card_prices_currency ON card_prices(currency, store, cents)`);
      db.prepare("INSERT INTO meta (key, value) VALUES ('prices_as_of', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value").run(asOf);
      db.prepare("INSERT INTO meta(key,value) VALUES ('price_stores_version',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value").run(PRICE_STORES_VERSION);
    });
  } finally { db.exec("DROP TABLE IF EXISTS card_prices_staging"); }
  const status = priceStatus(db);
  return { priced: status.priced, unpriced: status.unpriced };
}

export interface PriceStatus {
  currency: Currency;
  store: Store;
  /** When Scryfall generated the prices in use; null until the first refresh. */
  as_of: string | null;
  /** Cards in the database with a price, and without one. */
  priced: number;
  unpriced: number;
}

export function pricesAsOf(db: Database): string | null {
  const row = db.prepare("SELECT value FROM meta WHERE key = 'prices_as_of'").get() as
    | { value: string }
    | undefined;
  return row?.value ?? null;
}

export function priceStatus(db: Database, store: Store = "tcgplayer"): PriceStatus {
  const { total, priced } = db
    .prepare(
      `SELECT COUNT(*) AS total, COUNT(p.oracle_id) AS priced
       FROM cards c LEFT JOIN card_prices p ON p.oracle_id = c.oracle_id AND p.store = ?`,
    )
    .get(store) as { total: number; priced: number };
  return { currency: STORES[store].currency, store, as_of: pricesAsOf(db), priced, unpriced: total - priced };
}

export function formatPrice(cents: number, currency: Currency = "USD"): string {
  return `${currency === "EUR" ? "€" : "$"}${(cents / 100).toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

export interface CostEstimate {
  cents: number;
  /** Cards (not copies) with no price, which the total leaves out. */
  unpriced: number;
}

/** What a list of cards costs, copies counted. */
export function estimateCost(cards: Array<{ cents: number | null; quantity: number }>): CostEstimate {
  let cents = 0;
  let unpriced = 0;
  for (const c of cards) {
    if (c.cents == null) unpriced++;
    else cents += c.cents * c.quantity;
  }
  return { cents, unpriced };
}
