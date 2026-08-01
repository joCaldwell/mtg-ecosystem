// Backtracking token cursor shared by all parse functions.
//
// Conventions:
//  - Parse functions take a Cursor and return a node or null. On null they
//    must leave the cursor where they found it — use attempt() for that.
//  - fail() records the farthest point reached and what was expected there,
//    so a failed line reports a useful diagnostic instead of "no viable
//    alternative".

import type { Token } from "../lexer.ts";

export class Cursor {
  pos = 0;
  readonly tokens: Token[];
  private farthestPos = 0;
  private farthestExpected: string[] = [];

  constructor(tokens: Token[]) {
    this.tokens = tokens;
  }

  done(): boolean {
    return this.pos >= this.tokens.length;
  }

  peek(offset = 0): Token | undefined {
    return this.tokens[this.pos + offset];
  }

  next(): Token | undefined {
    return this.tokens[this.pos++];
  }

  /** Run fn; if it returns null, rewind to where we started. */
  attempt<T>(fn: (c: this) => T | null): T | null {
    const start = this.pos;
    const result = fn(this);
    if (result === null) this.pos = start;
    return result;
  }

  /** Record a failure expectation at the current position; returns null. */
  fail(expected: string): null {
    if (this.pos > this.farthestPos) {
      this.farthestPos = this.pos;
      this.farthestExpected = [expected];
    } else if (this.pos === this.farthestPos && !this.farthestExpected.includes(expected)) {
      this.farthestExpected.push(expected);
    }
    return null;
  }

  /** Farthest position any parse attempt reached (for picking best diagnostic). */
  get farthest(): number {
    return this.farthestPos;
  }

  errorMessage(): string {
    const tok = this.tokens[this.farthestPos];
    const at = tok
      ? `"${tok.kind === "word" ? tok.raw : tok.kind === "number" ? tok.raw : tok.kind === "symbol" ? `{${tok.value}}` : tok.kind === "selfref" ? "~" : tok.value}"`
      : "end of line";
    const expected = this.farthestExpected.length
      ? this.farthestExpected.join(" | ")
      : "?";
    return `expected ${expected} at ${at} (token ${this.farthestPos})`;
  }

  // -- token-level matchers -------------------------------------------------

  /** Consume a word token whose value is one of `values`; returns it or null. */
  word(...values: string[]): string | null {
    const t = this.peek();
    if (t?.kind === "word" && !t.possessive && values.includes(t.value)) {
      this.pos++;
      return t.value;
    }
    return this.fail(values.length <= 3 ? values.map((v) => `'${v}'`).join("/") : `'${values[0]}'…`);
  }

  /** Consume a possessive word ("owner's" → "owner"). */
  possessive(...values: string[]): string | null {
    const t = this.peek();
    if (t?.kind === "word" && t.possessive && values.includes(t.value)) {
      this.pos++;
      return t.value;
    }
    return this.fail(values.map((v) => `'${v}'s'`).join("/"));
  }

  /** Consume an exact sequence of word values — all or nothing. */
  words(...seq: string[]): boolean {
    const start = this.pos;
    for (const w of seq) {
      if (this.word(w) === null) {
        this.pos = start;
        return false;
      }
    }
    return true;
  }

  /** Consume any word token (non-possessive). */
  anyWord(): { value: string; raw: string } | null {
    const t = this.peek();
    if (t?.kind === "word" && !t.possessive) {
      this.pos++;
      return { value: t.value, raw: t.raw };
    }
    return this.fail("word");
  }

  number(): number | null {
    const t = this.peek();
    if (t?.kind === "number") {
      this.pos++;
      return t.value;
    }
    return this.fail("number");
  }

  punct(value: string): boolean {
    const t = this.peek();
    if (t?.kind === "punct" && t.value === value) {
      this.pos++;
      return true;
    }
    this.fail(`'${value}'`);
    return false;
  }

  symbol(): string | null {
    const t = this.peek();
    if (t?.kind === "symbol") {
      this.pos++;
      return t.value;
    }
    return this.fail("{symbol}");
  }

  selfref(): boolean {
    const t = this.peek();
    if (t?.kind === "selfref" && !t.possessive) {
      this.pos++;
      return true;
    }
    this.fail("~");
    return false;
  }

  selfrefPossessive(): boolean {
    const t = this.peek();
    if (t?.kind === "selfref" && t.possessive) {
      this.pos++;
      return true;
    }
    this.fail("~'s");
    return false;
  }

  /** Peek: is the next token a word with one of these values? */
  isWord(...values: string[]): boolean {
    const t = this.peek();
    return t?.kind === "word" && !t.possessive && values.includes(t.value);
  }

  isPunct(value: string): boolean {
    const t = this.peek();
    return t?.kind === "punct" && t.value === value;
  }
}
