// "creatures you controlled that were destroyed this way": the past tense
// places control at the event of the same resolution that the next qualifier
// names, not now; nothing in between can change control. A filter with an
// event or zone-change relation already describes each object as that event
// happened to it (CR 608.2h-i). Over a turn ("you controlled that died this
// turn") the time of control is not stated, so it is not read here.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

export interface PastControlGrammar {
  parsePlayerRef(c: Cursor): PlayerRef | null;
  parseQualifier(c: Cursor, head: ObjectFilter): ObjectFilter | null;
}

export function parsePastControlQualifier(c: Cursor, head: ObjectFilter, g: PastControlGrammar): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (head.control || head.controlledBy || head.relation) return null;
    const who = g.parsePlayerRef(c);
    if (!who || c.word("controlled") === null) return null;
    // "that were destroyed this way" is the relative form of "destroyed this way".
    c.attempt(c => c.word("that") !== null && c.word("was", "were") !== null ? true : null);
    const event = g.parseQualifier(c, head);
    if (!event) return null;
    const keys = Object.keys(event);
    if (keys.length !== 1) return null;
    const relation = event.relation;
    if (!relation || (relation.relation !== "event-result" && relation.relation !== "zone-change") || relation.scope !== "this-way" || relation.period) return null;
    return { relation, ...(who.player === "you" ? { control: "you" as const } : { controlledBy: who }) };
  });
}
