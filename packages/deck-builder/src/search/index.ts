import type { DatabaseSync } from "node:sqlite";
import { parse, SearchError } from "./parse.ts";
import { compile } from "./compile.ts";

export { SearchError };

export interface SearchResult {
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
}

const RESULT_COLUMNS =
  "oracle_id, name, mana_cost, cmc, type_line, oracle_text, power, toughness, loyalty, color_identity, commander_legality, is_commander";

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

export function search(db: DatabaseSync, query: string, opts: SearchOptions = {}): SearchResult[] {
  const { sql, params } = compile(parse(query), { deckId: opts.deckId });
  const clauses = [sql];
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
  return stmt.all(...params, limit) as unknown as SearchResult[];
}
