// Match the complete resolution sequence so "it" keeps the same referent
// and hand placement is required in both outcomes. CR 608.2c–d, 701.20.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// https://magic.wizards.com/en/news/feature/march-of-the-machine-release-notes
import type { Effect } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseAmount } from "./refs.ts";

type TopMana = Extract<Effect, { effect: "look-top-may-reveal-for-mana" }>;

export function parseLibraryTopMana(c: Cursor): TopMana | null {
  return c.attempt(c => {
    if (!c.words("look", "at", "the", "top", "card", "of", "your", "library") || !c.punct(".") ||
        !c.words("you", "may", "reveal", "that", "card", "if", "it", "has")) return null;

    const threshold = parseAmount(c);
    if (threshold?.amount !== "fixed" || !Number.isSafeInteger(threshold.value) || threshold.value < 1 ||
        !c.words("or", "more", "colored", "mana", "symbols", "in", "its", "mana", "cost") || !c.punct(".") ||
        !c.words("if", "you", "do") || !c.punct(",") || c.word("add") === null) return null;

    const mana = parseAmount(c);
    if (mana?.amount !== "fixed" || !Number.isSafeInteger(mana.value) || mana.value < 1 ||
        !c.words("mana", "in", "any", "combination", "of", "its", "colors", "and", "put", "it", "into", "your", "hand") ||
        !c.punct(".") || !c.words("if", "you", "don't", "reveal", "it") || !c.punct(",") ||
        !c.words("put", "it", "into", "your", "hand") || (!c.done() && !c.punct(".")) || !c.done()) return null;

    return { effect: "look-top-may-reveal-for-mana", who: { player: "you" }, library: { zone: "library", owner: "your" },
      revealIf: { op: "ge", value: threshold }, mana, to: { zone: "hand", owner: "your" } };
  });
}
