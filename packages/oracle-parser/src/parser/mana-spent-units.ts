// "for each mana [from a Treasure] spent to cast it": the unit counts the
// units of mana actually spent (CR 601.2f/h, 609.4b), optionally only those
// produced by a source matching a description (CR 106.3), never the spell's
// mana cost or mana value.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseFilter, parseObjectRef } from "./refs.ts";

/** After "for each" / "each": "mana [from <a|an source>] [that was] spent to cast <spell>". */
export function parseManaSpentUnit(c: Cursor): Amount | null {
  // "for each {S} spent to cast this spell": each mana produced by a snow
  // source among the mana spent (CR 107.4h), as "the amount of {S} spent".
  const snow = c.attempt((c): Amount | null => {
    const symbol = c.peek();
    if (symbol?.kind !== "symbol" || symbol.value !== "S") return null;
    c.pos++;
    if (!c.words("spent", "to", "cast")) return null;
    const what = parseObjectRef(c);
    return what ? { amount: "mana-spent", what, quantity: "snow" } : null;
  });
  if (snow) return snow;
  return c.attempt((c): Amount | null => {
    if (c.word("mana") === null) return null;
    let source;
    if (c.word("from") !== null) {
      if (c.word("a", "an") === null) return null;
      source = parseFilter(c);
      if (!source) return null;
    }
    c.words("that", "was");
    if (!c.words("spent", "to", "cast")) return null;
    const what = parseObjectRef(c);
    return what ? { amount: "mana-spent", what, quantity: "total", ...(source ? { source } : {}) } : null;
  });
}
