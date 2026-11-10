// "Put a charge counter on this artifact or remove one from it.": the second
// alternative leaves out the counter kind, which is the kind the first
// alternative names (CR 608.2c). It is the same instruction as "remove a
// charge counter from it", offered as the same one-actor choice (CR 608.2d).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

/** Called after "or" with the alternative directly before it. */
export function parseElidedCounterRemoval(c: Cursor, previous: Effect, parseObjectRef: (c: Cursor) => ObjectRef | null): Effect | null {
  return c.attempt((c): Effect | null => {
    if (previous.effect !== "put-counters" || previous.who) return null;
    if (previous.count.amount !== "fixed" || previous.count.value !== 1) return null;
    if (!c.words("remove", "one", "from")) return null;
    const from = parseObjectRef(c);
    return from && { effect: "remove-counters", counter: previous.counter, count: { amount: "fixed", value: 1 }, from };
  });
}
