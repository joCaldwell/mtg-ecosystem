// "Spend only [black] mana on X." / "Spend only mana produced by [basic
// lands] to cast this spell." These limit which mana may pay the spell's or
// ability's own cost as it is paid (CR 601.2h, 602.2b; X: 107.3). They are
// not properties of produced mana (106.6) and never resolve.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Color, PaymentRestriction } from "../ast.ts";
import { COLORS } from "../vocab.ts";
import type { Cursor } from "./cursor.ts";
import { parseFilter } from "./refs.ts";

function parseColors(c: Cursor): Color[] | null {
  return c.attempt(c => {
    const colors: Color[] = [];
    for (;;) {
      const word = c.anyWord();
      if (!word || !COLORS.has(word.value) || colors.includes(word.value as Color)) return null;
      colors.push(word.value as Color);
      // "black and/or red mana": mana of either listed color qualifies.
      if (!c.attempt(c => c.word("and") !== null && c.punct("/") && c.word("or") !== null ? true : null)) break;
    }
    return colors;
  });
}

/** `purpose` is the closing phrase the caller's position allows for a limit
 * on the whole cost; limits on X are the same everywhere. Consumes complete
 * sentences, including the trailing period when present. */
export function parsePaymentRestriction(c: Cursor, purpose: "cast-this-spell" | "cast-it-this-way" | "x-only"): PaymentRestriction | null {
  return c.attempt((c): PaymentRestriction | null => {
    if (!c.words("spend", "only")) return null;
    let restriction: PaymentRestriction;
    if (c.words("mana", "produced", "by")) {
      const sources = parseFilter(c);
      if (!sources || sources.zone) return null;
      const closed = purpose === "cast-this-spell" ? c.words("to", "cast", "this", "spell")
        : purpose === "cast-it-this-way" ? c.words("to", "cast", "it", "this", "way") : false;
      if (!closed) return null;
      restriction = { part: "total", mana: { mana: "produced-by", sources } };
    } else {
      const colored = c.word("colored") !== null;
      const colors = colored ? null : parseColors(c);
      if (!colored && !colors) return null;
      if (!c.words("mana", "on", "x")) return null;
      restriction = { part: "x", mana: colors ? { mana: "colors", colors } : { mana: "colored" } };
    }
    if (!c.punct(".") && !c.done()) return null;
    if (restriction.mana.mana === "colored" && c.words("no", "more", "than", "one", "mana", "of", "each", "color", "may", "be", "spent", "this", "way")) {
      if (!c.punct(".") && !c.done()) return null;
      restriction.maximumPerColor = 1;
    }
    return restriction;
  });
}
