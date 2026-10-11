import type { Amount, Comparison, Condition, GroupConstraint, ObjectFilter, PlayerRef } from "../ast.ts";
import { parseSharedGroupConstraint } from "./characteristic-relations.ts";
import type { Cursor } from "./cursor.ts";

/** Restrictions on one selected group, not on each eligible object. CR
 * 601.2c/608.2d preserve the selection timing of the enclosing reference.
 * Signed power contributes to a total without clamping (107.1b). Different
 * names means every object has a name and no pair shares any name (201.2b),
 * not merely a count of distinct strings (split cards can have two names).
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export function parseGroupConstraint(c: Cursor, parseComparison: (c: Cursor) => Comparison | null): GroupConstraint | null {
  return c.attempt((c): GroupConstraint | null => {
    const shared = parseSharedGroupConstraint(c);
    if (shared) return shared;
    if (c.word("with") === null) return null;
    if (c.word("total") !== null) {
      const attribute = c.words("mana", "value") ? "mana-value" : c.word("power", "toughness") as "power" | "toughness" | null;
      if (!attribute) return null;
      const comparison = parseComparison(c);
      return comparison && { constraint: "attribute-sum", attribute, comparison };
    }
    if (c.word("different") !== null) {
      const property = c.words("mana", "values") ? "mana-value"
        : c.word("names") !== null ? "name" : c.word("powers") !== null ? "power" : null;
      return property && { constraint: "pairwise-different", property };
    }
    return null;
  });
}

/** A fixed negative/noninteger number cannot be a selection count. Dynamic
 * amounts remain symbolic and must be checked when their choice is made. */
export function validSelectionCount(count: Amount): boolean {
  return count.amount !== "fixed" || Number.isSafeInteger(count.value) && count.value >= 0;
}

interface SubsetGrammar {
  parsePlayerRef(c: Cursor): PlayerRef | null;
  parseComparison(c: Cursor): Comparison | null;
  parseFilter(c: Cursor): ObjectFilter | null;
}

/** Coven is existence of a suitable subset; duplicate-powered creatures
 * outside that subset do not invalidate it, and an intervening-if trigger
 * can use a different subset when it resolves (CR603.4). It neither chooses
 * creatures nor captures them. Names likewise require a pairwise-compatible
 * subset under CR201.2b, not the union of all printed names.
 * https://magic.wizards.com/en/news/feature/innistrad-midnight-hunt-mechanics
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export function parseControlledSubset(c: Cursor, g: SubsetGrammar): Condition | null {
  return c.attempt(c => {
    const who = g.parsePlayerRef(c);
    if (!who || c.word("control", "controls") === null) return null;
    const comparison = g.parseComparison(c);
    // Upper bounds/negation quantify differently, and bare exact counts are
    // intentionally left to the ordinary control grammar.
    if (comparison?.op !== "ge" || !validSelectionCount(comparison.value)) return null;
    const of = g.parseFilter(c);
    if (!of) return null;
    const constraint = parseGroupConstraint(c, g.parseComparison);
    if (constraint?.constraint !== "pairwise-different") return null;
    return { condition: "controls-subset", who, of, minimum: comparison.value, groupConstraints: [constraint] };
  });
}
