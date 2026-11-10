// "with greater power", "with lesser mana value [than it]": a comparison of
// one attribute between the candidate and a standard (CR 208.1, 202.3).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

export interface ImplicitComparisonGrammar {
  parseObjectRef(c: Cursor): ObjectRef | null;
}

/** With "than X" the standard is that object's same attribute. Without it
 * the standard is left unstated and stays symbolic (implicitComparison). */
export function parseComparativeAttribute(c: Cursor, g: ImplicitComparisonGrammar): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (c.word("with") === null) return null;
    // "with equal or lesser mana value": inclusive of the standard's value.
    const inclusive = c.words("equal", "or");
    const degree = c.word("greater", "lesser");
    if (!degree) return null;
    const attribute = c.words("mana", "value") ? "mana-value" : c.word("power", "toughness") as "power" | "toughness" | null;
    if (!attribute) return null;
    const op = degree === "greater" ? (inclusive ? "ge" : "gt") : (inclusive ? "le" : "lt");
    if (c.word("than") === null) {
      // "with greater power or toughness", "… and toughness": not one attribute.
      if (c.isWord("or", "and")) return null;
      return { implicitComparison: { attribute, op } };
    }
    const of = g.parseObjectRef(c);
    const singular = of && (of.ref === "self" || of.ref === "it" || of.ref === "this" || of.ref === "enchanted"
      || of.ref === "equipped" || (of.ref === "that" && !of.plural));
    return singular ? { properties: [{ property: attribute, comparison: { op, value: { amount: "attribute", of, attribute } } }] } : null;
  });
}

/** "with the same controller": the standard's controller, left unstated. */
export function parseSameController(c: Cursor): ObjectFilter | null {
  return c.words("with", "the", "same", "controller") ? { implicitSameController: true } : null;
}
