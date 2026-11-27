import type { Token } from "../lexer.ts";
import type { ImplicitReason } from "./schema.ts";

export interface Occurrence { line: number; index: number }

export type Origin =
  | { basis: "recorded"; tokens: Occurrence[]; folds?: Occurrence[] }
  | { basis: "implicit"; reason: ImplicitReason };

interface Entry {
  node: object;
  origin: Origin;
  active: boolean;
  previous?: Entry;
}

const sources = new WeakMap<Token, { capture: PrintedCapture; occurrence: Occurrence }>();
const owners = new WeakMap<object, PrintedCapture>();
let capturing = false;

export function tokenCapture(tokens: readonly Token[]): PrintedCapture | undefined {
  if (!capturing) return undefined;
  for (const token of tokens) {
    const source = sources.get(token);
    if (source) return source.capture;
  }
  return undefined;
}

export function carryPrintedToken(from: Token, to: Token): Token {
  if (capturing) {
    const source = sources.get(from);
    if (source) sources.set(to, source);
  }
  return to;
}

export function carryPrintedOrigin<T>(from: object, to: T): T {
  if (capturing && to && typeof to === "object") owners.get(from)?.carry(from, to);
  return to;
}

/** One journal per face. Sliced cursors share it through original token identity. */
export class PrintedCapture {
  private readonly journal: Entry[] = [];
  private readonly entries = new WeakMap<object, Entry>();

  /** The parser runs synchronously. The guard keeps ordinary cursors off the
   * token table, while token identity still selects the journal for slices. */
  run<T>(parse: () => T): T {
    const previous = capturing;
    capturing = true;
    try { return parse(); }
    finally { capturing = previous; }
  }

  register(token: Token, occurrence: Occurrence): void {
    sources.set(token, { capture: this, occurrence });
  }

  occurrence(token: Token): Occurrence | undefined {
    const source = sources.get(token);
    return source?.capture === this ? source.occurrence : undefined;
  }

  checkpoint(): number { return this.journal.length; }

  rewind(start: number, end = this.checkpoint()): void {
    for (let i = end - 1; i >= start; i--) {
      const entry = this.journal[i];
      if (!entry.active) continue;
      entry.active = false;
      if (this.entries.get(entry.node) !== entry) continue;
      let previous = entry.previous;
      while (previous && !previous.active) previous = previous.previous;
      if (previous) this.entries.set(entry.node, previous);
      else this.entries.delete(entry.node);
    }
  }

  origin(node: object): Origin | undefined {
    return this.entries.get(node)?.origin;
  }

  mark(node: object, tokens: readonly Token[], mode?: "complete-production"): void {
    const occurrences = tokens.map(token => this.occurrence(token));
    if (occurrences.length && occurrences.every(occurrence => occurrence !== undefined)) {
      this.write(node, { basis: "recorded", tokens: occurrences }, mode === "complete-production");
    }
  }

  fold(node: object, tokens: readonly Token[]): void {
    const origin = this.origin(node);
    const occurrences = tokens.map(token => this.occurrence(token));
    if (origin?.basis === "recorded" && occurrences.length && occurrences.every(item => item !== undefined))
      this.write(node, { ...origin, folds: [...(origin.folds ?? []), ...occurrences] }, true);
  }

  implicit(node: object, reason: ImplicitReason): void {
    this.write(node, { basis: "implicit", reason });
  }

  carry(from: object, to: object): void {
    const origin = this.origin(from);
    if (origin) this.write(to, origin);
  }

  private write(node: object, origin: Origin, replace = false): void {
    // The first committed production wins. Wrappers returning the same
    // object do not expand its lexical proof.
    const previous = this.entries.get(node);
    if (previous && !replace) return;
    const entry: Entry = { node, origin, active: true, previous };
    this.entries.set(node, entry);
    owners.set(node, this);
    this.journal.push(entry);
  }
}
