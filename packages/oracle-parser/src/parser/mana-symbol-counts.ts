// Chroma (an ability word, CR 207.2c) counts the mana symbols of one color in
// the mana costs of a set of objects. A hybrid or Phyrexian symbol of that
// color is a mana symbol of that color (CR 107.4e–f). This is not devotion
// (700.5): effects that modify devotion (700.5a) do not apply to it.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Color, ObjectFilter } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { COLORS } from "../vocab.ts";

/** After "the number of": "<color> mana symbols in the mana costs of <objects>". */
export function parseManaSymbolCount(c: Cursor, parseFilter: (c: Cursor) => ObjectFilter | null): Amount | null {
  return c.attempt((c): Amount | null => {
    const word = c.anyWord();
    if (!word || !COLORS.has(word.value) || !c.words("mana", "symbols", "in", "the", "mana", "costs", "of")) return null;
    const among = parseFilter(c);
    // The counted objects must be a described set, not one anaphoric object.
    return among && !among.relation ? { amount: "mana-symbols", color: word.value as Color, in: among } : null;
  });
}
