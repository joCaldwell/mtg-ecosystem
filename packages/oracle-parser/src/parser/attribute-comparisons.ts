// Comparisons between the same characteristic of two identified objects:
// "if that creature has greater power or toughness than this creature",
// "if that creature's power is greater than this creature's". Each side is
// one object's current value, compared when the condition is checked
// (CR 208, 603.4). "Power or toughness" is either comparison holding, the
// reading CR 702.100a spells out for evolve ("power is greater than this
// creature's power and/or ... toughness is greater than this creature's
// toughness").
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Condition, ObjectRef } from "../ast.ts";
import type { ConditionGrammar } from "./conditions.ts";
import type { Cursor } from "./cursor.ts";

type Attribute = "power" | "toughness";

/** A single, already identified object: never a quantified or fresh one. */
function identified(ref: ObjectRef | null): ref is ObjectRef {
  if (!ref) return false;
  if (["self", "this", "it", "enchanted", "equipped"].includes(ref.ref)) return true;
  return ref.ref === "that" && !ref.plural;
}

function compare(attribute: Attribute, what: ObjectRef, op: "gt" | "lt", than: ObjectRef): Condition {
  const value = (of: ObjectRef): Amount => ({ amount: "attribute", of, attribute });
  return { condition: "compare", value: value(what), comparison: { op, value: value(than) } };
}

/** "<object> has greater|less power [or toughness] than <object>" */
function parseHasGreater(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const what = g.parseObjectRef(c);
    if (!identified(what) || c.word("has") === null) return null;
    const degree = c.word("greater", "less");
    if (!degree) return null;
    const first = c.word("power", "toughness") as Attribute | null;
    if (!first) return null;
    let second: Attribute | null = null;
    if (c.word("or") !== null) {
      second = c.word("power", "toughness") as Attribute | null;
      if (!second || second === first) return null;
    }
    if (c.word("than") === null) return null;
    const than = g.parseObjectRef(c);
    if (!identified(than)) return null;
    const op = degree === "greater" ? "gt" : "lt";
    if (!second) return compare(first, what, op, than);
    return { condition: "any-of", conditions: [compare(first, what, op, than), compare(second, what, op, than)] };
  });
}

/** "<object>'s power is greater than <object>'s": the compared object's
 * value is the same characteristic, left unrepeated. */
function parseElidedPossessive(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    if (!g.parsePossessiveObject) return null;
    const value = g.parseAmount(c);
    if (value?.amount !== "attribute" || (value.attribute !== "power" && value.attribute !== "toughness")) return null;
    if (!identified(value.of) || c.word("is") === null) return null;
    const degree = c.words("greater", "than") ? "gt" : c.words("less", "than") ? "lt" : null;
    if (!degree) return null;
    const than = g.parsePossessiveObject(c);
    if (!identified(than)) return null;
    // A following characteristic word is the ordinary, unelided comparison.
    if (c.peek()?.kind === "word") return null;
    return compare(value.attribute, value.of, degree, than);
  });
}

/** "the amount of mana spent to cast it was less than its mana value": the
 * mana actually paid is a fixed fact of the casting (CR 601.2h), so the past
 * tense states the same comparison as "is". Other past-tense values (an
 * object's last known power) are a different question and are not read here. */
function parsePastManaSpent(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const value = g.parseAmount(c);
    if (value?.amount !== "mana-spent" || c.word("was") === null) return null;
    const comparison = g.parseComparison(c);
    return comparison && { condition: "compare", value, comparison };
  });
}

export function parseAttributeComparison(c: Cursor, g: ConditionGrammar): Condition | null {
  return parseHasGreater(c, g) ?? parseElidedPossessive(c, g) ?? parsePastManaSpent(c, g);
}
