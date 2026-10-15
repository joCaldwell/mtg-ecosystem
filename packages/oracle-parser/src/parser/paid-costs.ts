// The object of the verb "pay" when it lists mana and life: "pay {1} and 1
// life" is both payments, "pays {1} or 1 life" is one of them chosen by the
// payer (CR 118.1–118.3, 119.4). Single payments keep their existing grammar.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Cost } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseSymbolCosts } from "./costs.ts";
import { parseAmount } from "./refs.ts";

/** Mana symbols or "<amount> life". */
function parseItem(c: Cursor): Cost | null {
  return c.attempt((c): Cost | null => {
    const symbols = c.attempt(parseSymbolCosts);
    if (symbols) return symbols.length === 1 && symbols[0].cost === "mana" ? symbols[0] : null;
    const amount = parseAmount(c);
    return amount && c.word("life") !== null ? { cost: "pay-life", amount } : null;
  });
}

/** "<item> and <item>" or "<item> or <item>"; mixed "and"/"or" lists are left
 * unsupported (their grouping is ambiguous). At least one item is life. */
export function parsePaidCosts(c: Cursor): Cost[] | null {
  return c.attempt((c): Cost[] | null => {
    const items: Cost[] = [];
    const first = parseItem(c);
    if (!first) return null;
    items.push(first);
    const joiner = c.word("and", "or");
    if (!joiner) return null;
    for (;;) {
      const item = parseItem(c);
      if (!item) return null;
      items.push(item);
      if (!c.isWord("and", "or")) break;
      if (c.word(joiner) === null) return null;
    }
    if (!items.some(item => item.cost === "pay-life")) return null;
    return joiner === "and" ? items : [{ cost: "choice", options: items.map(item => [item]) }];
  });
}

/** "Pay {B} and 1 life" in a cost list: the verb governs the life payment too. */
export function parsePaidLifeContinuation(c: Cursor): Cost | null {
  return c.attempt((c): Cost | null => {
    if (c.word("and") === null) return null;
    const amount = parseAmount(c);
    return amount && c.word("life") !== null && !c.isWord("or") ? { cost: "pay-life", amount } : null;
  });
}
