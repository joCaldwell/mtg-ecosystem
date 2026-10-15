// After "pay": an object's mana cost as the amount paid ("sacrifice this
// creature unless you pay its mana cost"). The mana cost is the one printed
// on that object (CR 202.1); paying it is paying that mana (CR 118.1).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Cost } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parsePossessiveObject } from "./refs.ts";

export function parseManaCostPayment(c: Cursor): Cost | null {
  return c.attempt((c): Cost | null => {
    const of = parsePossessiveObject(c);
    // "reduced by", "plus" and similar adjustments are not represented here.
    return of && c.words("mana", "cost") && !c.isWord("reduced", "plus", "minus", "rather") ? { cost: "mana-cost-of", of } : null;
  });
}
