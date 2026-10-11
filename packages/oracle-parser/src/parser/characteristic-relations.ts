import type { CharacteristicOperand, CharacteristicRelation, Condition, GroupConstraint, ObjectFilter, ObjectRef, SharedCharacteristic } from "../ast.ts";
import { Cursor } from "./cursor.ts";

interface Grammar { parseObjectRef(c: Cursor): ObjectRef | null }
type Scalar = "power" | "toughness" | "mana-value";
type Parsed = { test: CharacteristicRelation } | { property: Scalar; of: ObjectRef; negated: boolean };

function characteristic(c: Cursor): SharedCharacteristic | null {
  if (c.words("creature", "type")) return "creature-type";
  if (c.words("card", "type")) return "card-type";
  if (c.words("land", "type")) return "land-type";
  return c.word("color", "name") as "color" | "name" | null;
}

/** A queried set is not a new choice. Close definite nouns before parsing
 * outer qualifiers: in Harness Storm, "from your graveyard" qualifies the
 * eligible card, not "that spell". Token slices retain target provenance. */
function operand(c: Cursor, g: Grammar): CharacteristicOperand | null {
  return c.attempt(c => {
    const start = c.pos;
    let length: number | undefined;
    if (c.peek()?.kind === "selfref" || c.isWord("it", "them")) length = 1;
    else if (c.isWord("this", "that", "those", "enchanted", "equipped")) length = 2;
    else if (c.isWord("the")) {
      const next = c.peek(1), noun = c.peek(2);
      if (next?.kind === "word" && ["exiled", "revealed", "discarded", "sacrificed", "destroyed"].includes(next.value)
        && noun?.kind === "word" && ["card", "cards"].includes(noun.value)) length = 3;
    }
    let what: ObjectRef | null;
    if (length !== undefined) {
      const limited = new Cursor(c.tokens.slice(start, start + length));
      what = g.parseObjectRef(limited);
      if (!what || !limited.done()) return null;
      c.pos += length;
      // A fresh eligibility qualifier could describe either this operand or
      // the outer candidate. Spatial/pool tails remain outer syntax, but
      // ambiguous characteristic tails must not be attached by proximity.
      const tail = c.peek(1);
      if (c.isWord("with") || (c.isWord("and") && tail?.kind === "word" && tail.value === "with")) return null;
    } else {
      what = g.parseObjectRef(c);
      if (!what) return null;
      // Post-operand coordinated qualifiers have ambiguous attachment.
      for (let end = start + 1; end < c.pos; end++) {
        const token = c.tokens[end];
        if (token.kind !== "word" || token.possessive || token.value !== "and") continue;
        const before = new Cursor(c.tokens.slice(start, end));
        if (g.parseObjectRef(before) && before.done()) return null;
      }
    }
    if (what.ref === "them" || what.ref === "rest" || what.ref === "card-extraction-group"
      || (what.ref === "that" && what.plural) || ((what.ref === "related" || what.ref === "choice-result" || what.ref === "copy-result") && what.number === "plural")) return null;
    if (("upTo" in what && what.upTo) || ("groupConstraints" in what && what.groupConstraints)
      || ("count" in what && what.count && (what.count.amount !== "fixed" || what.count.value !== 1))) return null;
    if (what.ref === "filter" || what.ref === "each" || what.ref === "all") {
      const first = c.tokens[start];
      const every = first?.kind === "word" && ["each", "all"].includes(first.value);
      return { operand: "matching-set", filter: what.filter, quantifier: every ? "every" : "any" };
    }
    return { operand: "object", what };
  });
}

/** CR201.2a/c deliberately distinguish common names, different names, and
 * negated common names. Nameless objects make those predicates inequivalent.
 * Sharing a color/type asks for common characteristic values, not identity
 * of entire sets (CR105.2,205.2/3). A named matching set has its own quantifier.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseTest(c: Cursor, g: Grammar, context: "filter" | "condition"): Parsed | null {
  return c.attempt(c => {
    let nominal = false;
    if (context === "filter") {
      nominal = c.word("with") !== null;
      if (!nominal) c.word("that");
    }
    let negative = false;
    if (!nominal) {
      negative = c.word("doesn't", "don't") !== null || c.words("does", "not") || c.words("do", "not");
      if (c.word("share", "shares", "sharing") !== null) {
        if (c.word("a", "an") === null) return null;
        const property = characteristic(c);
        if (!property || c.word("with") === null) return null;
        const withOperand = operand(c, g);
        if (!withOperand) return null;
        const test: CharacteristicRelation = { relation: "shared-value", characteristic: property, with: withOperand };
        return { test: negative ? { relation: "not", of: test } : test };
      }
      if (c.word("has", "have") === null) return null;
      nominal = true;
    }
    c.word("the", "a");
    if (c.word("different") !== null) {
      if (negative || c.word("name") === null || c.word("than", "from") === null) return null;
      const from = operand(c, g);
      return from && { test: { relation: "different-name", from } };
    }
    if (c.word("same") === null) return null;
    if (c.word("name") !== null) {
      if (c.word("as") === null) return null;
      const withOperand = operand(c, g);
      if (!withOperand) return null;
      const test: CharacteristicRelation = { relation: "shared-value", characteristic: "name", with: withOperand };
      return { test: negative ? { relation: "not", of: test } : test };
    }
    const property = c.words("mana", "value") ? "mana-value" : c.word("power", "toughness") as Scalar | null;
    if (!property || c.word("as") === null) return null;
    const of = operand(c, g);
    return of?.operand === "object" ? { property, of: of.what, negated: negative } : null;
  });
}

export function parseCharacteristicRelationQualifier(c: Cursor, g: Grammar): ObjectFilter | null {
  return c.attempt(c => {
    const parsed = parseTest(c, g, "filter");
    if (!parsed) return null;
    if ("test" in parsed) return { characteristicRelations: [parsed.test] };
    if (parsed.negated) return null;
    return { properties: [{ property: parsed.property, comparison: { op: "eq", value: { amount: "attribute", attribute: parsed.property, of: parsed.of } } }] };
  });
}

export function parseCharacteristicRelationCondition(c: Cursor, g: Grammar): Condition | null {
  return c.attempt(c => {
    const what = g.parseObjectRef(c);
    if (!what || ["all", "each", "filter"].includes(what.ref)) return null;
    const parsed = parseTest(c, g, "condition");
    if (!parsed) return null;
    if ("test" in parsed) return { condition: "characteristic-relation", what, test: parsed.test };
    const comparison: Condition = { condition: "compare", value: { amount: "attribute", attribute: parsed.property, of: what },
      comparison: { op: "eq", value: { amount: "attribute", attribute: parsed.property, of: parsed.of } } };
    return parsed.negated ? { condition: "not", of: comparison } : comparison;
  });
}

/** This constrains the chosen group, not each candidate. The enclosing
 * parser opts into actual target/payment/resolution selections explicitly.
 * Binding and evaluation must retain the complete group's characteristic
 * sets; pairwise intersections do not prove one common value for 3+ objects. */
export function parseSharedGroupConstraint(c: Cursor): GroupConstraint | null {
  return c.attempt(c => {
    if (!c.words("that", "share", "a")) return null;
    const property = characteristic(c);
    if (!property || c.isWord("with")) return null;
    return { constraint: "shared-value", characteristic: property };
  });
}
