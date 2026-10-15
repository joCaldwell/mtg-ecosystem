// Several target players declared by one target word: "any number of target
// players each discard a card", "up to two target players each draw a card".
// CR 115.3, 601.2c: the controller announces how many and which different
// players; the floating "each" makes every one of them perform the action.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, PlayerRef } from "../ast.ts";
import { wordNumber } from "../vocab.ts";
import type { Cursor } from "./cursor.ts";
import { parsePlayerQualifiers, targetSelectionProvenance } from "./refs.ts";

function parsePluralCount(c: Cursor): { count: Amount; upTo?: true } | null {
  return c.attempt(c => {
    if (c.words("any", "number", "of")) return { count: { amount: "chosen", minimum: 0 } as Amount };
    const upTo = c.words("up", "to");
    const token = c.peek();
    const value = token?.kind === "number" ? token.value
      : token?.kind === "word" && !token.possessive ? wordNumber(token.value) : undefined;
    if (value === undefined || value < (upTo ? 1 : 2)) return null;
    c.pos++;
    return { count: { amount: "fixed", value } as Amount, ...(upTo ? { upTo: true as const } : {}) };
  });
}

/** The subject of an instruction, followed by its distributive "each". A
 * plural target phrase without "each" is not read here. */
export function parsePluralTargetPlayers(c: Cursor): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    const quantity = parsePluralCount(c);
    if (!quantity) return null;
    const token = c.peek();
    if (c.word("target") === null) return null;
    const noun = c.word("players", "opponents");
    if (!noun) return null;
    const base: PlayerRef = { player: noun === "players" ? "target-player" : "target-opponent", ...quantity, ...targetSelectionProvenance(token) };
    const filter = parsePlayerQualifiers(c);
    if (c.word("each") === null) return null;
    return filter ? { player: "filtered", base, filter } : base;
  });
}

/** Several target players: a following plural verb ("each lose 2 life and
 * sacrifice a creature", "then draw seven cards") keeps them as its actor. */
export function isPluralTargetPlayers(ref: PlayerRef): boolean {
  if (ref.player === "filtered") return isPluralTargetPlayers(ref.base);
  return (ref.player === "target-player" || ref.player === "target-opponent") && !!ref.count;
}
