// Backtracking token cursor shared by all parse functions.
//
// Conventions:
//  - Parse functions take a Cursor and return a node or null. On null they
//    must leave the cursor where they found it — use attempt() for that.
//  - fail() records the farthest point reached and what was expected there,
//    so a failed line reports a useful diagnostic instead of "no viable
//    alternative".

import type { Token } from "../lexer.ts";
import { tokenCapture, type PrintedCapture } from "../printed/capture.ts";
import type { ImplicitReason } from "../printed/schema.ts";

export class Cursor {
  private position = 0;
  readonly printed?: PrintedCapture;
  private readonly writes: { position: number; start: number; end: number }[] = [];

  get pos(): number { return this.position; }
  set pos(position: number) {
    if (this.printed && position < this.position) {
      while (this.writes.length && this.writes.at(-1)!.position > position) {
        const write = this.writes.pop()!;
        this.printed.rewind(write.start, write.end);
      }
    }
    this.position = position;
  }
  /** Set only while parsing the instructions of a pregame opening-hand action
   * (CR 103.6b), where "the first upkeep" names a turn of the game that has not
   * begun; elsewhere that wording stays unsupported. */
  pregame = false;
  readonly tokens: Token[];
  private farthestPos = 0;
  private farthestExpected: string[] = [];

  constructor(tokens: Token[]) {
    this.tokens = tokens;
    this.printed = tokenCapture(tokens);
  }

  done(): boolean {
    return this.pos >= this.tokens.length;
  }

  peek(offset = 0): Token | undefined {
    return this.tokens[this.pos + offset];
  }

  next(): Token | undefined {
    if (this.done()) return undefined;
    return this.tokens[this.pos++];
  }

  /** Run fn; if it returns null, rewind to where we started. */
  attempt<T>(fn: (c: this) => T | null, origin: "record" | "preserve" = "record"): T | null {
    const start = this.pos;
    const checkpoint = this.printed?.checkpoint();
    const previousWrites = this.writes.length;
    const result = fn(this);
    if (result === null) {
      this.pos = start;
      if (checkpoint !== undefined) {
        this.printed!.rewind(checkpoint);
        // The failed production may have returned null without advancing.
        while (this.writes.length > previousWrites) this.writes.pop();
      }
    } else if (checkpoint !== undefined) {
      if (origin === "record" && result && typeof result === "object" && !Array.isArray(result))
        this.printed!.mark(result, this.tokens.slice(start, this.pos));
      this.noteWrite(checkpoint);
    }
    return result;
  }

  mark<const T extends object>(node: T, start: number, end = this.pos, mode?: "complete-production"): T {
    if (this.printed) {
      const checkpoint = this.printed.checkpoint();
      this.printed.mark(node, this.tokens.slice(start, end), mode);
      this.noteWrite(checkpoint);
    }
    return node;
  }

  markTokens<const T extends object>(node: T, tokens: readonly Token[]): T {
    if (this.printed) {
      const checkpoint = this.printed.checkpoint();
      this.printed.mark(node, tokens);
      this.noteWrite(checkpoint);
    }
    return node;
  }

  fold(node: object, start: number, end = this.pos): void {
    if (this.printed) this.foldTokens(node, this.tokens.slice(start, end));
  }

  foldTokens(node: object, tokens: readonly Token[]): void {
    if (this.printed) {
      const checkpoint = this.printed.checkpoint();
      this.printed.fold(node, tokens);
      this.noteWrite(checkpoint);
    }
  }

  foldOrigin(node: object, clause: object): void {
    if (this.printed) {
      const checkpoint = this.printed.checkpoint();
      this.printed.foldOrigin(node, clause);
      this.noteWrite(checkpoint);
    }
  }

  hull<const T extends object>(node: T, start = this.pos, end = this.pos): T {
    if (this.printed) {
      const checkpoint = this.printed.checkpoint();
      this.printed.hull(node, this.tokens.slice(start, end));
      this.noteWrite(checkpoint);
    }
    return node;
  }

  imperativeController(): { player: "you" } {
    return this.implicit({ player: "you" }, "imperative-controller");
  }

  alias<T extends object>(node: T): T {
    if (!this.printed) return node;
    const checkpoint = this.printed.checkpoint();
    const alias = this.printed.alias(node);
    this.noteWrite(checkpoint);
    return alias;
  }

  implicit<const T extends object>(node: T, reason: ImplicitReason): T {
    if (this.printed) {
      const checkpoint = this.printed.checkpoint();
      this.printed.implicit(node, reason);
      this.noteWrite(checkpoint);
    }
    return node;
  }

  private noteWrite(start: number): void {
    const end = this.printed!.checkpoint();
    if (end > start) this.writes.push({ position: this.pos, start, end });
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

  errorMessage(offset = 0): string {
    const tok = this.tokens[this.farthestPos];
    const at = tok
      ? `"${tok.kind === "word" ? tok.raw : tok.kind === "number" ? tok.raw : tok.kind === "symbol" ? `{${tok.value}}` : tok.kind === "selfref" ? "~" : tok.value}"`
      : "end of line";
    const expected = this.farthestExpected.length
      ? this.farthestExpected.slice(0, 8).join(" | ") +
        (this.farthestExpected.length > 8 ? ` | … (${this.farthestExpected.length - 8} more alternatives)` : "")
      : "?";
    return `expected ${expected} at ${at} (token ${offset + this.farthestPos})`;
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
