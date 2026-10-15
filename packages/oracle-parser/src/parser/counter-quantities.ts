// Counters as the unit of a "for each" rider. CR 122.1: counters are not
// objects, so "for each charge counter on this artifact" can never be an
// object filter; it is the counter count of that object or player, and "for
// each charge counter removed this way" is what the stated removal actually
// removed (608.2h/i). Either one multiplies a quantity inside one instruction.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, CounterSpec, Effect, ObjectFilter, ObjectRef, PlayerRef } from "../ast.ts";
import { hasCombatHistory, parseAggregateCount } from "./aggregates.ts";
import { wordNumber } from "../vocab.ts";
import type { Cursor } from "./cursor.ts";
import { isPluralObjectRef } from "./quantified-selections.ts";

export interface CounterUnitGrammar {
  parseCounterSpec(c: Cursor): CounterSpec | null;
  parseObjectRef(c: Cursor): ObjectRef | null;
  parsePlayerRef(c: Cursor): PlayerRef | null;
  isPluralPlayerRef(ref: PlayerRef): boolean;
}

/** Words that make the phrase something other than one counter kind: a kind
 * iterator ("each kind of counter"), a group size ("each five counters") or
 * a contrast ("each other counter"). */
const NOT_A_KIND = new Set(["kind", "kinds", "type", "types", "of", "each", "other", "additional", "different"]);

/** Unit after "for each": "[kind] counter on <object or player>", "[kind]
 * counter <player> has", or "[kind] counter removed this way". The singular
 * "counter" is required; anything left over fails the enclosing sentence. */
export function parseCounterUnit(c: Cursor, g: CounterUnitGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    const counter = c.isWord("counter") ? "any" : g.parseCounterSpec(c);
    if (!counter || c.word("counter") === null) return null;
    if (counter !== "any" && counter.type.split(" ").some(word => NOT_A_KIND.has(word) || wordNumber(word) !== undefined)) return null;
    if (c.words("removed", "this", "way")) return { amount: "event-quantity", event: "counter-removal", counter, scope: "this-way" };
    // One holder only. Counters on several objects or players ("on each
    // opponent", "on other creatures you control") would need an explicit
    // summed or per-holder reading, so they stay rejected.
    if (c.word("on") !== null) {
      const player = g.parsePlayerRef(c);
      if (player) return g.isPluralPlayerRef(player) ? null : { amount: "counter-count", counter, on: player };
      const on = g.parseObjectRef(c);
      return on && !isPluralObjectRef(on) ? { amount: "counter-count", counter, on } : null;
    }
    // CR 122.1: a player's own counters. "Your opponents have" is collective
    // possession, an explicit total (unlike distributive "each opponent has").
    if (c.words("your", "opponents", "have")) return { amount: "counter-count", counter, on: { player: "each-opponent" }, total: true };
    const on = g.parsePlayerRef(c);
    return on && !g.isPluralPlayerRef(on) && c.word("have", "has") !== null ? { amount: "counter-count", counter, on } : null;
  });
}

export interface EachUnitGrammar extends CounterUnitGrammar {
  parseFilter(c: Cursor): ObjectFilter | null;
}

/** Any unit after "for each" as one number: counters, distinct values
 * ("basic land type among lands you control"), or the matching objects. */
export function parseEachUnit(c: Cursor, g: EachUnitGrammar): Amount | null {
  return parseCounterUnit(c, g) ?? parseAggregateCount(c, g, { singular: true }) ?? c.attempt((c): Amount | null => {
    const of = g.parseFilter(c);
    return of && !hasCombatHistory(of) ? { amount: "count", of } : null;
  });
}

/** Instructions whose quantity is not a plain amount field. "Add {G} for
 * each ..." is that many {G}, the same production as "an amount of {G} equal
 * to ..."; a symbolic production multiplies its own amount; "discards a card
 * for each ..." discards that many cards in one discard. Anything else has no
 * scalar to multiply and is rejected by returning false. */
export function scaleByUnit(e: Effect, unit: Amount): boolean {
  const product = (amount: Amount): Amount => ({ amount: "product", factors: [amount, unit] });
  if (e.effect === "add-mana") {
    const mana = e.mana;
    if (mana.mana === "fixed" && mana.symbols.length === 1 && /^[WUBRGC]$/.test(mana.symbols[0])) {
      e.mana = { mana: "scaled-symbol", symbol: mana.symbols[0], amount: unit };
      return true;
    }
    if (mana.mana === "any-color" || mana.mana === "any-type" || mana.mana === "chosen-color") {
      e.mana = { ...mana, amount: product(mana.amount) };
      return true;
    }
    return false;
  }
  if (e.effect === "discard" && e.what !== "hand" && e.what.ref === "filter" && !e.what.upTo && !e.random && !e.what.groupConstraints) {
    e.what = { ...e.what, count: product(e.what.count) };
    return true;
  }
  if (e.effect === "sacrifice" && e.what.ref === "filter" && !e.what.upTo && !e.what.groupConstraints) {
    e.what = { ...e.what, count: product(e.what.count) };
    return true;
  }
  return false;
}
