// Quantities and relations defined by what a keyword did to this object's
// own spell or entry: kicker payments (CR 702.33, 400.7d) and devoured
// permanents (CR 702.82b). Only an already identified object can be asked.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, CounterSpec, ObjectRef, ObjectRelation } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

type RefParser = (c: Cursor) => ObjectRef | null;

function parseIdentified(c: Cursor, parseObjectRef: RefParser): ObjectRef | null {
  return c.attempt(c => {
    const what = parseObjectRef(c);
    return what && (what.ref === "it" || what.ref === "self" || what.ref === "this") ? what : null;
  });
}

/** After "for each" or "the number of": "time(s) it was kicked". */
export function parseKickCount(c: Cursor, parseObjectRef: RefParser): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word("time", "times") === null) return null;
    const of = parseIdentified(c, parseObjectRef);
    if (!of) return null;
    // CR 702.140d: "time(s) this creature has mutated".
    if (c.words("has", "mutated")) return { amount: "mutation-count", of };
    return c.words("was", "kicked") ? { amount: "kicker-count", of } : null;
  });
}

/** Filter qualifier "[creature] it devoured". */
export function parseDevouredRelation(c: Cursor, parseObjectRef: RefParser): ObjectRelation | null {
  return c.attempt((c): ObjectRelation | null => {
    const by = parseIdentified(c, parseObjectRef);
    return by && c.word("devoured") !== null ? { relation: "devoured", by } : null;
  });
}

/** Filter qualifier "that had [a / a double strike] counter(s) put on it /
 * them this way": the objects an earlier instruction of this resolution
 * actually put counters on (CR 122.1, 608.2c); a bare "counters" is any kind. */
export function parseCountersPutRelation(c: Cursor, parseCounterSpec: (c: Cursor) => CounterSpec | null): ObjectRelation | null {
  return c.attempt((c): ObjectRelation | null => {
    if (!c.words("that", "had")) return null;
    c.word("a", "an");
    const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
    if (!counter || c.word("counter", "counters") === null) return null;
    if (!c.words("put", "on") || c.word("it", "them") === null || !c.words("this", "way")) return null;
    return { relation: "counters-put", counter, scope: "this-way" };
  });
}
