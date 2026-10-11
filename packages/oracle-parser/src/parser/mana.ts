// Mana production is distinct from payment. CR 106.8-.11 define production
// even for hybrid, Phyrexian, generic, and snow symbols; keep their validated
// symbols so those rules determine the produced color/type and quantity.
// CR 106.6 governs spending restrictions, 106.7 "could produce" choices.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ManaProduction, ManaSpendingRestriction, ObjectFilter, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { isManaSymbol } from "../symbols.ts";
import { parseAmount, parseFilter, parseObjectRef, parsePlayerRef } from "./refs.ts";
import { parseChosenValueRef } from "./choices.ts";

function readSymbols(c: Cursor): string[] | null {
  return c.attempt(c => {
    const symbols: string[] = [];
    while (c.peek()?.kind === "symbol") {
      const token = c.peek();
      if (token?.kind !== "symbol" || !isManaSymbol(token.value)) return c.fail("mana-production symbol");
      symbols.push(token.value);
      c.pos++;
    }
    return symbols.length ? symbols : null;
  });
}

function parseAtomicProduction(c: Cursor): ManaProduction | null {
  return c.attempt(c => {
    if (c.peek()?.kind === "symbol") {
      const symbols = readSymbols(c);
      return symbols ? { mana: "fixed", symbols } : null;
    }
    const scaled = c.attempt((c): ManaProduction | null => {
      if (!c.words("an", "amount", "of")) return null;
      const symbol = c.symbol();
      if (!symbol || !/^[WUBRGC]$/.test(symbol) || !c.words("equal", "to")) return null;
      const amount = parseAmount(c);
      return amount && { mana: "scaled-symbol", symbol, amount };
    });
    if (scaled) return scaled;
    const amount = parseAmount(c);
    if (!amount || c.word("mana") === null) return null;
    if (c.words("in", "any", "combination", "of")) {
      if (c.word("colors") !== null) return { mana: "combination", amount, options: "colors" };
      const options: string[] = [];
      for (;;) {
        const symbol = c.symbol();
        // Listed choices are mana types, not multi-unit bundles or costs.
        if (!symbol || !/^[WUBRGC]$/.test(symbol)) return null;
        options.push(symbol);
        const join = c.attempt(c => {
          const comma = c.punct(",");
          const and = c.word("and") !== null;
          const slash = and && c.punct("/");
          if (slash && c.word("or") === null) return null;
          const or = !and && c.word("or") !== null;
          return comma || and || or ? true : null;
        });
        if (!join) break;
      }
      return { mana: "combination", amount, options };
    }
    if (c.word("of") === null) return null;
    const chosen = parseChosenValueRef(c);
    if (chosen) return chosen.category === "color" ? { mana: "chosen-color", amount,
      choice: { ...chosen, category: "color" } } : null;
    if (c.word("any") === null) return null;
    if (c.words("one", "color")) return { mana: "any-one-color", amount };
    const kind = c.word("color", "type");
    if (!kind) return null;
    const productionSource = c.attempt((c): ManaProduction | null => {
      const what = parseObjectRef(c) ?? c.attempt(c => {
        if (c.word("that") === null) return null;
        return parseObjectRef(c);
      });
      if (!what) return null;
      if (c.words("could", "produce")) return { mana: "could-produce", amount, what, colorOnly: kind === "color" };
      return c.word("produced") !== null ? { mana: "produced", amount, what, colorOnly: kind === "color" } : null;
    });
    if (productionSource) return productionSource;
    if (kind === "type") return { mana: "any-type", amount };
    if (c.attempt(c => c.words("in", "your") && c.possessive("commander") !== null && c.words("color", "identity") ? true : null))
      return { mana: "commander-color", amount, whose: { player: "you" } };
    return { mana: "any-color", amount };
  });
}

/** CR 106: a production alternative can be a fixed mana bundle or a
 * symbolic color/amount. All options are choices by the stated recipient;
 * they are not separate add instructions and are not simultaneously added. */
function parseProduction(c: Cursor): ManaProduction | null {
  return c.attempt(c => {
    const first = parseAtomicProduction(c);
    if (!first) return null;
    const options = [first];
    let finalOr = false;
    for (;;) {
      const start = c.pos;
      const comma = c.punct(",");
      const or = c.word("or") !== null;
      if (!comma && !or) break;
      const option = parseAtomicProduction(c);
      if (!option) {
        // An ordinary coordinated action may follow a comma. An explicit
        // "or" after a production always needs another complete production.
        if (comma && !or) { c.pos = start; break; }
        return null;
      }
      options.push(option); finalOr = or;
    }
    if (options.length > 1 && !finalOr) return c.fail("final mana-choice conjunction");
    return options.length === 1 ? first : { mana: "choice", options };
  });
}

function parseSpendingUse(c: Cursor): ManaSpendingRestriction["uses"][number] | null {
  return c.attempt(c => {
    if (c.word("cast") !== null) {
      if (c.words("your", "commander")) return { use: "cast-commander", whose: { player: "you" } };
      c.word("a", "an");
      const filter = parseFilter(c);
      if (!filter) return null;
      // The restricted purpose is casting from a zone, not casting a spell
      // currently in that zone. Karolina Dean, Runaway forbids hand-origin
      // spells; by payment time those spells are already on the stack.
      const nestedZone = (filter: ObjectFilter): boolean => Boolean(filter.zone) ||
        (filter.allOf ?? []).some(nestedZone) || (filter.anyOf ?? []).some(nestedZone);
      if ([...(filter.allOf ?? []), ...(filter.anyOf ?? [])].some(nestedZone)) return null;
      const { zone: from, ...qualities } = filter;
      return { use: "cast", filter: qualities, ...(from ? { from } : {}) };
    }
    if (c.word("activate") === null) return null;
    c.word("an");
    if (c.word("ability", "abilities") === null || c.word("of") === null) return null;
    c.word("a", "an");
    const source = parseFilter(c);
    return source && { use: "activate", source };
  });
}

function parseRestriction(c: Cursor): ManaSpendingRestriction | null {
  return c.attempt(c => {
    let restriction: ManaSpendingRestriction["restriction"];
    if (c.words("spend", "this", "mana", "only", "to")) restriction = "only";
    else if (c.words("this", "mana", "can't", "be", "spent", "to")) restriction = "not";
    else return null;
    const first = parseSpendingUse(c);
    if (!first) return null;
    const uses = [first];
    while (c.word("or") !== null) {
      const next = parseSpendingUse(c);
      if (!next) return null;
      uses.push(next);
    }
    // A modifier or extra effect after the restriction belongs to this mana
    // rider, not to the preceding add instruction's resolution condition.
    if (!c.done() && !c.isPunct(".")) return c.fail("end of mana-spending restriction");
    return { restriction, uses };
  });
}

/** Actor state is committed by the caller only on success; failure rolls back
 * all tokens, including optional attached spending-restriction sentences. */
export function parseAddManaEffect(c: Cursor, previousPlayer?: PlayerRef): { effects: Effect[]; actor: PlayerRef } | null {
  return c.attempt(c => {
    const explicit = parsePlayerRef(c);
    const verb = c.word("add", "adds");
    if (!verb) return null;
    const actor = explicit ?? (verb === "add" && previousPlayer?.player !== "they" ? { player: "you" as const } : previousPlayer);
    if (!actor) return c.fail("actor of mana production");
    c.words("an", "additional");
    const mana = parseProduction(c);
    if (!mana) return null;
    const restriction = c.attempt(c => {
      if (!c.punct(".")) return null;
      return parseRestriction(c);
    });
    return { actor, effects: [{ effect: "add-mana", who: actor, mana, ...(restriction ? { restriction } : {}) }] };
  });
}
