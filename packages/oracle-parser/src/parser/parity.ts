// "with mana value of the chosen quality", "with power of that quality",
// "with even mana values": whether a number characteristic is odd or even,
// as printed or as the linked "choose odd or even" value (CR 607.2d). Zero is
// even; an object without a mana cost has mana value 0 (CR 202.3a).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

type Parity = NonNullable<ObjectFilter["parity"]>;

function property(c: Cursor, plural: boolean): Parity["property"] | null {
  if (c.word("power") !== null) return "power";
  return c.word("mana") !== null && c.word(plural ? "values" : "value", "value") !== null ? "mana-value" : null;
}

export function parseParityQualifier(c: Cursor): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (c.word("with") === null) return null;
    const literal = c.word("odd", "even") as "odd" | "even" | null;
    if (literal) {
      const attribute = property(c, true);
      return attribute && { parity: { property: attribute, parity: literal } };
    }
    const attribute = property(c, false);
    if (!attribute || c.word("of") === null) return null;
    if (!c.words("the", "chosen") && c.word("that") === null) return null;
    if (c.word("quality") === null) return null;
    return { parity: { property: attribute, parity: { choice: "chosen-value", category: "parity", which: "chosen" } } };
  });
}
