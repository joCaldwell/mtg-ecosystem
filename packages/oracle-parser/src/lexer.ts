// Hand-written lexer for normalized oracle text (one line at a time).
//
// Guarantees, unlike the old ANTLR lexer:
//  - Unknown characters are a loud LexError, never silently dropped.
//  - Apostrophes are handled structurally: known contractions stay one token
//    ("can't"), possessives are flagged ("owner's" → word "owner" + possessive).
//  - Every mana/game symbol `{…}` is one token regardless of its contents
//    (hybrid, Phyrexian, snow, … all covered for free).

import type { CardNameCatalog } from "./card-names.ts";

export type Punct = "." | "," | ":" | ";" | "—" | "•" | '"' | "'" | "+" | "-" | "/" | "!" | "|" | "[" | "]";

/** Location within one normalized source line, retained when parsers slice or
 * clone tokens. The index counts tokens, including punctuation and symbols. */
export interface TokenProvenance { sourceId: string; index: number }

export type Token = { provenance?: TokenProvenance } & (
  /** possessiveSuffix is the printed ending of a possessive ("'s" or the bare
   * apostrophe of "owners'"), kept so labels can be reproduced exactly. */
  | { kind: "word"; value: string; raw: string; possessive: boolean; possessiveSuffix?: "'s" | "'" }
  | { kind: "number"; value: number; raw: string }
  | { kind: "symbol"; value: string } // "{2}" → "2", "{G/P}" → "G/P", "{T}" → "T"
  | { kind: "selfref"; possessive: boolean }
  | { kind: "card-name"; value: string }
  | { kind: "punct"; value: Punct });

export class LexError extends Error {
  readonly position: number;

  constructor(message: string, position: number) {
    super(message);
    this.name = "LexError";
    this.position = position;
  }
}

const PUNCT_CHARS = new Set([".", ",", ":", ";", "—", "•", '"', "+", "-", "/", "!", "|", "[", "]"]);

/** Contractions kept whole as a single word token (value includes the apostrophe). */
const CONTRACTIONS = new Set([
  "can't", "don't", "doesn't", "isn't", "aren't", "won't", "wasn't",
  "weren't", "hasn't", "haven't", "hadn't", "didn't", "couldn't",
  "shouldn't", "wouldn't", "it's", "that's", "there's", "they're",
  "you're", "you've", "he's", "she's", "what's", "who's", "let's",
]);

function isLetter(c: string): boolean {
  return /\p{L}/u.test(c);
}

export function lex(line: string, sourceId?: string, catalog?: CardNameCatalog): Token[] {
  const tokens: Token[] = [];
  let i = 0;

  while (i < line.length) {
    const c = line[i];

    if (c === " " || c === "\t") {
      i++;
      continue;
    }

    if (c === "{") {
      const close = line.indexOf("}", i);
      if (close === -1) throw new LexError(`unclosed symbol brace`, i);
      tokens.push({ kind: "symbol", value: line.slice(i + 1, close) });
      i = close + 1;
      continue;
    }

    if (c === "~") {
      i++;
      let possessive = false;
      if (line.startsWith("'s", i)) {
        possessive = true;
        i += 2;
      }
      tokens.push({ kind: "selfref", possessive });
      continue;
    }

    // Normalization quotes protected literal names in explicit name slots. This is
    // lexical data, never an opaque fallback for an instruction or ability.
    const previous = tokens[tokens.length - 1];
    const beforePrevious = tokens[tokens.length - 2];
    const partnerName = previous?.kind === "word" && previous.value === "with" && !previous.possessive &&
      beforePrevious?.kind === "word" && beforePrevious.value === "partner" && !beforePrevious.possessive;
    const nameContext = previous?.kind === "word" && !previous.possessive && ["named", "create", "creates"].includes(previous.value) || partnerName;
    if (c === '"' && nameContext) {
      let end = i + 1;
      while (end < line.length) {
        if (line[end] === "\\") { end += 2; continue; }
        if (line[end] === '"') break;
        end++;
      }
      if (end === line.length) throw new LexError("unclosed literal card name", i);
      let value: unknown;
      try { value = JSON.parse(line.slice(i, end + 1)); } catch { throw new LexError("invalid literal card name", i); }
      if (typeof value !== "string" || !value.trim()) throw new LexError("empty literal card name", i);
      if (catalog && !catalog.exact(value, partnerName ? ["card"] : ["card", "token"])) throw new LexError("literal name is absent from the source catalog", i);
      tokens.push({ kind: "card-name", value });
      i = end + 1;
      continue;
    }

    if (/[0-9]/.test(c)) {
      let j = i;
      while (j < line.length && /[0-9]/.test(line[j])) j++;
      const raw = line.slice(i, j);
      tokens.push({ kind: "number", value: parseInt(raw, 10), raw });
      i = j;
      continue;
    }

    if (isLetter(c)) {
      // Read a word: letters, plus internal hyphens/apostrophes followed by a letter.
      let j = i;
      while (j < line.length) {
        const d = line[j];
        if (isLetter(d)) {
          j++;
        } else if ((d === "'" || d === "-") && j + 1 < line.length && isLetter(line[j + 1])) {
          j++;
        } else {
          break;
        }
      }
      let raw = line.slice(i, j);
      i = j;
      let possessive = false;
      let possessiveSuffix: "'s" | "'" | undefined;
      let value = raw.toLowerCase();

      if (CONTRACTIONS.has(value)) {
        // keep whole
      } else if (value.endsWith("'s")) {
        value = value.slice(0, -2);
        raw = raw.slice(0, -2);
        possessive = true;
        possessiveSuffix = "'s";
      } else if (i < line.length && line[i] === "'" && value.endsWith("s")) {
        // plural possessive: owners'
        i++;
        possessive = true;
        possessiveSuffix = "'";
      }
      tokens.push({ kind: "word", value, raw, possessive, ...(possessiveSuffix ? { possessiveSuffix } : {}) });
      continue;
    }

    if (PUNCT_CHARS.has(c)) {
      tokens.push({ kind: "punct", value: c as Punct });
      i++;
      continue;
    }

    // CR 702.186: the infinity symbol opens an Infinity permanent's "∞ —"
    // ability label; it is a word of that label, not a number.
    if (c === "∞" && tokens.length === 0) {
      tokens.push({ kind: "word", value: "∞", raw: "∞", possessive: false });
      i++;
      continue;
    }

    // An ability quoted inside a quoted ability uses single quotation marks
    // ("Mountains you control have '{T}: ...'"). Apostrophes inside words and
    // plural possessives were consumed with their word above, so a free mark
    // is a quotation mark, and only within an open double-quoted ability.
    if (c === "'" && tokens.filter(token => token.kind === "punct" && token.value === '"').length % 2 === 1) {
      tokens.push({ kind: "punct", value: "'" });
      i++;
      continue;
    }

    throw new LexError(`unexpected character ${JSON.stringify(c)}`, i);
  }

  settleQuotedPunctuation(tokens);
  return sourceId === undefined ? tokens : tokens.map((token, index) => ({
    ...token, provenance: { sourceId, index },
  }));
}

/** Oracle typesets a comma or final period inside a closing quotation mark
 * although it punctuates the enclosing sentence ('has "…," and is a Rogue';
 * 'with "…." Then …'). Give the mark to the enclosing sentence: no token is
 * dropped, and a quoted ability never needed its own trailing punctuation.
 * A period stays inside when the enclosing sentence visibly continues (in
 * lowercase, or with its own punctuation), and before "Then", which the
 * effect grammar already reads as a continuation of the quoted instruction.
 * Literal names are card-name tokens, so ability quotation marks alternate. */
function settleQuotedPunctuation(tokens: Token[]): void {
  let open = false;
  for (let index = 0; index < tokens.length; index++) {
    const token = tokens[index];
    if (token.kind !== "punct" || token.value !== '"') continue;
    const previous = tokens[index - 1];
    const next = tokens[index + 1];
    const continues = next?.kind === "word" ? next.raw[0] === next.raw[0].toLowerCase() || next.value === "then"
      : next?.kind === "punct" && next.value !== '"';
    if (open && previous?.kind === "punct" && (previous.value === "," || (previous.value === "." && !continues))) {
      tokens[index - 1] = token;
      tokens[index] = previous;
    }
    // The same mark typeset inside a nested closing quotation: ...target.'"
    const inner = tokens[index - 2];
    if (open && previous?.kind === "punct" && previous.value === "'" && inner?.kind === "punct" &&
        (inner.value === "," || (inner.value === "." && !continues))) {
      tokens[index - 2] = previous;
      tokens[index - 1] = token;
      tokens[index] = inner;
    }
    open = !open;
  }
}
