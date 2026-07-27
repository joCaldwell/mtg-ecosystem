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
  // identity are excluded before the query runs (spec §6.3).
  colorIdentityMask?: number;
  // Deck context: enables slot:/tag: filters, and (unless disabled) applies
  // the deck's own color identity as the pre-filter.
  deckId?: number;
  filterByDeckIdentity?: boolean;
  // Exclude cards the user hard-filtered for this deck (spec §7.2) — the
  // agent's searches always set this; the UI leaves them visible.
  excludeHardFilters?: boolean;
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
  if (opts.excludeHardFilters && opts.deckId !== undefined) {
    clauses.push(
      `oracle_id NOT IN (SELECT oracle_id FROM hard_filters WHERE deck_id = ${Number(opts.deckId)})`,
    );
  }
  const limit = opts.limit ?? 50;
  const stmt = db.prepare(
    `SELECT ${RESULT_COLUMNS} FROM cards WHERE ${clauses.join(" AND ")} ORDER BY name LIMIT ?`,
  );
  return stmt.all(...params, limit) as unknown as SearchResult[];
}

// Exact-match [[Card Name]] resolution (spec §6.4). Returns matches on the
// full name or any face name. Never fuzzy.
export function resolveExactName(db: DatabaseSync, name: string): SearchResult[] {
  const norm = name.trim().toLowerCase().normalize("NFC");
  return db
    .prepare(
      `SELECT ${RESULT_COLUMNS} FROM cards WHERE oracle_id IN (SELECT oracle_id FROM card_names WHERE name_norm = ?)`,
    )
    .all(norm) as unknown as SearchResult[];
}

// "Did you mean" suggestions for a failed exact resolution — surfaced to the
// agent as candidates it must explicitly re-resolve, never auto-applied.
export function suggestNames(db: DatabaseSync, name: string, limit = 5): string[] {
  const terms = name
    .split(/\s+/)
    .map((t) => t.replace(/[^\p{L}\p{N}]/gu, ""))
    .filter(Boolean);
  if (!terms.length) return [];
  const query = terms.map((t) => `"${t}"*`).join(" OR ");
  try {
    const rows = db
      .prepare("SELECT name FROM cards_fts WHERE cards_fts MATCH ? ORDER BY rank LIMIT ?")
      .all(query, limit) as unknown as { name: string }[];
    return rows.map((r) => r.name);
  } catch {
    return [];
  }
}
