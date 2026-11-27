// Normalization: raw Scryfall Oracle text → physical ability lines.
// Preserve ability boundaries and exact-case, word-bounded self-references.

import { canonicalizeNamePunctuation, type CardNameCatalog } from "./card-names.ts";
import { TrackedText, type EditTag, type TrackedNormalization } from "./printed/text.ts";
import { namesLegendaryCharacter, sourcePronounRewrites } from "./source-pronouns.ts";

type LiteralTag = Extract<EditTag, { edit: "literal-name" }>;
type SelfNameForm = Extract<EditTag, { edit: "self-name" }>["form"];

interface Rewrite {
  start: number;
  end: number;
  replacement: string;
  tag: EditTag;
}

interface ProtectedLiteral {
  name: string;
  tag: LiteralTag;
  quoted: boolean;
}

function escapeRegExp(s: string): string {
  return s.replace(/[\\^$*+?.()|[\]{}]/g, "\\$&");
}

/** Gathers mutations against one pass's input. Apply right-to-left to keep its offsets valid. */
function applyRewrites(text: string, rewrites: Rewrite[], tracked?: TrackedText, mode?: "reminder" | "protect"): string {
  let result = "";
  let end = text.length;
  for (let i = rewrites.length - 1; i >= 0; i--) {
    const rewrite = rewrites[i];
    tracked?.splice(rewrite.start, rewrite.end, rewrite.replacement, rewrite.tag, rewrite.tag.edit === "literal-name" ? "protect" : mode);
    result = rewrite.replacement + text.slice(rewrite.end, end) + result;
    end = rewrite.start;
  }
  return text.slice(0, end) + result;
}

// CR 207.2a. Preserve line boundaries and keep malformed parentheses visible.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function reminderRewrites(text: string): Rewrite[] {
  const rewrites: Rewrite[] = [];
  for (let i = 0; i < text.length; i++) {
    if (text[i] !== "(") continue;
    let depth = 1, end = i + 1;
    while (end < text.length && depth > 0) {
      if (text[end] === "(") depth++;
      else if (text[end] === ")") depth--;
      end++;
    }
    if (depth !== 0) break;
    const newlines = text.slice(i, end).match(/\n/g)?.join("") ?? "";
    rewrites.push({ start: i, end, replacement: newlines || " ", tag: { edit: "gap" } });
    i = end - 1;
  }
  return rewrites;
}

function distinctMarker(text: string, marker: string): string {
  while (text.includes(marker)) marker += "@";
  return marker;
}

// Only explicit grammar slots establish literals. A name lookup cannot extend
// a comma/and list into a following instruction or amount.
export function catalogLiteralSpans(text: string, catalog: CardNameCatalog): (ProtectedLiteral & { start: number; end: number })[] {
  const spans = [];
  for (const match of text.matchAll(/\b(named|partner\s+with|creates?)\s+/gi)) {
    const context = match[1].toLowerCase();
    if (context === "named" && /\bdifferently\s+$/i.test(text.slice(0, match.index))) continue;
    const slot = context.startsWith("partner") ? "partner-with" : context.startsWith("create") ? "create" : "named";
    const start = match.index + match[0].length;
    const kinds = slot === "partner-with" ? ["card" as const] : ["card" as const, "token" as const];
    const candidates = catalog.matches(text, start, kinds);
    let literal = candidates[0]?.end !== candidates[1]?.end ? candidates[0] : undefined;
    let quoted = false;
    if (!literal && text[start] === '"') {
      let end = start + 1;
      while (end < text.length) {
        if (text[end] === "\\") { end += 2; continue; }
        if (text[end] === '"') break;
        end++;
      }
      if (end < text.length) {
        try {
          const value: unknown = JSON.parse(text.slice(start, end + 1));
          const known = typeof value === "string" ? catalog.exact(value, kinds) : undefined;
          if (known) {
            literal = { ...known, end: end + 1 };
            quoted = true;
          }
        } catch { /* Malformed literal remains visible to the lexer. */ }
      }
    }
    if (!literal || (slot === "create" && !/^,\s+(?:a|an)\s+/.test(text.slice(literal.end)))) continue;
    const tag: LiteralTag = { edit: "literal-name", slot, via: "catalog" };
    spans.push({ start, end: literal.end, name: literal.name, tag, quoted });
  }
  return spans;
}

function nameRewrites(
  text: string, name: string, form: SelfNameForm, protect: (name: string, tag: LiteralTag) => string,
): Rewrite[] {
  const rewrites: Rewrite[] = [];
  // Includes possessives, e.g. "Urza's" → "~'s".
  const re = new RegExp(`(?<![\\p{L}\\p{M}\\p{N}_])${escapeRegExp(name)}(?![\\p{L}\\p{M}\\p{N}_])`, "gu");
  for (const match of text.matchAll(re)) {
    const [matched] = match;
    const start = match.index, end = start + matched.length;
    const before = text.slice(0, start);
    if (form !== "full" && form !== "article") {
      // A planeswalker type and a prefix of a literal name do not refer to this object.
      if (/\b(?:named|partner\s+with)\s+"?$/i.test(before)) continue;
      // A flavor/ability-word label before a dash is printed text, not a reference.
      const head = before.slice(before.lastIndexOf("\n") + 1);
      const rest = text.slice(end);
      const dash = rest.indexOf(" — "), lineEnd = rest.indexOf("\n");
      if (!head.includes("—") && dash >= 0 && (lineEnd < 0 || dash < lineEnd)) {
        const label = head + matched + rest.slice(0, dash);
        if (!/[.,:;{]/.test(label) && label.trim().split(/\s+/u).length <= 4) continue;
      }
      if (/\b(?:a|an|another|each|every|all|other)\s+$/i.test(before) ||
          /\btarget\s+$/i.test(before) && !/\bthat\s+target\s+$/i.test(before) ||
          /^\s+(?:planeswalkers?|spells?|cards?|creatures?|tokens?)\b/i.test(rest)) continue;
    }
    // "named Squadron Hawk" stays literal when the ability is copied (CR 201.5).
    if (/\bnamed\s+"$/i.test(before)) continue;
    if (/\bnamed\s+$/i.test(before)) {
      const tag: LiteralTag = { edit: "literal-name", slot: "named", via: "own-name" };
      rewrites.push({ start, end, replacement: protect(matched, tag), tag });
    } else {
      rewrites.push({ start, end, replacement: "~", tag: { edit: "self-name", form } });
    }
  }
  return rewrites;
}

/** Contextual legendary shortenings (CR 201.5c). First-word form also needs a printed trigger subject. */
export function shortNameForm(name: string, typeLine?: string): [string, SelfNameForm] | undefined {
  const comma = name.indexOf(",");
  if (comma > 0) return [name.slice(0, comma).trim(), "before-comma"];
  const prefix = name.split(" ")[0];
  const [types, printedSubtypes] = typeLine?.split(/—|\s-\s/u) ?? [];
  const subtypes = printedSubtypes?.trim().split(/\s+/u) ?? [];
  if (types && /\bPlaneswalker\b/.test(types)) {
    return name !== prefix && subtypes.includes(prefix) ? [prefix, "planeswalker-subtype"] : undefined;
  }
  if (!namesLegendaryCharacter(typeLine)) return undefined;
  const epithet = /^(.+?) (?:the|of) ./u.exec(name);
  if (epithet) return [epithet[1], "before-epithet"];
  if (name.includes(" ") && /^\p{Lu}/u.test(prefix) && !subtypes.includes(prefix)) return [prefix, "first-word"];
}

function pronounRewrites(text: string): Rewrite[] {
  const rewrites: Rewrite[] = [];
  let start = 0;
  for (const line of text.split("\n")) {
    for (const word of sourcePronounRewrites(line)) {
      rewrites.push({
        start: start + word.start, end: start + word.end, replacement: word.replacement,
        tag: { edit: "pronoun", role: word.role, as: word.as },
      });
    }
    start += line.length + 1;
  }
  return rewrites;
}

function restoreLiterals(text: string, marker: string, literals: ProtectedLiteral[], tracked?: TrackedText): string {
  for (const [index, literal] of literals.entries()) {
    const key = `${marker}${index}${marker}`;
    const start = text.indexOf(key);
    if (start < 0) continue;
    const json = JSON.stringify(literal.name);
    // Keep legacy String.replace expansion of $$, $&, $` and $' in catalog names.
    const restored = text.replace(key, json);
    if (tracked) {
      const end = start + key.length;
      const replacement = restored.slice(start, restored.length - (text.length - end));
      if (replacement !== json) tracked.invalidate();
      tracked.splice(start, end, replacement, literal.tag, literal.quoted ? "restore" : "quote");
    }
    text = restored;
  }
  return text;
}

function normalize(
  original: string, cardName?: string, catalog?: CardNameCatalog, typeLine?: string, tracked?: TrackedText,
): { lines: string[]; starts: number[] } {
  if (!original) return { lines: [], starts: [] };
  let text = canonicalizeNamePunctuation(original);
  if (tracked) {
    for (let i = 0; i < text.length; i++) {
      if (original[i] !== text[i]) tracked.splice(i, i + 1, text[i], { edit: "punct" });
    }
  }

  const literalMarker = distinctMarker(text, "@@CATALOG_LITERAL@@");
  const literals: ProtectedLiteral[] = [];
  if (catalog) {
    const spans = catalogLiteralSpans(text, catalog);
    // Legacy protection runs in descending order, even on overlapping matches.
    for (let i = spans.length - 1; i >= 0; i--) {
      const span = spans[i];
      const marker = `${literalMarker}${literals.push(span) - 1}${literalMarker}`;
      tracked?.splice(span.start, span.end, marker, span.tag, "protect");
      text = text.slice(0, span.start) + marker + text.slice(span.end);
    }
  }
  text = applyRewrites(text, reminderRewrites(text), tracked, "reminder");

  if (cardName) {
    const name = canonicalizeNamePunctuation(cardName);
    const marker = distinctMarker(text, "@@NAMED_CARD@@");
    const names: ProtectedLiteral[] = [];
    const protect = (value: string, tag: LiteralTag) => `${marker}${names.push({ name: value, tag, quoted: false }) - 1}${marker}`;
    text = applyRewrites(text, nameRewrites(text, name, "full", protect), tracked);
    if (name.startsWith("The ")) {
      text = applyRewrites(text, nameRewrites(text, `the ${name.slice(4)}`, "article", protect), tracked);
    }
    const short = shortNameForm(name, typeLine);
    if (short) {
      const [prefix, form] = short;
      if (form !== "first-word" || new RegExp(
        `\\b(?:When|Whenever) ${escapeRegExp(prefix)}(?= (?:enters|attacks|dies|leaves|blocks|deals|becomes|is|transforms|or|and)\\b)`, "u",
      ).test(text)) {
        text = applyRewrites(text, nameRewrites(text, prefix, form, protect), tracked);
      }
    }
    // Literal names stay opaque while pronouns are decided.
    if (namesLegendaryCharacter(typeLine)) text = applyRewrites(text, pronounRewrites(text), tracked);
    text = restoreLiterals(text, marker, names, tracked);
  }
  text = restoreLiterals(text, literalMarker, literals, tracked);

  const folds: Rewrite[] = [];
  for (const match of text.matchAll(/[^\S\n]+/g)) {
    if (match[0] !== " ") folds.push({ start: match.index, end: match.index + match[0].length, replacement: " ", tag: { edit: "gap" } });
  }
  text = applyRewrites(text, folds, tracked);
  const lines: string[] = [], starts: number[] = [];
  let offset = 0;
  for (const line of text.split("\n")) {
    const normalized = line.trim();
    if (normalized) {
      lines.push(normalized);
      if (tracked) starts.push(offset + line.length - line.trimStart().length);
    }
    offset += line.length + 1;
  }
  return { lines, starts };
}

export function normalizeOracleText(text: string, cardName?: string, catalog?: CardNameCatalog, typeLine?: string): string[] {
  return normalize(text, cardName, catalog, typeLine).lines;
}

export function normalizeOracleTextTracked(text: string, cardName?: string, catalog?: CardNameCatalog, typeLine?: string): TrackedNormalization {
  // These byte spellings collide with the source marker or are outside the R6 evidence contract.
  const reason = text.includes("~") ? "raw-tilde" : text.includes("\0") ? "raw-nul" : undefined;
  if (reason) return { status: "unmappable", lines: normalizeOracleText(text, cardName, catalog, typeLine), reason };
  const tracked = new TrackedText(text);
  const { lines, starts } = normalize(text, cardName, catalog, typeLine, tracked);
  return tracked.finish(lines, starts);
}
