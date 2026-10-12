// Quantifier shapes of object selections: "each of two target creatures",
// "two targets", "any other target", "the other", a floating distributive
// "each" after a plural subject. CR 115.1/3/4, 601.2c, 608.2c.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, ObjectFilter, ObjectRef } from "../ast.ts";
import { wordNumber } from "../vocab.ts";
import type { Cursor } from "./cursor.ts";
import { validSelectionCount } from "./selection-constraints.ts";
import { targetSelectionProvenance } from "./refs.ts";

/** Does this reference denote a group that a distributive "each" can range
 * over? A count that is not exactly one (including "up to"/X/any number)
 * is grammatically plural even when it resolves to a single object. */
export function isPluralObjectRef(ref: ObjectRef): boolean {
  switch (ref.ref) {
    case "them": case "each": case "all": return true;
    case "that": return !!ref.plural;
    case "related": case "choice-result": case "copy-result": return ref.number === "plural";
    case "target": case "filter": case "from-pool":
      return !!ref.upTo || ref.count.amount !== "fixed" || ref.count.value !== 1;
    case "any-target": return !!ref.count && (!!ref.upTo || ref.count.amount !== "fixed" || ref.count.value !== 1);
    default: return false;
  }
}

/** Descriptor keys of a definite "those <nouns>" reference. Anything else
 * after "each of those creatures" (e.g. "with flying") would restrict the
 * members rather than describe the antecedent, which needs its own node. */
const DESCRIPTOR_KEYS = new Set<keyof ObjectFilter>(["types", "subtypes", "supertypes", "cls", "permanent", "nonToken", "colors"]);

/** "each of <group>": the instruction applies to every member.
 *  - A counted selection ("each of up to two target creatures", "each of two
 *    creatures you control") stays that selection: one "target" word is one
 *    declaration of that many distinct targets (CR 115.3, 601.2c), and an
 *    instruction on a selected group already applies to each of its members,
 *    as in "Put a +1/+1 counter on each of up to two target creatures".
 *  - An established group ("each of those creatures", "each of them") is
 *    quantified individually: every object from among that group. Unlike
 *    the bare plural, a singular pronoun in the same instruction ("deals
 *    damage equal to its power") ranges over the member, as it does for
 *    "each creature". The group itself is never reselected (CR 608.2h).
 * Singular operands and fresh universal filters are other productions. */
export function parseEachOf(c: Cursor, parseSelection: (c: Cursor) => ObjectRef | null): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (!c.words("each", "of")) return null;
    const group = parseSelection(c);
    if (!group || !isPluralObjectRef(group)) return null;
    switch (group.ref) {
      case "target": case "any-target": case "filter": return group;
      case "that":
        if ((Object.keys(group.filter) as (keyof ObjectFilter)[]).some(key => !DESCRIPTOR_KEYS.has(key)))
          return c.fail("represented restriction of an established group");
        return { ref: "each", filter: { fromAmong: group } };
      case "them": case "choice-result": case "related": return { ref: "each", filter: { fromAmong: group } };
      default: return null;
    }
  });
}

/** CR 115.4: damage instructions can require "another target", "two
 * targets" or similar instead of "target [something]"; eligibility is then
 * the rule's creature/player/planeswalker/battle set, exactly as for "any
 * target". "other"/"another" stays the unresolved lexical contrast used by
 * ObjectFilter.other. One "target(s)" word is one declaration (CR 115.3).
 * A following noun means this was "target <filter>" or a targeted player. */
export function parseBareTargets(c: Cursor, startsNoun: (c: Cursor) => boolean): Extract<ObjectRef, { ref: "any-target" }> | null {
  return c.attempt((c): Extract<ObjectRef, { ref: "any-target" }> | null => {
    if (c.words("any", "other")) {
      const token = c.peek();
      if (c.word("target") === null || startsNoun(c)) return null;
      return { ref: "any-target", other: true, ...targetSelectionProvenance(token) };
    }
    if (c.word("another") !== null) {
      const token = c.peek();
      if (c.word("target") === null || startsNoun(c)) return null;
      return { ref: "any-target", other: true, ...targetSelectionProvenance(token) };
    }
    const upTo = c.words("up", "to");
    let count: Amount | null = null;
    const number = c.number();
    if (number !== null) count = { amount: "fixed", value: number };
    else if (c.word("x") !== null) count = { amount: "x" };
    else {
      const word = c.peek();
      const value = word?.kind === "word" && !word.possessive ? wordNumber(word.value) : undefined;
      if (value === undefined) return null;
      c.pos++;
      count = { amount: "fixed", value };
    }
    if (!validSelectionCount(count)) return null;
    const other = c.word("other") !== null;
    const token = c.peek();
    const singular = count.amount === "fixed" && count.value === 1;
    if (c.word(singular ? "target" : "targets") === null || startsNoun(c)) return null;
    return { ref: "any-target", count, ...(upTo ? { upTo: true } : {}), ...(other ? { other: true as const } : {}), ...targetSelectionProvenance(token) };
  });
}

/** "the other" after choosing all but one member of an established group is
 * the complement of that choice, i.e. the same reference as "the rest" with
 * one member remaining. "the other creature" (the opposing participant of an
 * event) and reciprocal "to the other" are different references and are not
 * read here; callers offer this only in explicit action slots. */
export function parseTheOther(c: Cursor): Extract<ObjectRef, { ref: "rest" }> | null {
  return c.attempt(c => {
    if (!c.words("the", "other")) return null;
    const next = c.peek();
    if (next?.kind === "word" && !["into", "onto", "on", "in", "to", "from"].includes(next.value)) return null;
    if (next && next.kind !== "word" && next.kind !== "punct") return null;
    return { ref: "rest" };
  });
}
