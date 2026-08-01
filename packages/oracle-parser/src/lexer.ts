// Hand-written lexer for normalized oracle text (one line at a time).
//
// Guarantees, unlike the old ANTLR lexer:
//  - Unknown characters are a loud LexError, never silently dropped.
//  - Apostrophes are handled structurally: known contractions stay one token
//    ("can't"), possessives are flagged ("owner's" → word "owner" + possessive).
//  - Every mana/game symbol `{…}` is one token regardless of its contents
//    (hybrid, Phyrexian, snow, … all covered for free).

export type Punct = "." | "," | ":" | ";" | "—" | "•" | '"' | "+" | "-" | "/";

export type Token =
  | { kind: "word"; value: string; raw: string; possessive: boolean }
  | { kind: "number"; value: number; raw: string }
  | { kind: "symbol"; value: string } // "{2}" → "2", "{G/P}" → "G/P", "{T}" → "T"
  | { kind: "selfref"; possessive: boolean }
  | { kind: "punct"; value: Punct };

export class LexError extends Error {
  readonly position: number;

  constructor(message: string, position: number) {
    super(message);
    this.name = "LexError";
    this.position = position;
  }
}

const PUNCT_CHARS = new Set([".", ",", ":", ";", "—", "•", '"', "+", "-", "/"]);

/** Contractions kept whole as a single word token (value includes the apostrophe). */
const CONTRACTIONS = new Set([
  "can't", "don't", "doesn't", "isn't", "aren't", "won't", "wasn't",
  "weren't", "hasn't", "haven't", "hadn't", "didn't", "couldn't",
  "shouldn't", "wouldn't", "it's", "that's", "there's", "they're",
  "you're", "you've", "he's", "she's", "what's", "who's", "let's",
]);

function isLetter(c: string): boolean {
  return /[a-zA-Zàáâäèéêëìíîïòóôöùúûü]/.test(c);
}

export function lex(line: string): Token[] {
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
      let value = raw.toLowerCase();

      if (CONTRACTIONS.has(value)) {
        // keep whole
      } else if (value.endsWith("'s")) {
        value = value.slice(0, -2);
        raw = raw.slice(0, -2);
        possessive = true;
      } else if (i < line.length && line[i] === "'" && value.endsWith("s")) {
        // plural possessive: owners'
        i++;
        possessive = true;
      }
      tokens.push({ kind: "word", value, raw, possessive });
      continue;
    }

    if (PUNCT_CHARS.has(c)) {
      tokens.push({ kind: "punct", value: c as Punct });
      i++;
      continue;
    }

    throw new LexError(`unexpected character ${JSON.stringify(c)}`, i);
  }

  return tokens;
}
