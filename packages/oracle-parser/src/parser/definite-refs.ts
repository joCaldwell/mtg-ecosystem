// Definite descriptions as anaphora: "the creature", "the token", "the spell",
// "the creature you control". Oracle text uses the definite article for an
// object the ability has already identified, exactly as "that creature" /
// "those tokens" (CR 608.2c: the instruction is read as written; the
// antecedent is bound later, never guessed here). Only plain descriptions
// (kind, type, color, controller) qualify; selections ("the creature with
// the greatest power"), event results ("the exiled card") and other definite
// forms have their own productions.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

const PLAIN_KEYS = new Set<keyof ObjectFilter>(["cls", "types", "subtypes", "supertypes", "colors", "control", "controlledBy"]);

/** "the attacking creature" / "the blocking creature": a combat role is a plain
 * description of the identified creature (CR 506.4, 509.1g), as in "the
 * blocking creature gets -1/-1". */
const COMBAT_ROLES = new Set(["attacking", "blocking"]);

export function parseDefiniteDescription(c: Cursor, g: {
  parseFilter: (c: Cursor) => ObjectFilter | null;
  number: (c: Cursor, start: number, end: number) => "singular" | "plural" | null;
}): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (c.word("the") === null) return null;
    const start = c.pos;
    const filter = g.parseFilter(c);
    if (!filter || !(Object.keys(filter) as (keyof ObjectFilter)[]).every(key => PLAIN_KEYS.has(key)
      || (key === "status" && filter.status?.every(status => COMBAT_ROLES.has(status))))) return null;
    const number = g.number(c, start, c.pos);
    if (!number) return null;
    return { ref: "that", filter, ...(number === "plural" ? { plural: true as const } : {}) };
  });
}
