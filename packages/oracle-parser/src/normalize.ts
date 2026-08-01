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

const REMINDER = /\s?\([^()]*\)/g;

function escapeRegExp(s: string): string {
  return s.replace(/[-\/\\^$*+?.()|[\]{}]/g, "\\$&");
}

function replaceName(text: string, name: string): string {
  // Word-bounded, case-sensitive. Handles possessive: "Urza's" → "~'s".
  const re = new RegExp(`(?<![A-Za-z])${escapeRegExp(name)}(?![A-Za-z])`, "g");
  return text.replace(re, "~");
}

export function normalizeOracleText(text: string, cardName?: string): string[] {
  if (!text) return [];

  let t = text
    .replace(/’/g, "'")
    .replace(/[“”]/g, '"')
    .replace(/−/g, "-") // real minus sign (loyalty costs) → hyphen
    .replace(/–/g, "—"); // en dash (rare typo) → em dash

  t = t.replace(REMINDER, "");

  if (cardName) {
    t = replaceName(t, cardName);
    const comma = cardName.indexOf(",");
    if (comma > 0) {
      // Legendary short name: "Halsin, Emerald Archdruid" → also replace "Halsin".
      t = replaceName(t, cardName.slice(0, comma).trim());
    }
  }

  return t
    .split("\n")
    .map((line) => line.replace(/\s+/g, " ").trim())
    .filter((line) => line.length > 0);
}
