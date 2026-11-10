// CR 702.95b-e: soulbond pairs two creatures. Abilities may ask whether a
// creature is paired and may refer to both members of its pair.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, ObjectFilter, ObjectRef, StaticEffect, StaticModifier } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { bindOtherExclusion } from "./other-exclusion.ts";

type PairedCondition = Extract<Condition, { condition: "paired" }>;
type Modifiers = (c: Cursor, what: ObjectRef) => { modifiers: StaticModifier[]; condition?: Condition } | null;

/** "[this creature] is paired with another creature". The contrast of
 * "another" is the stated subject, so it is bound here instead of being left
 * as an unresolved lexical marker. */
export function parsePairedCondition(c: Cursor, g: { parseObjectRef(c: Cursor): ObjectRef | null; parseFilter(c: Cursor): ObjectFilter | null }): PairedCondition | null {
  return c.attempt(c => {
    const what = g.parseObjectRef(c);
    if (!what || (what.ref !== "self" && what.ref !== "this") || !c.words("is", "paired", "with", "another")) return null;
    const partner = g.parseFilter(c);
    // 702.95a: only creatures become paired.
    if (!partner || partner.types?.length !== 1 || partner.types[0] !== "creature" || Object.keys(partner).length !== 1) return null;
    const bound = bindOtherExclusion({ ...partner, other: true }, what);
    return bound && { condition: "paired", what, with: bound };
  });
}

/** "As long as [it] is paired …, both creatures have / each of those
 * creatures has|gets …": one continuous effect on both members of the pair.
 * The pair subject exists only under its own paired condition. */
export function parsePairStatic(c: Cursor, condition: Condition, modifiers: Modifiers): StaticEffect | null {
  if (condition.condition !== "paired") return null;
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("both", "creatures") && !c.words("each", "of", "those", "creatures")) return null;
    const what: ObjectRef = { ref: "soulbond-pair", of: structuredClone(condition.what) };
    const group = modifiers(c, what);
    if (!group || group.condition) return null;
    c.punct(".");
    return c.done() ? { static: "modify", what, modifiers: group.modifiers } : null;
  });
}
