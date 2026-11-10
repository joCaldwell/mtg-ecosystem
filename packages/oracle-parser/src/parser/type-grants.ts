// "gains all creature types" / "loses all creature types": a type-changing
// effect worded like an ability grant. CR 205.1b, 205.3m: every creature type
// is added to (or removed from) the object's subtypes; nothing else changes.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { TypeChange } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

/** After the verb. `verb` decides whether the types are added or removed. */
export function parseAllCreatureTypes(c: Cursor, verb: string): TypeChange | null {
  return c.attempt((c): TypeChange | null => {
    const operation = verb.startsWith("gain") ? "add" as const : verb.startsWith("lose") ? "remove" as const : null;
    if (!operation || !c.words("all", "creature", "types")) return null;
    // "… and abilities" / "… in addition to" would be a different instruction.
    if (c.isWord("and") && c.peek(1)?.kind === "word" && (c.peek(1) as { value: string }).value === "abilities") return null;
    return { subtypes: [{ family: "creature", operation, types: { all: true } }] };
  });
}

/** "loses all land types and abilities" / "loses all creature types and
 * abilities": both removals of one continuous effect, in layer 4 and layer
 * 6 (CR 205.3m, 613.1d/f). After the verb; only "lose(s)". */
export function parseLoseAllTypesAndAbilities(c: Cursor, verb: string): TypeChange | null {
  return c.attempt((c): TypeChange | null => {
    if (!verb.startsWith("lose") || c.word("all") === null) return null;
    const family = c.word("land", "creature");
    if (!family || !c.words("types", "and", "abilities")) return null;
    return { subtypes: [{ family: family as "land" | "creature", operation: "remove", types: { all: true } }] };
  });
}
