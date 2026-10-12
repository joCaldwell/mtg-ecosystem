// Comparative quantities: "controls more lands than you", "has at least 7
// life more than your starting life total". The standard of comparison is
// the same measurement taken for the reference player, kept as an explicit
// Amount, so each side is evaluated once when the comparison is applied
// (CR 107.1–107.2, 608.2h). Nothing is chosen and no object is selected.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Comparison, Condition, ObjectFilter, PlayerFilter, PlayerRef, ZoneRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

export interface ComparativeGrammar {
  parseAmount(c: Cursor): Amount | null;
  parseCount(c: Cursor): Amount | null;
  parseFilter(c: Cursor): ObjectFilter | null;
  parsePlayerRef(c: Cursor): PlayerRef | null;
}

type Predicate =
  | { measure: "controls"; filter: ObjectFilter; comparison: Comparison }
  | { measure: "life" | "hand"; comparison: Comparison };

interface Degree { direction: "more" | "fewer"; margin?: Amount }

/** "more" / "fewer" are strict; "at least N more" compares against the
 * reference quantity plus N (minus N for "at least N fewer"). */
function parseDegree(c: Cursor, g: ComparativeGrammar): Degree | null {
  return c.attempt((c): Degree | null => {
    if (c.words("at", "least")) {
      const margin = g.parseCount(c);
      const direction = margin && c.word("more", "fewer");
      return margin && (direction === "more" || direction === "fewer") ? { direction, margin } : null;
    }
    const direction = c.word("more", "fewer", "less");
    return direction ? { direction: direction === "more" ? "more" : "fewer" } : null;
  });
}

function comparison(degree: Degree, reference: Amount): Comparison {
  if (!degree.margin) return { op: degree.direction === "more" ? "gt" : "lt", value: reference };
  return degree.direction === "more"
    ? { op: "ge", value: { amount: "plus", a: reference, b: degree.margin } }
    : { op: "le", value: { amount: "minus", a: reference, b: degree.margin } };
}

/** One identified player. A quantified reference ("an opponent", "each other
 * player") would need existential or universal comparison semantics. */
function parseReferencePlayer(c: Cursor, g: ComparativeGrammar): PlayerRef | null {
  return c.attempt(c => {
    if (c.word("than") === null) return null;
    const who = g.parsePlayerRef(c);
    if (!who || !["you", "that-player", "they", "the-player", "defending-player", "controller", "owner",
      "target-player", "target-opponent", "chosen-player", "enchanted-player"].includes(who.player)) return null;
    // Pro-verb repeating the compared predicate: "than you do".
    c.word("do", "does");
    return who;
  });
}

/** Same spelling the filter grammar gives "lands you control" and
 * "lands that player controls", so equal measurements stay equal nodes. */
function controlledBy(filter: ObjectFilter, who: PlayerRef): ObjectFilter {
  if (who.player === "you") return { ...filter, control: "you" };
  if (who.player === "that-player") return { ...filter, control: "that-player" };
  return { ...filter, controlledBy: who };
}

function handOwner(who: PlayerRef): ZoneRef["owner"] {
  return who.player === "you" ? "your" : who.player === "that-player" ? "that-player" : who.player === "they" ? "their" : who;
}

function hasScope(filter: ObjectFilter): boolean {
  return !!(filter.control || filter.controlledBy || filter.own || filter.zone ||
    filter.allOf?.some(hasScope) || filter.anyOf?.some(hasScope));
}

/** Predicate beginning at the verb; the subject is supplied by the caller. */
function parsePredicate(c: Cursor, g: ComparativeGrammar, subject: PlayerRef | "candidate"): Predicate | null {
  return c.attempt((c): Predicate | null => {
    const verb = c.word("control", "controls", "have", "has");
    if (!verb) return null;
    if (verb.startsWith("control")) {
      const degree = parseDegree(c, g);
      const filter = degree && g.parseFilter(c);
      // The compared objects are scoped by each side's controller only.
      if (!degree || !filter || hasScope(filter)) return null;
      const reference = parseReferencePlayer(c, g);
      return reference && { measure: "controls", filter,
        comparison: comparison(degree, { amount: "count", of: controlledBy(filter, reference) }) };
    }
    // "at least 7 life more than <amount>": the margin precedes the noun and
    // the standard is a stated quantity, not another player's life total.
    const surplus = c.attempt((c): Predicate | null => {
      if (!c.words("at", "least")) return null;
      const margin = g.parseCount(c);
      if (!margin || !c.words("life", "more", "than")) return null;
      const reference = g.parseAmount(c);
      return reference && { measure: "life", comparison: { op: "ge", value: { amount: "plus", a: reference, b: margin } } };
    });
    if (surplus) return surplus;
    const degree = parseDegree(c, g);
    if (!degree) return null;
    if (c.word("life") !== null) {
      const reference = parseReferencePlayer(c, g);
      return reference && { measure: "life", comparison: comparison(degree, { amount: "life-total", of: reference }) };
    }
    if (c.word("cards") === null || c.word("in") === null) return null;
    // A possessive must agree with the subject whose hand is measured.
    const possessive = c.word("your", "their");
    if (possessive && subject !== "candidate" && (possessive === "your") !== (subject.player === "you")) return null;
    if (possessive === "your" && subject === "candidate") return null;
    if (c.word("hand") === null) return null;
    const reference = parseReferencePlayer(c, g);
    return reference && { measure: "hand", comparison: comparison(degree,
      { amount: "count", of: { cls: "card", zone: { zone: "hand", owner: handOwner(reference) } } }) };
  });
}

/** "<player> controls more lands than you" and its life/hand analogues. A
 * quantified subject keeps its quantifier: "an opponent" is existential. */
export function parseComparativeCondition(c: Cursor, g: ComparativeGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    const who = g.parsePlayerRef(c);
    const predicate = who && parsePredicate(c, g, who);
    if (!who || !predicate) return null;
    if (predicate.measure === "controls") return { condition: "control-count", who, of: predicate.filter, comparison: predicate.comparison };
    return predicate.measure === "life"
      ? { condition: "life-total", who, comparison: predicate.comparison }
      : { condition: "cards-in-hand", who, comparison: predicate.comparison };
  });
}

/** "… who controls more lands than you": each candidate player supplies the
 * measured side; the reference player is fixed. */
export function parseComparativePlayerQualifier(c: Cursor, g: ComparativeGrammar): PlayerFilter | null {
  return c.attempt((c): PlayerFilter | null => {
    if (c.word("who") === null) return null;
    const predicate = parsePredicate(c, g, "candidate");
    if (!predicate) return null;
    if (predicate.measure === "controls") return { controls: { filter: predicate.filter, comparison: predicate.comparison } };
    return predicate.measure === "life" ? { lifeTotal: predicate.comparison } : { handSize: predicate.comparison };
  });
}
