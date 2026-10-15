// "The damage can't be prevented" as a rider on the damage instruction it
// follows (CR 608.2c, 615.12), never a turn-long rule about other damage.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, Sentence } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

type Damage = Extract<Effect, { effect: "damage" }>;

/** The damage instructions "the damage" can mean: every top-level step of
 * the instruction must be a damage deal (several recipients of one deal are
 * simultaneous). Anything else leaves the antecedent unproven. */
function damageOf(steps: Effect[]): Damage[] | null {
  const found: Damage[] = [];
  for (const step of steps) {
    const parts = step.effect === "simultaneous" ? step.steps : [step];
    for (const part of parts) {
      if (part.effect !== "damage" || part.unpreventable) return null;
      found.push(part);
    }
  }
  return found.length ? found : null;
}

function mark(steps: Effect[]): boolean {
  const damage = damageOf(steps);
  if (!damage) return false;
  for (const effect of damage) effect.unpreventable = true;
  return true;
}

/** "... deals 10 damage to any target and the damage can't be prevented":
 * call with the instruction just parsed, before it joins its sentence. */
export function foldUnpreventableClause(c: Cursor, parsed: Effect[]): boolean {
  return c.attempt(c => {
    if (c.word("and") === null || !c.words("the", "damage", "can't", "be", "prevented")) return null;
    return mark(parsed) ? true : c.fail("damage instruction whose damage can't be prevented");
  }) === true;
}

/** "The damage can't be prevented." directly after a sentence that only
 * deals damage. */
export function foldUnpreventableSentence(c: Cursor, previous: Sentence): boolean {
  return c.attempt(c => {
    if (!c.words("the", "damage", "can't", "be", "prevented") || (!c.punct(".") && !c.done())) return null;
    if (previous.otherwise) return c.fail("unambiguous damage instruction before its rider");
    return mark(previous.steps) ? true : c.fail("damage instruction whose damage can't be prevented");
  }) === true;
}
