// Payments stated as a quantity rather than printed symbols: "any amount of
// life", "any amount of mana", "any amount of {R}", "life equal to its power",
// "mana equal to ...". CR 118.1-118.3, 119.4 (paying life), 107.1b-107.3
// (a chosen number). "Any amount" lets the payer choose any number, including
// 0; a player can always pay 0 (CR 118.3, 119.4).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Cost } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseAmount } from "./refs.ts";

const MANA_UNITS = new Set(["W", "U", "B", "R", "G", "C"]);

/** After "pay": a quantity of life or mana. */
export function parseStatedPayment(c: Cursor): Cost | null {
  return c.attempt((c): Cost | null => {
    const chosen = c.words("any", "amount", "of");
    if (chosen) {
      const amount: Amount = { amount: "chosen", minimum: 0 };
      if (c.word("life") !== null) return { cost: "pay-life", amount };
      if (c.word("mana") !== null) return { cost: "mana-amount", amount };
      const symbol = c.peek();
      if (symbol?.kind !== "symbol" || !MANA_UNITS.has(symbol.value)) return null;
      c.pos++;
      return { cost: "mana-amount", amount, unit: symbol.value };
    }
    const resource = c.word("life", "mana");
    if (!resource || !c.words("equal", "to")) return null;
    const amount = parseAmount(c);
    if (!amount) return null;
    return resource === "life" ? { cost: "pay-life", amount } : { cost: "mana-amount", amount };
  });
}
