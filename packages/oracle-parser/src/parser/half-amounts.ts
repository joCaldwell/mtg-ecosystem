// "Half" amounts. CR 107.1a: a spell or ability that could generate a
// fractional number tells you whether to round up or down, so every `half`
// amount carries its stated direction. The direction usually follows the
// quantity directly ("half X, rounded down"), but is often printed after the
// whole instruction ("... draws half X cards, rounded down"). The latter is
// parsed as an unresolved node that the enclosing step must resolve with the
// trailing ", rounded up|down"; it never survives into a finished AST.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

type Half = Extract<Amount, { amount: "half" }>;

const unresolved = new WeakSet<object>();
let enabled = 0;

/** Enable deferred rounding while one effect step is being parsed. */
export function withDeferredRounding<T>(body: () => T): T {
  enabled++;
  try { return body(); } finally { enabled--; }
}

/** An amount whose rounding direction is stated after the instruction. Only
 * available while a step is being parsed, so that the step can resolve it. */
export function deferredHalf(of: Amount): Amount | null {
  if (enabled === 0) return null;
  const node: Half = { amount: "half", of, round: "up" };
  unresolved.add(node);
  return node;
}

function collect(value: unknown, found: Set<Half>, seen: Set<object>): void {
  if (typeof value !== "object" || value === null || seen.has(value)) return;
  seen.add(value);
  if (unresolved.has(value)) found.add(value as Half);
  for (const child of Object.values(value)) collect(child, found, seen);
}

/** After a step: if it contains an unresolved half, the trailing ", rounded
 * up|down" gives its direction. Exactly one half can be resolved this way; a
 * step with several would need to say which rounding belongs to which. */
export function resolveDeferredRounding(c: Cursor, result: unknown): boolean {
  const found = new Set<Half>();
  collect(result, found, new Set());
  if (found.size === 0) return true;
  if (found.size > 1) return false;
  const round = c.attempt((c): "up" | "down" | null => {
    if (!c.punct(",") || !c.words("rounded")) return null;
    return c.word("up", "down") as "up" | "down" | null;
  });
  if (!round) return false;
  const [node] = found;
  node.round = round;
  unresolved.delete(node);
  return true;
}

/** Resolves an unresolved half inside `amount` with a ", rounded up|down"
 * that directly follows the noun it quantifies ("half X damage, rounded
 * down, to …"). True when a rounding was consumed here; an amount without a
 * pending half, or without the trailing words, is left for the step. */
export function resolveRoundingHere(c: Cursor, amount: Amount): boolean {
  const found = new Set<Half>();
  collect(amount, found, new Set());
  if (found.size !== 1) return false;
  const start = c.pos;
  if (!resolveDeferredRounding(c, amount)) { c.pos = start; return false; }
  return c.pos !== start;
}

function possessive(c: Cursor): PlayerRef | null {
  if (c.word("your") !== null) return { player: "you" };
  if (c.word("their") !== null) return { player: "they" };
  return null;
}

/** "half your life, rounded up", "half their life, rounded down": half of the
 * named player's life total (CR 119.1), rounding stated directly. */
export function parseHalfOwnLife(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word("half") === null) return null;
    const of = possessive(c);
    if (!of || c.word("life") === null) return null;
    c.punct(",");
    if (!c.words("rounded")) return null;
    const round = c.word("up", "down") as "up" | "down" | null;
    return round && { amount: "half", of: { amount: "life-total", of }, round };
  });
}

/** "half their library, rounded down": half the number of cards in that
 * library, rounding stated directly. */
export function parseHalfOwnLibrary(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word("half") === null) return null;
    const owner = c.word("your") !== null ? "your" : c.word("their") !== null ? "their" : null;
    if (!owner || c.word("library") === null) return null;
    c.punct(",");
    if (!c.words("rounded")) return null;
    const round = c.word("up", "down") as "up" | "down" | null;
    return round && {
      amount: "half",
      of: { amount: "count", of: { cls: "card", zone: { zone: "library", owner } } },
      round,
    };
  });
}
