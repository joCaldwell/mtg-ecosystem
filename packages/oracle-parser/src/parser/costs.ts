// Cost parsing: activation costs ("{T}, Sacrifice a creature:") and
// keyword parameter costs ("Equip {2}", "Ward—Pay 3 life.").

import type { Cost } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseAmount, parseCounterSpec, parseObjectRef } from "./refs.ts";

const MANA_SYMBOL = /^([WUBRGCSXYZ]|[0-9]+|[WUBRG2C]\/[WUBRGP]|[WUBRG]\/[WUBRG]\/P)$/;

/** A run of `{…}` symbols → mana / tap / untap / energy cost items. */
export function parseSymbolCosts(c: Cursor): Cost[] | null {
  const costs: Cost[] = [];
  let mana: string[] = [];
  let energy = 0;
  for (;;) {
    const t = c.peek();
    if (t?.kind !== "symbol") break;
    if (t.value === "T") {
      c.pos++;
      costs.push({ cost: "tap-self" });
    } else if (t.value === "Q") {
      c.pos++;
      costs.push({ cost: "untap-self" });
    } else if (t.value === "E") {
      c.pos++;
      energy++;
    } else if (MANA_SYMBOL.test(t.value)) {
      c.pos++;
      mana.push(t.value);
    } else {
      break;
    }
  }
  if (mana.length) costs.unshift({ cost: "mana", symbols: mana });
  if (energy) costs.push({ cost: "energy", amount: energy });
  return costs.length ? costs : null;
}

/** One non-symbol cost item: "Pay 2 life", "Sacrifice a creature", … */
function parseActionCost(c: Cursor): Cost | null {
  return c.attempt((c): Cost | null => {
    if (c.word("pay") !== null) {
      const symbols = c.attempt(parseSymbolCosts);
      if (symbols) return symbols[0] ?? null;
      const amount = parseAmount(c);
      if (!amount || c.word("life") === null) return null;
      return { cost: "pay-life", amount };
    }
    if (c.word("sacrifice") !== null) {
      const what = parseObjectRef(c);
      return what && { cost: "sacrifice", what };
    }
    if (c.word("discard") !== null) {
      if (c.words("your", "hand")) return { cost: "discard", what: "hand" };
      const what = parseObjectRef(c);
      return what && { cost: "discard", what };
    }
    if (c.word("exile") !== null) {
      const what = parseObjectRef(c);
      return what && { cost: "exile", what };
    }
    if (c.word("tap") !== null) {
      const what = parseObjectRef(c);
      return what && { cost: "tap-objects", what };
    }
    if (c.word("return") !== null) {
      const what = parseObjectRef(c);
      if (!what) return null;
      if (!c.words("to", "its", "owner's", "hand") && !c.words("to", "your", "hand")) {
        // "to their owner's hand" (plural) — owner is a possessive token
        if (c.word("to") === null) return null;
        if (c.word("their", "its") === null) return null;
        if (c.possessive("owner", "owners") === null) return null;
        if (c.word("hand", "hands") === null) return null;
      }
      return { cost: "return", what, to: "hand" };
    }
    if (c.word("remove") !== null) {
      const count = parseAmount(c);
      if (!count) return null;
      const counter = parseCounterSpec(c);
      if (!counter || c.word("counter", "counters") === null) return null;
      if (c.word("from") === null) return null;
      const from = parseObjectRef(c);
      return from && { cost: "remove-counters", counter, count, from };
    }
    return c.fail("cost");
  });
}

/** Full comma-separated cost list before the colon of an activated ability. */
export function parseCostList(c: Cursor): Cost[] | null {
  const costs: Cost[] = [];
  for (;;) {
    const symbols = c.attempt(parseSymbolCosts);
    if (symbols) costs.push(...symbols);
    else {
      const action = parseActionCost(c);
      if (!action) break;
      costs.push(action);
    }
    if (c.isPunct(",")) {
      c.punct(",");
      c.word("and");
      continue;
    }
    if (c.isWord("and")) {
      c.word("and");
      continue;
    }
    break;
  }
  return costs.length ? costs : null;
}
