// Refreshing card data (spec §4.1, §4.4): the cards themselves, then what
// each costs. One operation, because the owner means one thing by it — "make
// the app's card data current" — whether they run the command or press the
// button in Settings.

import type { Database } from "./sqlite.ts";
import { withTransaction } from "./db.ts";
import { stageTokenCatalog, tokenStatus } from "./token-catalog.ts";
import { ingestFromFile } from "./ingest.ts";
import { PRICE_STORES_VERSION, cheapestPrintings, ingestPrices, priceStatus, pricesAsOf } from "./prices.ts";
import { bulkEntry, ensureBulkFile, streamPrintings, relatedTokenPrintings } from "./scryfall.ts";

export interface RefreshResult {
  cards: number;
  /** Whether the printing stream was republished (also true on token-catalog upgrades). */
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
  const current = !opts.force && !!db.prepare("SELECT 1 FROM meta WHERE key = 'price_stores_version' AND value = ?").get(PRICE_STORES_VERSION) && pricesAsOf(db) === entry.updated_at && tokenStatus(db).ready && tokenStatus(db).as_of === entry.updated_at;
  if (current) {
    console.log(`Prices are current (Scryfall's are from ${entry.updated_at}); skipping download.`);
  } else {
    const mb = entry.compressed_size == null ? "" : ` (${(entry.compressed_size / 1e6).toFixed(0)} MB)`;
    console.log(`Downloading every printing for prices and tokens${mb}...`);
    const staging = stageTokenCatalog(db);
    try {
      async function* observedPrintings() {
        for await (const card of streamPrintings(entry)) { staging.add(card); yield card; }
      }
      const prices = await cheapestPrintings(observedPrintings());
      for await (const card of relatedTokenPrintings(staging.missingPrintingIds())) staging.add(card);
      const result = withTransaction(db, () => {
        const tokens = staging.finish(entry.updated_at);
        return { ...ingestPrices(db, prices, entry.updated_at), ...tokens };
      });
      console.log(`Done: ${result.priced} cards priced; ${result.tokens} token identities and ${result.links} card links.`);
    } finally { staging.close(); }
  }
  const { priced, unpriced } = priceStatus(db);
  return { cards, prices_updated: !current, priced, unpriced, prices_as_of: entry.updated_at };
}
