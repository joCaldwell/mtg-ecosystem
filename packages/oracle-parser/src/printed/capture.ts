import type { Token } from "../lexer.ts";
import type { ImplicitReason } from "./schema.ts";

export interface Occurrence { line: number; index: number }

export type Origin =
  | { basis: "recorded"; tokens: Occurrence[]; folds?: Occurrence[] }
  | { basis: "hull"; tokens: Occurrence[] }
  | { basis: "alias"; target: object; fallback?: ImplicitReason }
  | { basis: "implicit"; reason: ImplicitReason };

// Own keys and primitives survive the parser's deep-copy finalization.
// Each object-valued slot has its own evidence and mutation check. An
// array's primitive elements ("types") are own values; its objects are slots.
const childSlot = Symbol("AST child");
type OwnFields = Map<string, unknown>;

class PrimitiveArray {
  readonly key: string;
  constructor(key: string) { this.key = key; }
}

function ownValue(value: unknown): unknown {
  if (Array.isArray(value)) return new PrimitiveArray(JSON.stringify(value.flatMap(item =>
    item !== null && typeof item === "object" ? [] : [[typeof item, item]])));
  return value !== null && typeof value === "object" ? childSlot : value;
}

function sameOwn(a: unknown, b: unknown): boolean {
  return a instanceof PrimitiveArray ? b instanceof PrimitiveArray && a.key === b.key : Object.is(a, b);
}

function ownFields(node: object): OwnFields {
  return new Map(Object.entries(node).map(([key, value]) =>
    [key, ownValue(value)]));
}

interface Entry {
  node: object;
  origin: Origin;
  identity: object;
  fields?: OwnFields;
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

  identity(node: object): object | undefined {
    return this.entries.get(node)?.identity;
  }

  /** The recorded occurrences proving a node, through an alias to its canonical entry. */
  proof(node: object): Occurrence[] | undefined {
    let origin = this.origin(node);
    if (origin?.basis === "alias") origin = this.origin(origin.target);
    return origin?.basis === "recorded" ? origin.tokens : undefined;
  }

  changedSinceMark(node: object): boolean {
    const fields = this.entries.get(node)?.fields;
    if (!fields) return false;
    const current = Object.entries(node);
    return current.length !== fields.size || current.some(([key, value]) =>
      !fields.has(key) || !sameOwn(fields.get(key), ownValue(value)));
  }

  mark(node: object, tokens: readonly Token[], mode?: "complete-production"): void {
    // An imperative verb, connector, or inherited actor is no proof of a
    // printed controller pronoun, even when a wrapper returns a PlayerRef.
    if ("player" in node && node.player === "you" && !tokens.some(token =>
      token.kind === "word" && /^you(?:r(?:s|self)?|'(?:re|ve|ll|d))?$/.test(token.value))) return;
    const occurrences = tokens.map(token => this.occurrence(token));
    if (occurrences.length && occurrences.every(occurrence => occurrence !== undefined)) {
      this.write(node, { basis: "recorded", tokens: occurrences }, mode === "complete-production");
    }
  }

  fold(node: object, tokens: readonly Token[]): void {
    const occurrences = tokens.map(token => this.occurrence(token));
    if (occurrences.length && occurrences.every(item => item !== undefined)) this.extend(node, occurrences);
  }

  foldOrigin(node: object, clause: object): void {
    const origin = this.origin(clause);
    if (origin?.basis === "recorded") this.extend(node, [...origin.tokens, ...(origin.folds ?? [])]);
  }

  private extend(node: object, clause: Occurrence[]): void {
    const origin = this.origin(node);
    if (origin?.basis !== "recorded" && origin?.basis !== "hull") return;
    const earlier = [...origin.tokens, ...(origin.basis === "recorded" ? origin.folds ?? [] : [])];
    const extra = clause.filter(({ line, index }) => !earlier.some(token => token.line === line && token.index === index));
    if (origin.basis === "hull") this.write(node, { basis: "hull", tokens: [...earlier, ...extra] }, true);
    else this.write(node, { ...origin, ...(extra.length ? { folds: [...(origin.folds ?? []), ...extra] } : {}) }, true);
  }

  hull(node: object, tokens: readonly Token[] = []): void {
    const occurrences = tokens.map(token => this.occurrence(token));
    if (!occurrences.every(item => item !== undefined)) return;
    const origin = this.origin(node);
    const prior = origin?.basis === "recorded" ? [...origin.tokens, ...(origin.folds ?? [])]
      : origin?.basis === "hull" ? origin.tokens : [];
    if (prior.length || occurrences.length) this.write(node, { basis: "hull", tokens: [...prior, ...occurrences] }, true);
  }

  /** Reuse one lexical occurrence in a second AST slot. The canonical
   * occurrence keeps its marks, and only the alias copy inherits them. */
  alias<T extends object>(node: T, fallback?: ImplicitReason): T {
    const target = this.canonical(node);
    if (!target) return node;
    const alias = structuredClone(node);
    this.write(alias, { basis: "alias", target, ...(fallback ? { fallback } : {}) });
    return alias;
  }

  /** Re-origin a node as an alias in place. Its identity is kept, so aliases
   * already made of it resolve through it to the canonical occurrence. */
  realias(node: object, of: object): void {
    const target = this.canonical(of);
    if (target && this.entries.has(node)) this.write(node, { basis: "alias", target }, true);
  }

  private canonical(node: object): object | undefined {
    const entry = this.entries.get(node);
    return entry && (entry.origin.basis === "alias" ? entry.origin.target : entry.identity);
  }

  implicit(node: object, reason: ImplicitReason): void {
    this.write(node, { basis: "implicit", reason });
  }

  carry(from: object, to: object): void {
    const entry = this.entries.get(from);
    if (entry) this.write(to, entry.origin, false, entry.fields, entry.identity);
  }

  private write(node: object, origin: Origin, replace = false,
    fields?: OwnFields, identity?: object): void {
    // The first committed production wins. Wrappers returning the same
    // object do not expand its lexical proof.
    const previous = this.entries.get(node);
    if (previous && !replace) return;
    const entry: Entry = { node, origin, identity: identity ?? previous?.identity ?? node,
      fields: origin.basis === "recorded" ? fields ?? ownFields(node) : undefined, active: true, previous };
    this.entries.set(node, entry);
    owners.set(node, this);
    this.journal.push(entry);
  }
}
