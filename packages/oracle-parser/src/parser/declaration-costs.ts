// Costs to attack or block: "[creatures] can't attack/block unless [a player]
// [pays a cost]". CR 508.1h / 509.1d: the cost is determined and paid while
// attackers or blockers are declared, and nobody is required to pay it
// (508.1d, 509.1c). It is never an "unless" choice made on resolution
// (118.12a), nor a continuously tested condition.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, CombatDefender, CombatRule, Cost, ObjectFilter, ObjectRef, PlayerRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseCostList } from "./costs.ts";
import { parseAmount, parseCounterSpec, parseFilter, parseObjectRef } from "./refs.ts";

export type DeclarationCost = Extract<CombatRule, { rule: "declaration-cost" }>;

export interface DeclarationCostGrammar {
  subject(c: Cursor): ObjectRef | null;
  defender(c: Cursor): CombatDefender | null;
  objects(c: Cursor): ObjectRef | null;
}

const PAYMENT_VERBS: Record<string, string> = {
  pay: "pay", pays: "pay", sacrifice: "sacrifice", sacrifices: "sacrifice",
  discard: "discard", discards: "discard", return: "return", returns: "return",
};

/** One creature per application: its own printed subject, or a distributive
 * "each creature ..." whose payer is "its controller". */
const SINGLE_SUBJECTS = new Set(["self", "this", "enchanted", "equipped", "it"]);

/** A count of the creatures being declared, e.g. "creature they control
 * that's attacking you" or "blocking creature they control". */
function countsDeclaredCreatures(filter: ObjectFilter): boolean {
  return Boolean(filter.attacking) || (filter.status ?? []).some(status => status === "attacking" || status === "blocking");
}

function parsePayment(c: Cursor): Cost[] | null {
  return c.attempt(c => {
    const verb = c.peek();
    if (verb?.kind !== "word" || verb.possessive || !PAYMENT_VERBS[verb.value]) return c.fail("payment to attack or block");
    // A following ", where X is ..." defines X; it is not another cost.
    let end = c.pos + 1;
    while (end < c.tokens.length) {
      const token = c.tokens[end], next = c.tokens[end + 1];
      if (token.kind === "punct" && token.value === "," && next?.kind === "word" && next.value === "where") break;
      end++;
    }
    const costCursor = new Cursor([
      { ...verb, value: PAYMENT_VERBS[verb.value], raw: PAYMENT_VERBS[verb.value] },
      ...c.tokens.slice(c.pos + 1, end),
    ]);
    const costs = parseCostList(costCursor);
    if (!costs || costs.some(cost => cost.cost === "choice" || cost.cost === "tap-self" || cost.cost === "untap-self")) return null;
    c.pos += costCursor.pos;
    return costs;
  });
}

function parseEachAmount(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    const counters = c.attempt((c): Amount | null => {
      const counter = c.isWord("counter", "counters") ? "any" as const : parseCounterSpec(c);
      if (!counter || c.word("counter", "counters") === null || c.word("on") === null) return null;
      const on = parseObjectRef(c);
      return on && { amount: "counter-count", counter, on };
    });
    if (counters) return counters;
    const of = parseFilter(c);
    return of && { amount: "count", of };
  });
}

export function parseDeclarationCost(c: Cursor, grammar: DeclarationCostGrammar, previousSubject?: ObjectRef): { what: ObjectRef; rule: DeclarationCost } | null {
  return c.attempt((c): { what: ObjectRef; rule: DeclarationCost } | null => {
    const what = grammar.subject(c) ?? previousSubject;
    if (!what || c.word("can't") === null) return null;
    let action: DeclarationCost["action"];
    let defenders: CombatDefender[] | undefined;
    let attackers: ObjectRef | undefined;
    if (c.words("attack", "or", "block")) action = "attack-or-block";
    else if (c.word("attack") !== null) {
      action = "attack";
      const first = grammar.defender(c);
      if (first) {
        defenders = [first];
        for (;;) {
          const next = c.attempt(c => c.word("or") !== null ? grammar.defender(c) : null);
          if (!next) break;
          defenders.push(next);
        }
      }
    } else if (c.word("block") !== null) {
      action = "block";
      attackers = c.attempt(grammar.objects) ?? undefined;
    } else return null;
    if (c.word("unless") === null) return null;

    let who: PlayerRef;
    let single = SINGLE_SUBJECTS.has(what.ref);
    if (c.word("you") !== null) {
      if (!single) return c.fail("payer for each affected creature");
      who = { player: "you" };
    } else if (c.words("its", "controller")) {
      // "Each creature ... its controller": one creature at a time.
      if (!single && what.ref !== "each") return null;
      single = true;
      who = { player: "controller", of: { ref: "it" } };
    } else if (c.words("their", "controller")) {
      if (single) return null;
      who = { player: "controller", of: { ref: "them" } };
    } else return null;
    const costs = parsePayment(c);
    if (!costs) return null;

    let scope: DeclarationCost["scope"] = "each-affected-creature";
    let multiplier: Amount | undefined;
    if (c.attempt(c => c.word("for") !== null && c.word("each") !== null ? true : null)) {
      if (c.words("of", "those", "creatures")) {
        // The affected creatures in this declaration, one payment for each.
        if (single) return null;
      } else {
        const amount = parseEachAmount(c);
        if (!amount) return null;
        if (!single) {
          // A group's printed total must count the declared creatures;
          // any other quantity would leave the per-creature reading open.
          if (amount.amount !== "count" || !countsDeclaredCreatures(amount.of)) return c.fail("count of the creatures being declared");
          scope = "declaration";
        }
        multiplier = amount;
      }
    } else if (!single) return c.fail("'for each' basis of a cost shared by several creatures");
    const binding = c.attempt(c => c.punct(",") && c.words("where", "x", "is") ? parseAmount(c) : null);
    return { what, rule: { rule: "declaration-cost", action, who, costs, scope,
      ...(multiplier ? { multiplier } : {}), ...(defenders ? { defenders } : {}), ...(attackers ? { attackers } : {}),
      ...(binding ? { bindings: [{ variable: "x" as const, value: binding }] } : {}) } };
  });
}
