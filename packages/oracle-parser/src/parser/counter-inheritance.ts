// "Put its counters on target creature you control." CR 122.2: the counters of
// an object that left the battlefield ceased to exist, so the instruction puts
// new counters of the same kinds and numbers (608.2h) rather than moving any.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";

type Inherited = Extract<Effect, { effect: "put-counters-of" }>;

/** Only your own imperative "put": another actor or an elided one would need
 * the actor recorded, which this instruction has no printed case for. */
export function parseInheritedCounters(c: Cursor, parseObjectRef: (c: Cursor) => ObjectRef | null): Inherited | null {
  return c.attempt((c): Inherited | null => {
    if (c.word("put") === null) return null;
    const counters: Inherited["counters"] | null = c.words("its", "counters") ? { of: { ref: "it" } }
      : c.words("those", "counters") ? { ref: "those-counters", scope: "contextual" } : null;
    if (!counters || c.word("on") === null) return null;
    const on = parseObjectRef(c);
    return on && { effect: "put-counters-of", counters, on };
  });
}
