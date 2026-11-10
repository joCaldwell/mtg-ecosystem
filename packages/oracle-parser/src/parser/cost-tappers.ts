// "creature that crewed it this turn", "creatures that saddled it this turn",
// "each creature that convoked this spell": the creatures tapped to pay a
// crew or saddle activation cost (CR 702.122b-c, 702.171c) or, by convoke,
// mana in a spell's total cost (702.51c). A history of the tapped object, not
// its current tapped status.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

const ACTIONS = { crewed: "crew", saddled: "saddle", convoked: "convoke" } as const;

/** Crewing and saddling happen any number of times, so they need the printed
 * "this turn"; a spell is convoked only while it is being cast. The object
 * must be a definite reference to one Vehicle, mount or spell. */
export function parseCostTapperQualifier(c: Cursor, parseObjectRef: (c: Cursor) => ObjectRef | null): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (c.word("that") === null) return null;
    const verb = c.word("crewed", "saddled", "convoked") as keyof typeof ACTIONS | null;
    if (!verb) return null;
    const of = parseObjectRef(c);
    if (!of || !["self", "this", "it"].includes(of.ref)) return null;
    const action = ACTIONS[verb];
    const thisTurn = c.words("this", "turn");
    if (thisTurn === (action === "convoke")) return null;
    return { relation: { relation: "tapped-for-cost", action, of, ...(thisTurn ? { period: "this-turn" as const } : {}) } };
  });
}
