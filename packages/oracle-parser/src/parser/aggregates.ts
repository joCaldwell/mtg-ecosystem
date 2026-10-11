// Distinct game values and summed attributes are different from object counts.
// CR 105.2c, 205.2b, 305.6, 601.2f/h, 608.2h and 609.4b.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, ObjectFilter, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCombatActionHistoryPredicate } from "./history-filters.ts";

/** History-qualified current objects and historical event participants are
 * different sets. Unknown mixes of current qualifiers and past events must
 * not fall through to an ordinary object-count interpretation. */
export function hasCombatHistory(filter: ObjectFilter): boolean {
  return !!filter.combatHistory || !!filter.anyOf?.some(hasCombatHistory) || !!filter.allOf?.some(hasCombatHistory);
}

function parseCombatHistoryCount(c: Cursor, singular: boolean): Amount | null {
  return c.attempt(c => {
    if (c.word(singular ? "creature" : "creatures") === null) return null;
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
    c.word("different");
    const plural = !options.singular;
    const characteristic = c.words("basic", "land", plural ? "types" : "type") ? "basic-land-type"
      : c.words("card", plural ? "types" : "type") ? "card-type"
      : c.word(plural ? "colors" : "color") !== null ? "color" : null;
    if (!characteristic) return null;
    if (characteristic === "color" && c.words("of", "mana", "spent", "to", "cast")) {
      const what = g.parseObjectRef(c);
      return what ? { amount: "mana-spent", what, quantity: "distinct-colors" } : null;
    }
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
    c.word("the");
    if (c.word("total") === null) return null;
    const attribute = c.words("mana", "value") ? "mana-value" : c.word("power", "toughness");
    if (attribute !== "power" && attribute !== "toughness" && attribute !== "mana-value") return null;
    if (c.word("of") === null) return null;
    const among = g.parseObjectRef(c);
    return among ? { amount: "attribute-sum", attribute, among } : null;
  });
}
