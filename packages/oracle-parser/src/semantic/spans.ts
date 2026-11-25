// The span certificate: every printed token of an ability block, attributed
// to the AST field it produced. Segments are UTF-8 byte ranges of the block's
// normalized text (`lines[].text`, modal bullets and merged paragraphs joined
// by "\n"). They are sorted, disjoint and separated only by whitespace; each
// names block-relative JSON pointers. Every top-level sentence `/effects/i`
// has a segment of its own, and a clause the parser folded into an earlier
// sentence (a regeneration rider, a mana-spending rule) names the exact field
// it completed. The Rust compiler checks this arithmetic against the archived
// text and AST instead of counting printed punctuation.
// Ability words and flavor words (CR 207.2c-d) are `/abilityWord`; modal
// bullet marks are the only trivia (CR 700.2).
import type { Ability, ParsedLine } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { blockSource } from "../parser/block-sources.ts";
import { changed, sentenceSpans } from "../parser/sentence-spans.ts";

export type SpanSegment = { start: number; end: number } & ({ trivia: "bullet" } | { fields: string[] });
export interface SpanCertificate { bytes: number; segments: SpanSegment[] }
export type BlockSpans = { certificate: SpanCertificate } | { unsupported: string };

const NON_CLAUSE_KEYS = new Set(["kind", "effects", "abilityWord"]);
const escape = (key: string) => key.replace(/~/g, "~0").replace(/\//g, "~1");

/** Every pointer of an `options` array with exactly `count` entries. */
function optionsPointers(value: unknown, count: number, path = ""): string[] {
  if (Array.isArray(value)) return value.flatMap((item, index) => optionsPointers(item, count, `${path}/${index}`));
  if (!value || typeof value !== "object") return [];
  return Object.entries(value).flatMap(([key, item]) => [
    ...(key === "options" && Array.isArray(item) && item.length === count ? [`${path}/options`] : []),
    ...(key === "kind" && path !== "" ? [] : optionsPointers(item, count, `${path}/${escape(key)}`)),
  ]);
}

/** The options field the bullets print: each bullet is one whole option
 * (CR 700.2), so an options array inside another candidate lies within a
 * single bullet and is not it. */
function optionsPointer(value: unknown, count: number): string[] {
  const all = optionsPointers(value, count);
  return all.filter(pointer => !all.some(outer => outer !== pointer && pointer.startsWith(`${outer}/`)));
}

export function blockSpans(line: ParsedLine): BlockSpans {
  if (!line.ok) return { unsupported: "unparsed block" };
  const source = blockSource(line);
  if (!source || !source.lines.length) return { unsupported: "block has no recorded source tokens" };
  if (source.lines.map(l => l.text).join("\n") !== line.text) return { unsupported: "recorded source lines differ from the block text" };
  const ability: Ability = line.ability;
  const owner = new Map<Token, string>();
  const fields = (...pointers: string[]) => JSON.stringify(pointers);
  const header = source.lines.find(l => l.role === "header");
  if (!header || source.lines[0] !== header) return { unsupported: "block has no header line" };

  // CR 207.2c-d: "Label — " opens the block. An unspaced dash is never an
  // ability word: it separates a keyword from a nonmana cost (CR 702.47a
  // "Splice onto Arcane—An opponent gains 5 life"), so it stays unsupported.
  const label = "abilityWord" in ability ? ability.abilityWord : undefined;
  if (label !== undefined) {
    const dash = (from: number) => header.text.startsWith(`${label} — `, from) ? label.length + 2 : undefined;
    // First, or right after a chapter's numerals (CR 714.2b: "I — Label — ").
    const at = dash(0) !== undefined ? 0
      : "chapters" in ability && /^[IVX]+(?:, [IVX]+)* — /.exec(header.text)?.[0].length;
    const length = typeof at === "number" ? dash(at) : undefined;
    if (typeof at !== "number" || length === undefined) return { unsupported: "ability word is not printed first" };
    for (const token of header.tokens)
      if (token.provenance!.start >= at && token.provenance!.end <= at + length) owner.set(token, fields("/abilityWord"));
  }
  const byIndex = new Map(header.tokens.map(token => [`${token.provenance!.sourceId}#${token.provenance!.index}`, token]));
  const effects = "effects" in ability && Array.isArray(ability.effects) ? ability.effects : undefined;
  // Prefix (trigger, costs) and suffix (activation rules, limits) clauses
  // belong to the block's remaining fields.
  const rest = Object.entries(ability).filter(([key, value]) => value !== undefined && !NON_CLAUSE_KEYS.has(key)).map(([key]) => `/${escape(key)}`);
  const spans = effects && sentenceSpans(effects);
  if (effects && !spans) {
    // One sentence and no other clause field: every header token that is
    // not the label can only have produced that sentence.
    if (effects.length !== 1 || rest.length) return { unsupported: "sentences were not recorded clause by clause" };
    for (const token of header.tokens) if (!owner.has(token)) owner.set(token, fields("/effects/0"));
  }
  if (spans) {
    for (const span of spans) {
      if (!span.pointers.length) return { unsupported: "a clause changed no AST field" };
      for (const index of span.tokens) {
        const token = byIndex.get(`${span.sourceId}#${index}`);
        if (!token) return { unsupported: "a clause consumed tokens outside its header line" };
        owner.set(token, fields(...span.pointers.map(p => `/effects${p}`)));
      }
    }
  }
  const ordered = [...header.tokens].sort((a, b) => a.provenance!.start - b.provenance!.start);
  const owned = ordered.flatMap((token, index) => owner.has(token) && owner.get(token) !== fields("/abilityWord") ? [index] : []);
  for (const [index, token] of ordered.entries()) {
    if (owner.has(token)) continue;
    if (!effects) { owner.set(token, fields("")); continue; }
    if (owned.length && index > owned[0] && index < owned[owned.length - 1]) return { unsupported: "unattributed tokens between clauses" };
    if (!rest.length) return { unsupported: "printed text outside the sentences has no AST field" };
    owner.set(token, fields(...rest));
  }

  const options = source.lines.filter(l => l.role === "option");
  if (options.length) {
    const pointers = optionsPointer(ability, options.length);
    if (pointers.length !== 1) return { unsupported: "modal bullets have no unique options field" };
    options.forEach((option, index) => option.tokens.forEach((token, position) => {
      const mark = position === 0 && token.kind === "punct" && (token.value === "•" || token.value === "+");
      owner.set(token, mark ? "bullet" : fields(`${pointers[0]}/${index}`));
    }));
  }
  const paragraph = source.lines.find(l => l.role === "paragraph");
  if (paragraph) {
    const pointer = source.premerge ? changed(source.premerge, ability) : null;
    if (pointer === null) return { unsupported: "merged paragraph changed no AST field" };
    for (const token of paragraph.tokens) owner.set(token, fields(pointer));
  }

  // UTF-16 line offsets → UTF-8 bytes of the joined block text.
  const segments: SpanSegment[] = [];
  let base16 = 0;
  const bytesBefore = (text: string, offset: number) => Buffer.byteLength(text.slice(0, offset), "utf8");
  let base8 = 0;
  for (const sourceLine of source.lines) {
    const tokens = [...sourceLine.tokens].sort((a, b) => a.provenance!.start - b.provenance!.start);
    for (const token of tokens) {
      const key = owner.get(token);
      if (key === undefined) return { unsupported: "unattributed token" };
      const start = base8 + bytesBefore(sourceLine.text, token.provenance!.start);
      const end = base8 + bytesBefore(sourceLine.text, token.provenance!.end);
      const last = segments.at(-1);
      const same = last && (key === "bullet" ? "trivia" in last : "fields" in last && JSON.stringify(last.fields) === key);
      if (last && same && key !== "bullet" && last.end <= start) { last.end = Math.max(last.end, end); continue; }
      if (last && last.end > start) return { unsupported: "overlapping clause spans" };
      segments.push(key === "bullet" ? { start, end, trivia: "bullet" } : { start, end, fields: JSON.parse(key) as string[] });
    }
    base16 += sourceLine.text.length + 1;
    base8 += Buffer.byteLength(sourceLine.text, "utf8") + 1;
  }
  void base16;
  return { certificate: { bytes: Buffer.byteLength(line.text, "utf8"), segments } };
}
