// Refreshing card data (spec §4.1, §4.4): the cards themselves, then what
// each costs. One operation, because the owner means one thing by it — "make
// the app's card data current" — whether they run the command or press the
// button in Settings.

import type { Database } from "./sqlite.ts";
import { ingestFromFile } from "./ingest.ts";
import { cheapestPrintings, ingestPrices, priceStatus, pricesAsOf } from "./prices.ts";
import { bulkEntry, ensureBulkFile, streamPrintings } from "./scryfall.ts";

export interface RefreshResult {
  cards: number;
  /** False when Scryfall has published no newer prices than the ones in use. */
  prices_updated: boolean;
  priced: number;
  unpriced: number;
  prices_as_of: string;
}

export async function refreshCardData(
  db: Database,
  opts: { force?: boolean } = {},
): Promise<RefreshResult> {
  const cards = (await ingestFromFile(db, await ensureBulkFile(opts.force))).inserted;

  const entry = await bulkEntry("default_cards");
  const current = !opts.force && pricesAsOf(db) === entry.updated_at;
  if (current) {
    console.log(`Prices are current (Scryfall's are from ${entry.updated_at}); skipping download.`);
  } else {
    const mb = entry.compressed_size == null ? "" : ` (${(entry.compressed_size / 1e6).toFixed(0)} MB)`;
    console.log(`Downloading every printing for prices${mb}...`);
    const result = ingestPrices(db, await cheapestPrintings(streamPrintings(entry)), entry.updated_at);
    console.log(`Done: ${result.priced} cards priced, ${result.unpriced} have no price on any printing.`);
  }
  const { priced, unpriced } = priceStatus(db);
  return { cards, prices_updated: !current, priced, unpriced, prices_as_of: entry.updated_at };
}
