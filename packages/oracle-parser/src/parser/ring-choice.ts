// CR 701.54a: "Each time the Ring tempts you, choose a creature you control.
// That creature becomes your Ring-bearer …". "If you chose a creature other
// than ~ as your Ring-bearer" asks about that choice in the temptation that
// triggered the ability, so it is parsed only as the intervening-if of a
// "Whenever the Ring tempts you" trigger (701.54d).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseFilter } from "./refs.ts";

export function parseRingBearerChoice(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    if (!c.words("you", "chose") || c.word("a", "an") === null) return null;
    const chosen = parseFilter(c);
    if (!chosen || !c.words("as", "your", "ring-bearer") || !c.isPunct(",")) return null;
    return { condition: "ring-bearer-choice", who: { player: "you" }, chosen };
  });
}
