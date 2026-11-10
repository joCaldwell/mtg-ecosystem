// "A number of <cards> equal to <amount>": the same counted selection as
// "<N> <cards>", with the count stated after the noun phrase (CR 107.1,
// 608.2h: the amount is determined once, as the instruction is carried out).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, ObjectFilter, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

interface Grammar {
  parseFilter(c: Cursor): ObjectFilter | null;
  parseEqualTo(c: Cursor): Amount | null;
}

export function parseNumberOfSelection(c: Cursor, g: Grammar): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (!c.words("a", "number", "of")) return null;
    const filter = g.parseFilter(c);
    const count = filter && g.parseEqualTo(c);
    if (!filter || !count) return null;
    const { zone, ...rest } = filter;
    if (zone?.position) {
      // "a number of cards from the top of your library equal to N" is that
      // library's top N cards (CR 401.1), not a choice among its cards.
      if (zone.zone !== "library" || !zone.owner || zone.depth !== undefined || Object.keys(rest).length !== 1 || rest.cls !== "card") return null;
      const { position, ...of } = zone;
      return { ref: "library-cards", position, count, of };
    }
    return { ref: "filter", filter, count };
  });
}
