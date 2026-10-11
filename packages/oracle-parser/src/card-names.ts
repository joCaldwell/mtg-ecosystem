// Literal names come from source data, never a title-case/prose heuristic.
// CR 201.2, 201.4b–f, 201.5 distinguish literal names, individual face names,
// and a source object's self-reference. Token names are separate (111.4).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
export interface CardNameSource {
  name: string;
  layout: string;
  oracle_id?: string | null;
  card_faces?: { name: string; oracle_id?: string | null }[] | null;
}
export type CatalogNameKind = "card" | "token" | "other" | "combined-face-label";
export interface CatalogNameSource {
  sourceIndex: number;
  sourceFaceIndex?: number;
  kind: CatalogNameKind;
  oracleId?: string;
}
export interface CatalogNameMatch {
  /** Canonical source spelling, not a reconstructed sequence of word tokens. */
  name: string;
  end: number;
  sources: readonly CatalogNameSource[];
}
interface Trie { children: Map<string, Trie>; names?: Map<string, CatalogNameSource[]> }

/** Exactly the punctuation transformations applied to Oracle input. */
export function canonicalizeNamePunctuation(value: string): string {
  return value.replace(/’/g, "'").replace(/[“”]/g, '"').replace(/−/g, "-").replace(/–/g, "—");
}

/** Immutable after construction. Lookup returns possible literal spans; only
 * a grammar production may decide that the span is actually a name. */
export class CardNameCatalog {
  private readonly trie: Trie = { children: new Map() };
  readonly size: number;

  constructor(records: readonly CardNameSource[]) {
    let size = 0;
    const add = (name: string, source: CatalogNameSource) => {
      if (typeof name !== "string" || !name.trim() || /[\r\n]/.test(name)) throw new Error("Invalid catalog name");
      let node = this.trie;
      for (const character of canonicalizeNamePunctuation(name)) {
        let child = node.children.get(character);
        if (!child) { child = { children: new Map() }; node.children.set(character, child); }
        node = child;
      }
      node.names ??= new Map();
      const sources = node.names.get(name);
      if (sources) sources.push(Object.freeze(source));
      else { node.names.set(name, [Object.freeze(source)]); size++; }
    };
    records.forEach((record, sourceIndex) => {
      if (!record || typeof record.layout !== "string") throw new Error("Invalid catalog source");
      const kind = ["token", "double_faced_token"].includes(record.layout) ? "token"
        : ["art_series", "emblem"].includes(record.layout) ? "other" : "card";
      add(record.name, { sourceIndex, kind: record.card_faces?.length ? "combined-face-label" : kind,
        ...(record.oracle_id ? { oracleId: record.oracle_id } : {}) });
      record.card_faces?.forEach((face, sourceFaceIndex) => add(face.name, { sourceIndex, sourceFaceIndex, kind,
        ...(face.oracle_id ?? record.oracle_id ? { oracleId: (face.oracle_id ?? record.oracle_id)! } : {}) }));
    });
    this.size = size;
  }

  /** Longest-first, exact-case, Unicode-bounded. The input must use the same
   * canonical punctuation as Oracle normalization; no fuzzy name matching. */
  matches(text: string, start: number, kinds?: readonly CatalogNameKind[]): CatalogNameMatch[] {
    if (!Number.isInteger(start) || start < 0 || start > text.length || /(?:\p{L}|\p{M}|\p{N}|_)$/u.test(text.slice(0, start))) return [];
    const matches: CatalogNameMatch[] = [];
    let node = this.trie;
    let end = start;
    for (const character of text.slice(start)) {
      const next = node.children.get(character);
      if (!next) break;
      node = next;
      end += character.length;
      if (/^(?:\p{L}|\p{M}|\p{N}|_)/u.test(text.slice(end)) || !node.names) continue;
      for (const [name, allSources] of node.names) {
        const sources = allSources.filter(source => !kinds || kinds.includes(source.kind));
        if (sources.length) matches.push(Object.freeze({ name, end, sources: Object.freeze(sources) }));
      }
    }
    return matches.sort((a, b) => b.end - a.end || a.name.localeCompare(b.name));
  }

  exact(value: string, kinds?: readonly CatalogNameKind[]): CatalogNameMatch | undefined {
    const canonical = canonicalizeNamePunctuation(value);
    const matches = this.matches(canonical, 0, kinds).filter(match => match.end === canonical.length);
    // Distinct source names may collapse under punctuation normalization.
    // Never choose one source spelling arbitrarily in that case.
    return matches.length === 1 ? matches[0] : undefined;
  }
}

export function buildCardNameCatalog(records: readonly CardNameSource[]): CardNameCatalog {
  return new CardNameCatalog(records);
}
