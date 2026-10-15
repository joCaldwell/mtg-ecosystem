// Filters on a creature's base power / toughness: its values after
// characteristic-defining abilities and setting effects only, ignoring
// modifying effects and counters (CR 208.4b, 613.4a-b).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Comparison, ObjectFilter, PropertyConstraint } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

type BaseStat = Extract<PropertyConstraint["property"], "base-power" | "base-toughness">;

/** "with base power and toughness 1/1", "with base power or toughness 1
 * [or less]", "with base power 1". */
export function parseBaseStatQualifier(c: Cursor, parseComparison: (c: Cursor) => Comparison | null): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (!c.words("with", "base")) return null;
    const first = c.word("power", "toughness");
    if (!first) return null;
    const stat = (word: string): BaseStat => word === "power" ? "base-power" : "base-toughness";
    if (first === "power" && c.words("and", "toughness")) {
      const power = c.number();
      if (power === null || !c.punct("/")) return null;
      const toughness = c.number();
      if (toughness === null) return null;
      return { properties: [
        { property: "base-power", comparison: { op: "eq", value: { amount: "fixed", value: power } } },
        { property: "base-toughness", comparison: { op: "eq", value: { amount: "fixed", value: toughness } } }] };
    }
    // Either value satisfies the one stated comparison.
    if (first === "power" && c.words("or", "toughness")) {
      const comparison = parseComparison(c);
      return comparison && { anyOf: [{ properties: [{ property: "base-power", comparison }] },
        { properties: [{ property: "base-toughness", comparison: structuredClone(comparison) }] }] };
    }
    const comparison = parseComparison(c);
    return comparison && { properties: [{ property: stat(first), comparison }] };
  });
}
