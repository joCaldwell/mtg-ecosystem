// "Excess damage is dealt to that creature's controller instead." modifies
// the damage event of the instruction it follows (CR 120.4a), rather than
// being a later damage instruction or a standing replacement.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Sentence } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parsePlayerRef } from "./refs.ts";

/** Only a sentence that is one unconditional damage instruction to one
 * permanent has an unambiguous "excess damage". */
export function foldExcessDamageSentence(c: Cursor, previous: Sentence): boolean {
  return c.attempt(c => {
    if (!c.words("excess", "damage", "is", "dealt", "to")) return null;
    const to = parsePlayerRef(c);
    if (!to || c.word("instead") === null || (!c.punct(".") && !c.done())) return null;
    const only = previous.steps.length === 1 ? previous.steps[0] : undefined;
    if (previous.otherwise || previous.condition || only?.effect !== "damage" || only.excessTo
      || only.optional || only.forEach || only.to.to !== "object") return c.fail("one damage instruction to one permanent");
    const recipient = only.to.ref;
    const single = recipient.ref === "target" ? recipient.count.amount === "fixed" && recipient.count.value === 1
      : ["it", "self", "this", "enchanted", "equipped"].includes(recipient.ref) || (recipient.ref === "that" && !recipient.plural);
    if (!single) return c.fail("one damage instruction to one permanent");
    only.excessTo = to;
    return true;
  }) === true;
}
