// Normalization: raw Scryfall oracle text → clean ability lines.
//
// Contract:
//  - Newlines are ability boundaries and are PRESERVED (the old pipeline
//    flattened them, forcing the grammar to guess where abilities split).
//  - Reminder text (parentheticals) is removed.
//  - Self-references (the card's own name, and its pre-comma short name)
//    become `~`. Matching is exact-case and word-bounded — a card named
//    "Fear" must not rewrite the word "fear" in other contexts.
//  - Typographic quotes/minus are canonicalized so the lexer stays simple.

import { canonicalizeNamePunctuation, type CardNameCatalog } from "./card-names.ts";
import { namesLegendaryCharacter, rewriteSourcePronouns } from "./source-pronouns.ts";

// CR 207.2a: parenthetical reminder text summarizes a rule rather than adding
// an ability. Remove a whole balanced parenthetical, including nested pairs,
// but never remove its line boundaries or join the words on either side.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function stripReminderText(text: string): string {
  let result = "";
  let copiedThrough = 0;
  for (let i = 0; i < text.length; i++) {
    if (text[i] !== "(") continue;
    let depth = 1;
    let end = i + 1;
    while (end < text.length && depth > 0) {
      if (text[end] === "(") depth++;
      else if (text[end] === ")") depth--;
      end++;
    }
    // Malformed parentheticals remain visible to the lexer, which rejects
    // them. Silently deleting an unterminated tail would create false success.
    if (depth !== 0) break;
    const newlines = text.slice(i, end).match(/\n/g)?.join("") ?? "";
    result += text.slice(copiedThrough, i) + (newlines || " ");
    copiedThrough = end;
    i = end - 1;
  }
  return result + text.slice(copiedThrough);
}

function canonicalizePunctuation(text: string): string {
  return canonicalizeNamePunctuation(text);
}

function escapeRegExp(s: string): string {
  return s.replace(/[\\^$*+?.()|[\]{}]/g, "\\$&");
}

function replaceName(text: string, name: string, protect: (name: string) => string, shortened = false): string {
  // Word-bounded, case-sensitive. Handles possessive: "Urza's" → "~'s".
  const re = new RegExp(`(?<![\\p{L}\\p{M}\\p{N}_])${escapeRegExp(name)}(?![\\p{L}\\p{M}\\p{N}_])`, "gu");
  return text.replace(re, (matched, offset: number) => {
    const before = text.slice(0, offset);
    if (shortened) {
      // A planeswalker type is not a reference to this particular object:
      // Gideon's emblem says "a Gideon planeswalker" (CR 201.5 / 205.3j).
      // A prefix of a literal name such as Nissa's Chosen is not that name.
      if (/\b(?:named|partner\s+with)\s+"?$/i.test(before)) return matched;
      if (/\b(?:a|an|another|each|every|all|other)\s+$/i.test(before) ||
          /\btarget\s+$/i.test(before) && !/\bthat\s+target\s+$/i.test(before) ||
          /^\s+(?:planeswalkers?|spells?|cards?|creatures?|tokens?)\b/i.test(text.slice(offset + matched.length))) return matched;
    }
    // "named Squadron Hawk" specifies a literal name, not a reference to
    // this object (CR 201.5). Copying the ability cannot change that name.
    if (/\bnamed\s+"$/i.test(before)) return matched;
    if (/\bnamed\s+$/i.test(before)) return protect(matched);
    return "~";
  });
}

export function normalizeOracleText(text: string, cardName?: string, catalog?: CardNameCatalog, typeLine?: string): string[] {
  if (!text) return [];

  let t = canonicalizePunctuation(text);
  let literalMarker = "@@CATALOG_LITERAL@@";
  while (t.includes(literalMarker)) literalMarker += "@";
  const literals: string[] = [];
  // Only explicit grammar positions establish literal names. In particular,
  // do not extend comma/and lists by name lookup: Sacrifice and X are card
  // names, but can instead begin the following instruction or amount.
  if (catalog) {
    const spans: { start: number; end: number; name: string }[] = [];
    for (const match of t.matchAll(/\b(named|partner\s+with|creates?)\s+/gi)) {
      const context = match[1].toLowerCase();
      if (context === "named" && /\bdifferently\s+$/i.test(t.slice(0, match.index))) continue;
      const start = match.index + match[0].length;
      const kinds = context.startsWith("partner") ? ["card" as const] : ["card" as const, "token" as const];
      const candidates = catalog.matches(t, start, kinds);
      let literal = candidates[0]?.end !== candidates[1]?.end ? candidates[0] : undefined;
      if (!literal && t[start] === '"') {
        let end = start + 1;
        while (end < t.length) {
          if (t[end] === "\\") { end += 2; continue; }
          if (t[end] === '"') break;
          end++;
        }
        if (end < t.length) {
          try {
            const value: unknown = JSON.parse(t.slice(start, end + 1));
            const known = typeof value === "string" ? catalog.exact(value, kinds) : undefined;
            if (known) literal = { ...known, end: end + 1 };
          } catch { /* Malformed literal remains visible to the lexer. */ }
        }
      }
      if (!literal || (context.startsWith("create") && !/^,\s+(?:a|an)\s+/.test(t.slice(literal.end)))) continue;
      spans.push({ start, end: literal.end, name: literal.name });
    }
    for (const span of spans.sort((a, b) => b.start - a.start)) {
      const marker = `${literalMarker}${literals.push(span.name) - 1}${literalMarker}`;
      t = t.slice(0, span.start) + marker + t.slice(span.end);
    }
  }
  t = stripReminderText(t);

  if (cardName) {
    const name = canonicalizePunctuation(cardName);
    let marker = "@@NAMED_CARD@@";
    while (t.includes(marker)) marker += "@";
    const names: string[] = [];
    const protect = (value: string) => `${marker}${names.push(value) - 1}${marker}`;
    t = replaceName(t, name, protect);
    const comma = name.indexOf(",");
    if (comma > 0) {
      // Legendary short name: "Halsin, Emerald Archdruid" → also replace "Halsin".
      t = replaceName(t, name.slice(0, comma).trim(), protect, true);
    } else if (typeLine && /\bPlaneswalker\b/.test(typeLine.split(/—|\s-\s/u, 1)[0])) {
      // Some planeswalker names lack a comma (Vraska the Unseen, Ajani
      // Vengeant). Source metadata establishes the named character: require
      // an exact initial name word matching a printed planeswalker subtype.
      // Never shorten an arbitrary creature/card name to its first word.
      // CR 201.5c: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
      const prefix = name.split(" ")[0];
      const subtypes = typeLine.split(/—|\s-\s/u)[1]?.trim().split(/\s+/u) ?? [];
      if (name !== prefix && subtypes.includes(prefix)) t = replaceName(t, prefix, protect, true);
    }
    // Literal names are still opaque markers here, so a pronoun-shaped word
    // inside one ("A Girl and Her Dogs") cannot be rewritten.
    if (namesLegendaryCharacter(typeLine)) t = t.split("\n").map(rewriteSourcePronouns).join("\n");
    names.forEach((value, index) => { t = t.replace(`${marker}${index}${marker}`, JSON.stringify(value)); });
  }

  literals.forEach((value, index) => { t = t.replace(`${literalMarker}${index}${literalMarker}`, JSON.stringify(value)); });

  return t
    .split("\n")
    .map((line) => line.replace(/\s+/g, " ").trim())
    .filter((line) => line.length > 0);
}
