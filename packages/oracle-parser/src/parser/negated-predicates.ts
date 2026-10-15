// Negated relative clauses on a described object: "that isn't a Demon",
// "that aren't legendary", "that aren't of the chosen type". Only negations
// that the conjunctive filter can state exactly are accepted (CR 205).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

type Negated = Pick<ObjectFilter, "nonTypes" | "nonSubtypes" | "nonSupertypes" | "nonColors">;

/** "Not (A or B)" is "not A and not B", so alternatives of single values
 * become one conjunctive exclusion list. "Not (A and B)" has no conjunctive
 * form and is rejected. */
function negate(filter: ObjectFilter, into: Negated): boolean {
  const keys = Object.keys(filter) as (keyof ObjectFilter)[];
  if (keys.length !== 1) return false;
  const single = (values: string[] | undefined) => values?.length === 1 ? values[0] : undefined;
  switch (keys[0]) {
    case "types": { const value = single(filter.types); if (!value) return false; (into.nonTypes ??= []).push(value); return true; }
    case "subtypes": { const value = single(filter.subtypes); if (!value) return false; (into.nonSubtypes ??= []).push(value); return true; }
    case "supertypes": { const value = single(filter.supertypes); if (!value) return false; (into.nonSupertypes ??= []).push(value); return true; }
    case "colors": { const value = filter.colors?.length === 1 ? filter.colors[0] : undefined; if (!value) return false; (into.nonColors ??= []).push(value); return true; }
    case "anyOf": return filter.anyOf!.every(branch => negate(branch, into));
    case "allOf": return filter.allOf!.length === 1 && negate(filter.allOf![0], into);
    default: return false;
  }
}

export function parseNegatedPredicate(c: Cursor, parsePredicate: (c: Cursor) => ObjectFilter | null): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (!c.words("that", "isn't") && !c.words("that", "aren't") && !c.words("that", "is", "not") && !c.words("that", "are", "not")) return null;
    const chosenType = c.attempt((c): ObjectFilter | null => {
      if (!c.words("of", "the", "chosen")) return null;
      const kind = c.word("card", "creature", "land", "artifact", "enchantment", "planeswalker") as NonNullable<ObjectFilter["chosenType"]>["kind"] | null;
      return c.word("type") !== null ? { notChosenType: { choice: "chosen-type", ...(kind ? { kind } : {}) } } : null;
    });
    if (chosenType) return chosenType;
    // "that isn't enchanted": a status without an antonym word of its own;
    // tapped/untapped and face up/down are stated positively instead.
    const status = c.attempt(c => {
      const word = c.word("enchanted", "equipped", "attacking", "blocking", "monstrous", "modified", "goaded", "suspected");
      return word && (c.done() || c.isPunct(".") || c.isPunct(",") || c.isWord("to", "and", "or", "from", "this")) ? word : null;
    });
    if (status) return { nonStatus: [status] };
    c.word("a", "an");
    const positive = parsePredicate(c);
    if (!positive) return null;
    // A trailing zone phrase locates the described object ("…that isn't a
    // God from your graveyard"); it is not part of what is negated.
    const { zone, ...denied } = positive;
    const negated: Negated = {};
    return negate(denied, negated) ? { ...negated, ...(zone ? { zone } : {}) } : c.fail("a negated type, subtype, supertype or color");
  });
}
