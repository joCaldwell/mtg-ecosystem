// Referring to cards (spec §4.3) and linting prose that mentions them (§10.5).
//
// Resolution is exact, never fuzzy: a fuzzy match turns a visible mistake
// into a silent wrong answer. A reference that resolves to nothing fails the
// whole call with suggestions the caller must explicitly pick from.

import type { Database } from "./sqlite.ts";
import { AppError } from "./errors.ts";
import { normalizeName } from "./db.ts";
import type { ResolvedPrice } from "./prices.ts";

export interface CardRow {
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
  ci_mask: number;
  commander_legality: string;
  is_commander: number;
  /** Global resolution has no personal price. Front doors enrich through
   * priceResolver(db, actor); null then means this store has no price (§4.4). */
  cents?: number | null;
  currency?: ResolvedPrice["currency"];
  store?: ResolvedPrice["store"];
  as_of?: string | null;
  image_small: string | null;
  image_normal: string | null;
  image_art_crop: string | null;
  retired: number;
}

export const CARD_COLUMNS =
  "oracle_id, name, mana_cost, cmc, type_line, oracle_text, power, toughness, loyalty, " +
  `color_identity, ci_mask, commander_legality, is_commander, image_small, image_normal, image_art_crop, retired`;

export interface ResolutionFailure {
  ref: string;
  /** "unknown": nothing by that name. "ambiguous": several cards share it. */
  problem: "unknown" | "ambiguous";
  suggestions: string[];
  candidates: Array<{ name: string; oracle_id: string }>;
}

export class CardResolutionError extends AppError {
  failures: ResolutionFailure[];
  constructor(failures: ResolutionFailure[]) {
    super(describeFailures(failures), 422);
    this.failures = failures;
  }
}

function describeFailures(failures: ResolutionFailure[]): string {
  const lines = failures.map((f) => {
    if (f.problem === "ambiguous")
      return (
        `"${f.ref}" matches more than one card. Pass the oracle_id of the one you mean: ` +
        f.candidates.map((c) => `${c.name} (${c.oracle_id})`).join("; ")
      );
    return f.suggestions.length
      ? `"${f.ref}" is not a card name. Did you mean: ${f.suggestions.join("; ")}? ` +
          `Use one only if it is the card you meant; otherwise search for it.`
      : `"${f.ref}" is not a card name and nothing similar exists. Search for the card first.`;
  });
  return (
    `${failures.length === 1 ? "A card reference" : `${failures.length} card references`} did not resolve, ` +
    `so nothing was changed.\n` +
    lines.map((l) => `- ${l}`).join("\n")
  );
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function byId(db: Database, oracleId: string): CardRow | undefined {
  return db.prepare(`SELECT ${CARD_COLUMNS} FROM cards WHERE oracle_id = ?`).get(oracleId) as
    | CardRow
    | undefined;
}

/** Every card whose full name or any face name is exactly this. */
export function cardsNamed(db: Database, name: string): CardRow[] {
  return db
    .prepare(
      `SELECT ${CARD_COLUMNS} FROM cards
       WHERE oracle_id IN (SELECT oracle_id FROM card_names WHERE name_norm = ?) ORDER BY name`,
    )
    .all(normalizeName(name)) as unknown as CardRow[];
}

// One reference to one card, or a description of why not. A reference is an
// exact name or an oracle_id.
//
// Face names share the lookup table with full names, so a staple can look
// ambiguous purely because some double-faced card has a face with the same
// name. The card actually *called* that wins (spec §4.3); what remains
// ambiguous after that genuinely is, and needs an oracle_id.
function resolveOne(db: Database, ref: string): CardRow | ResolutionFailure {
  const trimmed = ref.trim();
  if (!trimmed) return { ref, problem: "unknown", suggestions: [], candidates: [] };
  // Test fixtures use short ids, so an id lookup is tried for anything that
  // could be one rather than only for well-formed UUIDs.
  if (UUID.test(trimmed) || !/\s/.test(trimmed)) {
    const hit = byId(db, trimmed);
    if (hit) return hit;
  }
  const matches = cardsNamed(db, trimmed);
  if (matches.length === 1) return matches[0];
  if (matches.length > 1) {
    const full = matches.filter((m) => normalizeName(m.name) === normalizeName(trimmed));
    if (full.length === 1) return full[0];
    return {
      ref,
      problem: "ambiguous",
      suggestions: [],
      candidates: (full.length ? full : matches).map((m) => ({ name: m.name, oracle_id: m.oracle_id })),
    };
  }
  return { ref, problem: "unknown", suggestions: suggestNames(db, trimmed), candidates: [] };
}

/** Resolve one reference or throw. */
export function resolveCard(db: Database, ref: string): CardRow {
  return resolveCards(db, [ref])[0];
}

/** Resolve every reference or throw once, naming all the failures. */
export function resolveCards(db: Database, refs: string[]): CardRow[] {
  const results = refs.map((r) => resolveOne(db, r));
  const failures = results.filter((r): r is ResolutionFailure => "problem" in r);
  if (failures.length) throw new CardResolutionError(failures);
  return results as CardRow[];
}

/** The non-throwing form, for callers that report failures line by line. */
export function tryResolveCard(db: Database, ref: string): CardRow | ResolutionFailure {
  return resolveOne(db, ref);
}

export function requireCardById(db: Database, oracleId: string): CardRow {
  const row = byId(db, oracleId);
  if (!row) throw new AppError(`Unknown oracle_id ${oracleId}`, 404);
  return row;
}

// ---------- suggestions ----------

function editDistance(a: string, b: string): number {
  const prev = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    let diag = prev[0];
    prev[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const tmp = prev[j];
      prev[j] = Math.min(prev[j] + 1, prev[j - 1] + 1, diag + (a[i - 1] === b[j - 1] ? 0 : 1));
      diag = tmp;
    }
  }
  return prev[b.length];
}

// "Did you mean" candidates for a name that resolved to nothing. Candidates
// come from the full-text index (any word, as a prefix) and from names that
// share the opening letters; they are then ordered by edit distance to what
// was typed, because the index's own rank prefers short documents over close
// spellings. These are offered, never applied.
export function suggestNames(db: Database, name: string, limit = 5): string[] {
  const norm = normalizeName(name);
  const terms = norm
    .split(/\s+/)
    .map((t) => t.replace(/[^\p{L}\p{N}]/gu, ""))
    .filter((t) => t.length >= 2);
  const candidates = new Set<string>();
  if (terms.length) {
    // Each term both whole and cut short, so a typo at the end of a word
    // ("Ringz") still finds the word it was meant to be.
    const prefixes = new Set<string>();
    for (const t of terms) {
      prefixes.add(t);
      if (t.length >= 4) prefixes.add(t.slice(0, Math.max(3, t.length - 2)));
    }
    const query = [...prefixes].map((t) => `name : "${t}"*`).join(" OR ");
    try {
      for (const r of db
        .prepare("SELECT name FROM cards_fts WHERE cards_fts MATCH ? LIMIT 300")
        .all(query) as unknown as { name: string }[])
        candidates.add(r.name);
    } catch {
      /* a query the index cannot parse yields no suggestions */
    }
  }
  const lead = norm.slice(0, 3);
  if (lead.length >= 2)
    for (const r of db
      .prepare("SELECT name FROM cards WHERE name LIKE ? ESCAPE '\\' LIMIT 200")
      .all(`${lead.replace(/[%_\\]/g, "\\$&")}%`) as unknown as { name: string }[])
      candidates.add(r.name);

  return [...candidates]
    .map((c) => ({ name: c, d: closeness(norm, c) }))
    .sort((a, b) => a.d - b.d || a.name.localeCompare(b.name))
    .slice(0, limit)
    .map((c) => c.name);
}

// Distance to the nearer of the full name and its first face, so a typo of a
// double-faced card's front name is not penalized for the half left out.
function closeness(norm: string, candidate: string): number {
  const full = normalizeName(candidate);
  const front = normalizeName(candidate.split(" // ")[0]);
  return Math.min(editDistance(norm, full), editDistance(norm, front));
}

// ---------- prose linting (spec §10.5) ----------

export function extractCardRefs(text: string): string[] {
  const refs: string[] = [];
  for (const m of text.matchAll(/\[\[([^\[\]]+)\]\]/g)) refs.push(m[1].trim());
  return [...new Set(refs)];
}

// Everything an agent stores as prose writes cards as [[Card Name]]. Each one
// must name a real card or the write fails, so nothing the app displays names
// a card that does not exist. A name shared by several cards passes: it
// exists, and prose has no way to carry an oracle_id. Returns the cards that
// resolved to exactly one card.
export function lintProse(db: Database, text: string): CardRow[] {
  const results = extractCardRefs(text).map((r) => resolveOne(db, r));
  const unknown = results.filter(
    (r): r is ResolutionFailure => "problem" in r && r.problem === "unknown",
  );
  if (unknown.length) throw new CardResolutionError(unknown);
  const cards = new Map<string, CardRow>();
  for (const r of results) if (!("problem" in r)) cards.set(r.oracle_id, r);
  return [...cards.values()];
}
