// Distinct game values and summed attributes are different from object counts.
// CR 105.2c, 205.2b, 305.6, 601.2f/h, 608.2h and 609.4b.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, ObjectFilter, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCombatActionHistoryPredicate } from "./history-filters.ts";
import { parsePossessiveObject } from "./refs.ts";
import { parseManaSpentUnit } from "./mana-spent-units.ts";

/** History-qualified current objects and historical event participants are
 * different sets. Unknown mixes of current qualifiers and past events must
 * not fall through to an ordinary object-count interpretation. */
export function hasCombatHistory(filter: ObjectFilter): boolean {
  return !!filter.combatHistory || !!filter.anyOf?.some(hasCombatHistory) || !!filter.allOf?.some(hasCombatHistory);
}

function parseCombatHistoryCount(c: Cursor, singular: boolean): Amount | null {
  return c.attempt(c => {
    if (c.word(singular ? "creature" : "creatures") === null) return null;
    // "creatures you attacked with this turn": declared as attackers by that player.
    const by = c.attempt(c => {
      if (!c.words("you", "attacked", "with")) return null;
      return c.words("this", "turn") ? { action: "attacked" as const, during: "this-turn" as const } : null;
    });
    if (by) return { amount: "combat-history-count", ...by, by: { player: "you" }, distinct: "object-incarnations" };
    const history = parseCombatActionHistoryPredicate(c);
    return history ? { amount: "combat-history-count", ...history, distinct: "object-incarnations" } : null;
  });
}

export interface AggregateGrammar {
  parseObjectRef(c: Cursor): ObjectRef | null;
}

/** Descriptor after "the number of" or "for each". A singular descriptor
 * still counts distinct values, not one arbitrarily selected representative.
 * References to a previously selected pool remain ObjectRefs. */
export function parseAggregateCount(c: Cursor, g: AggregateGrammar, options: { singular?: boolean } = {}): Amount | null {
  return c.attempt(c => {
    // Rowdy Research / Witchstalker Frenzy count past attacking creatures,
    // including those no longer on the battlefield (400.7,508.4). There is
    // no current controller/zone qualifier in this narrow production.
    const history = parseCombatHistoryCount(c, !!options.singular);
    if (history) return history;
    const plural = !options.singular;
    if (!plural) {
      const spent = parseManaSpentUnit(c);
      if (spent) return spent;
    }
    // "differently named lands you control": the counted set is the stated
    // objects themselves (CR 201.2b), not a characteristic "among" them.
    if (c.words("differently", "named")) {
      const among = g.parseObjectRef(c);
      return among && (among.ref === "each" || among.ref === "all") ? { amount: "distinct-count", characteristic: "name", among } : null;
    }
    // "for each of its colors", "for each of that spell's colors": the
    // colors that one object is (CR 105.1–105.2; a colorless object has none).
    const own = !plural && c.attempt(c => {
      if (c.word("of") === null) return null;
      const of = parsePossessiveObject(c);
      return of && c.word("colors") !== null ? of : null;
    });
    if (own) return { amount: "distinct-count", characteristic: "color", among: own };
    c.word("different");
    const characteristic = c.words("basic", "land", plural ? "types" : "type") ? "basic-land-type"
      : c.words(...(plural ? ["kinds", "of", "counters"] : ["kind", "of", "counter"])) ? "counter-kind"
      : c.words("card", plural ? "types" : "type") ? "card-type"
      // CR 110.4: the six permanent types; CR 205.3, 702.73a: a creature type
      // is a subtype, and a changeling is every creature type.
      : c.words("permanent", plural ? "types" : "type") ? "permanent-type"
      : c.words("creature", plural ? "types" : "type") ? "creature-type"
      : c.words("mana", plural ? "values" : "value") ? "mana-value"
      : c.word(plural ? "powers" : "power") !== null ? "power"
      : c.word(plural ? "colors" : "color") !== null ? "color" : null;
    if (!characteristic) return null;
    if (characteristic === "color" && c.words("of", "mana", "spent", "to", "cast")) {
      const what = g.parseObjectRef(c);
      return what ? { amount: "mana-spent", what, quantity: "distinct-colors" } : null;
    }
    // "the number of colors that spell is": the same value for one object.
    const is = characteristic === "color" && plural && c.attempt(c => {
      const of = g.parseObjectRef(c);
      return of && (of.ref === "it" || (of.ref === "that" && !of.plural)) && c.word("is") !== null ? of : null;
    });
    if (is) return { amount: "distinct-count", characteristic, among: is };
    if (c.word("among") === null) return null;
    const among = g.parseObjectRef(c);
    return among ? { amount: "distinct-count", characteristic, among } : null;
  });
}

/** Complete amount grammar, called before generic "the number of objects".
 * Actual mana paid includes colorless units, while its distinct-color count
 * does not count colorless or snow as extra colors (CR 106.1b, 107.4h). */
export function parseAggregateAmount(c: Cursor, g: AggregateGrammar): Amount | null {
  return c.attempt(c => {
    if (c.words("the", "number", "of")) return parseAggregateCount(c, g);
    if (c.words("the", "amount", "of", "mana", "spent", "to", "cast")) {
      const what = g.parseObjectRef(c);
      return what ? { amount: "mana-spent", what, quantity: "total" } : null;
    }
    if (c.words("the", "amount", "of") && c.attempt(c => {
      const symbol = c.peek();
      if (symbol?.kind !== "symbol" || symbol.value !== "S") return null;
      c.pos++;
      return c.words("spent", "to", "cast") ? true : null;
    })) {
      const what = g.parseObjectRef(c);
      return what ? { amount: "mana-spent", what, quantity: "snow" } : null;
    }
    // "their total power": the sum for the objects "they" stand for. Players
    // have no power, toughness or mana value, so the pronoun is the objects'.
    const their = c.word("their") !== null;
    if (!their) c.word("the");
    if (c.word("total") === null) return null;
    const attribute = c.words("mana", "value") ? "mana-value" : c.word("power", "toughness");
    if (attribute !== "power" && attribute !== "toughness" && attribute !== "mana-value") return null;
    if (their) return { amount: "attribute-sum", attribute, among: { ref: "them" } };
    if (c.word("of") === null) return null;
    const among = g.parseObjectRef(c);
    return among ? { amount: "attribute-sum", attribute, among } : null;
  });
}
