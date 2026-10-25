import { requireDeck } from "../deck/service.ts";
import type { Actor } from "../actor.ts";
import type { Database } from "../sqlite.ts";
import { parse, SearchError } from "./parse.ts";
import { compile } from "./compile.ts";
import { priceResolver, type ResolvedPrice } from "../prices.ts";

export { SearchError };

export interface SearchResult extends ResolvedPrice {
  oracle_id: string;
  name: string;
  mana_cost: string | null;
  cmc: number;
  type_line: string;
  oracle_text: string;
  power: string | null;
  toughness: string | null;
  loyalty: string | null;
  color_identity: string;
  commander_legality: string;
  is_commander: number;
  image_small: string | null;
  image_normal: string | null;
  image_art_crop: string | null;
  retired: number;
}

const RESULT_COLUMNS =
  "oracle_id, name, mana_cost, cmc, type_line, oracle_text, power, toughness, loyalty, color_identity, " +
  `commander_legality, is_commander, image_small, image_normal, image_art_crop, retired`;

export interface SearchOptions {
  limit?: number;
  // Bitmask of the deck's color identity. When set, cards outside this
  // identity are excluded before the query runs (spec §4.2).
  colorIdentityMask?: number;
  // Deck context: enables slot:/tag: filters, and (unless disabled) applies
  // the deck's own color identity as the pre-filter.
  deckId?: number;
  filterByDeckIdentity?: boolean;
}

/** Card facts are global; a viewer without an account searches them too (§18.6),
 * priced USD/lowest. A deck context is the owner's. */
export function search(db: Database, actor: Actor | null, query: string, opts: SearchOptions = {}): SearchResult[] {
  if (opts.deckId !== undefined) requireDeck(db, actor!, opts.deckId);
  const prices = priceResolver(db, actor);
  const { sql, params } = compile(parse(query), { deckId: opts.deckId, prices: { USD: priceResolver(db, actor, "USD").sql("cards.oracle_id"), EUR: priceResolver(db, actor, "EUR").sql("cards.oracle_id") } });
  const clauses = [sql, opts.deckId === undefined ? "retired = 0" :
    "(retired = 0 OR oracle_id IN (SELECT oracle_id FROM deck_cards WHERE deck_id = ?))"];
  if (opts.deckId !== undefined) params.push(opts.deckId);
  let ciMask = opts.colorIdentityMask;
  if (ciMask === undefined && opts.deckId !== undefined && opts.filterByDeckIdentity !== false) {
    const deck = db.prepare("SELECT ci_mask FROM decks WHERE id = ?").get(opts.deckId) as
      | { ci_mask: number | null }
      | undefined;
    if (deck?.ci_mask != null) ciMask = deck.ci_mask;
  }
  if (ciMask !== undefined) {
    clauses.push(`(ci_mask & ~${ciMask}) = 0`);
  }
  const limit = opts.limit ?? 50;
  const stmt = db.prepare(
    `SELECT ${RESULT_COLUMNS} FROM cards WHERE ${clauses.join(" AND ")} ORDER BY name LIMIT ?`,
  );
  return prices.resolve(stmt.all(...params, limit) as unknown as SearchResult[]);
}
