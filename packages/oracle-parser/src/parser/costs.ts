// Cost parsing: activation costs ("{T}, Sacrifice a creature:") and
// keyword parameter costs ("Equip {2}", "Ward—Pay 3 life.").

import type { Amount, Cost, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseAmount, parseCounterSpec, parseObjectRef, parseObjectSelection, parseZoneRef } from "./refs.ts";

import { isManaSymbol } from "../symbols.ts";

/** A run of `{…}` symbols → mana / tap / untap / energy cost items. */
export function parseSymbolCosts(c: Cursor): Cost[] | null {
  return c.attempt((c): Cost[] | null => {
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
      } else if (isManaSymbol(t.value)) {
        c.pos++;
        mana.push(t.value);
      } else {
        break;
      }
    }
    if (mana.length) costs.unshift({ cost: "mana", symbols: mana });
    if (energy) costs.push({ cost: "energy", amount: { amount: "fixed", value: energy } });
    return costs.length ? costs : null;
  });
}

/** Cost choices are payments, never target declarations (CR601.2c/h,
 * 602.2b). Dependent targets inside eligibility still require separate
 * grammar; accepting a target selector here would give it the wrong timing. */
function parsePaymentObjects(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    const what = parseObjectSelection(c);
    return what && what.ref !== "target" && what.ref !== "any-target" ? what : null;
  });
}

/** One non-symbol cost item: "Pay 2 life", "Sacrifice a creature", … */
function parseActionCost(c: Cursor): Cost | null {
  return c.attempt((c): Cost | null => {
    if (c.word("pay") !== null) {
      const symbols = c.attempt(parseSymbolCosts);
      if (symbols) return symbols.length === 1 ? symbols[0] : c.fail("single payment cost");
      const amount = parseAmount(c);
      if (!amount) return null;
      // CR 107.14: "Pay six {E}" and "Pay X {E}" remove that many
      // energy counters; the printed single symbol is a unit, not a count.
      // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
      const energySymbol = c.peek();
      if (energySymbol?.kind === "symbol" && energySymbol.value === "E") {
        c.symbol();
        return { cost: "energy", amount };
      }
      if (c.word("life") === null) return null;
      return { cost: "pay-life", amount };
    }
    if (c.word("sacrifice") !== null) {
      const what = parsePaymentObjects(c);
      return what && { cost: "sacrifice", what };
    }
    if (c.word("discard") !== null) {
      if (c.words("your", "hand")) return { cost: "discard", what: "hand" };
      const what = parsePaymentObjects(c);
      if (!what) return null;
      const random = c.words("at", "random");
      return { cost: "discard", what, ...(random ? { random: true } : {}) };
    }
    // Revealing a card is a payment, not discarding it (CR 118, 701.16).
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if (c.word("reveal") !== null) {
      const what = parsePaymentObjects(c);
      if (!what) return null;
      if (c.word("from") !== null) {
        const from = parseZoneRef(c);
        return from && { cost: "reveal", what, from };
      }
      return { cost: "reveal", what };
    }
    if (c.word("exile") !== null) {
      const what = parsePaymentObjects(c);
      if (!what) return null;
      if (c.word("from") !== null) {
        const from = parseZoneRef(c);
        return from && { cost: "exile", what, from };
      }
      return { cost: "exile", what };
    }
    if (c.word("tap") !== null) {
      const what = parsePaymentObjects(c);
      return what && { cost: "tap-objects", what };
    }
    // These are payment actions, not post-resolution effects. Exert keeps
    // its action identity, including CR 701.43's next-you untap restriction.
    // Mill costs must be payable in full (701.17b); untap is CR 701.26.
    if (c.word("untap") !== null) {
      const what = parsePaymentObjects(c);
      return what && { cost: "untap-objects", what };
    }
    if (c.word("exert") !== null) {
      const what = parsePaymentObjects(c);
      return what && { cost: "exert", what };
    }
    if (c.word("mill") !== null) {
      const amount = parseAmount(c);
      return amount && c.word("card", "cards") !== null ? { cost: "mill", amount } : null;
    }
    if (c.word("return") !== null) {
      const what = parsePaymentObjects(c);
      if (!what) return null;
      let from;
      if (c.word("from") !== null) {
        from = parseZoneRef(c);
        if (!from) return null;
      }
      // CR 400.3: the stated destination remains part of the instruction,
      // even where the ownership rule subsequently changes the actual zone.
      // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
      let owner: "your" | "owner";
      if (c.words("to", "your", "hand")) owner = "your";
      else {
        if (c.word("to") === null) return null;
        if (c.word("their", "its") === null) return null;
        if (c.possessive("owner", "owners") === null) return null;
        if (c.word("hand", "hands") === null) return null;
        owner = "owner";
      }
      return { cost: "return", what, to: "hand", owner, ...(from ? { from } : {}) };
    }
    if (c.word("remove") !== null) {
      const count: Amount | "all" | null = c.word("all") !== null ? "all" :
        c.words("any", "number", "of") ? { amount: "chosen", minimum: 0 } :
        c.words("one", "or", "more") ? { amount: "chosen", minimum: 1 } : parseAmount(c);
      if (!count) return null;
      if (c.isWord("or")) return c.fail("supported counter-payment quantity");
      const counter = c.isWord("counter", "counters") ? "any" as const : parseCounterSpec(c);
      if (!counter || c.word("counter", "counters") === null) return null;
      if (c.word("from") === null) return null;
      const distributed = c.word("among") !== null;
      const from = parseObjectRef(c);
      return from && { cost: "remove-counters", counter, count, from, ...(distributed ? { distributed: true } : {}) };
    }
    if (c.word("put") !== null) {
      const count = parseAmount(c);
      const counter = parseCounterSpec(c);
      if (!count || !counter || c.word("counter", "counters") === null || c.word("on") === null) return null;
      const on = parseObjectRef(c);
      return on && { cost: "put-counters", counter, count, on };
    }
    // Preserve keyword-action identity: these actions may trigger abilities
    // even when their underlying payment resembles ordinary exile or mana.
    // CR 701.59, 701.67: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if (c.words("collect", "evidence")) {
      const amount = parseAmount(c);
      return amount && { cost: "collect-evidence", amount };
    }
    if (c.word("forage") !== null) return { cost: "forage" };
    if (c.word("blight") !== null) {
      const amount = parseAmount(c);
      return amount && { cost: "blight", amount };
    }
    if (c.word("waterbend") !== null) {
      const costs = parseSymbolCosts(c);
      return costs?.length === 1 && costs[0].cost === "mana" ?
        { cost: "waterbend", symbols: costs[0].symbols } : null;
    }
    const getCounters = c.attempt((c): Cost | null => {
      c.word("you");
      if (c.word("get") === null) return null;
      const count = parseAmount(c);
      const counter = parseCounterSpec(c);
      if (!count || !counter || c.word("counter", "counters") === null) return null;
      return { cost: "get-counters", counter, count };
    });
    if (getCounters) return getCounters;
    return c.fail("cost");
  });
}

/** Full comma-separated cost list before the colon of an activated ability. */
function parseConjunctiveCosts(c: Cursor): Cost[] | null {
  return c.attempt((c): Cost[] | null => {
    const costs: Cost[] = [];
    for (;;) {
      const symbols = c.attempt(parseSymbolCosts);
      if (symbols) costs.push(...symbols);
      else {
        const action = parseActionCost(c);
        if (!action) return costs.length ? c.fail("cost after separator") : null;
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
  });
}

/** Alternatives preserve grouped payments: paying mana OR discarding is not
 * paying both. CR 118 and 601.2f-h govern the selected cost's full payment.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export function parseCostList(c: Cursor): Cost[] | null {
  return c.attempt((c): Cost[] | null => {
    const first = parseConjunctiveCosts(c);
    if (!first) return null;
    const options = [first];
    while (c.word("or") !== null) {
      const next = parseConjunctiveCosts(c);
      if (!next) return null;
      options.push(next);
    }
    // A comma before an action-level alternative can have a shared prefix
    // ("{3}, {T}, Sacrifice an artifact or discard a card"). Until grouped
    // action alternatives are represented explicitly, do not reinterpret
    // the mandatory prefix as part of only the first alternative. Pure
    // symbol alternatives are unambiguous (the five Shards in Mirrodin).
    if (options.length > 1 && options.some(option => option.length > 1) &&
        !options.every(option => option.every(cost =>
          ["mana", "tap-self", "untap-self", "energy"].includes(cost.cost)))) {
      return c.fail("unambiguous grouped cost alternatives");
    }
    return options.length === 1 ? first : [{ cost: "choice", options }];
  });
}
