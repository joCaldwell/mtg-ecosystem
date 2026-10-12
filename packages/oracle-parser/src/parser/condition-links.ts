// Conditions that query this object's own linked decisions or this
// ability's own history, rather than the current game state.
// CR 118.9, 118.12, 602.2, 607.2, 701.59c, 702.35b, 702.76a, 702.104b,
// 702.117a, 702.137a, 702.138b, 702.166b, 702.174k, 707.10b:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Comparison, Condition, LinkedChoiceKeyword, ObjectRef } from "../ast.ts";
import { isManaSymbol } from "../symbols.ts";
import type { ConditionGrammar } from "./conditions.ts";
import { Cursor } from "./cursor.ts";

const negated = (of: Condition, negate: boolean): Condition => negate ? { condition: "not", of } : of;

/** "was" / "wasn't" / "was not"; null when no past copula is present. */
function parseWas(c: Cursor): { negate: boolean } | null {
  if (c.word("wasn't") !== null) return { negate: true };
  if (c.word("was") === null) return null;
  return { negate: c.word("not") !== null };
}

/** A spell or the permanent it became, never a freshly selected object. */
function parseCastObject(c: Cursor, g: ConditionGrammar): ObjectRef | null {
  return c.attempt(c => {
    const what = g.parseObjectRef(c);
    if (!what || !["self", "this", "it", "that"].includes(what.ref)) return null;
    return what.ref === "that" && what.plural ? null : what;
  });
}

const ALTERNATIVE_COST_KEYWORDS = ["prowl", "surge", "madness", "spectacle"] as const;

function parseKeywordChoice(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    const linked = c.attempt((c): Condition | null => {
      let keyword: LinkedChoiceKeyword;
      let verb: string;
      if (c.words("the", "gift")) { keyword = "gift"; verb = "promised"; }
      else if (c.word("tribute") !== null) { keyword = "tribute"; verb = "paid"; }
      else if (c.word("evidence") !== null) { keyword = "collect-evidence"; verb = "collected"; }
      else return null;
      const was = parseWas(c);
      if (!was || c.word(verb) === null) return null;
      return negated({ condition: "keyword-choice", keyword }, was.negate);
    });
    if (linked) return linked;
    const cost = c.attempt((c): Condition | null => {
      if (c.word("its") === null) return null;
      const keyword = c.word(...ALTERNATIVE_COST_KEYWORDS) as typeof ALTERNATIVE_COST_KEYWORDS[number] | null;
      if (!keyword || c.word("cost") === null) return null;
      const was = parseWas(c);
      if (!was || c.word("paid") === null) return null;
      return negated({ condition: "keyword-choice", keyword, what: { ref: "it" } }, was.negate);
    });
    if (cost) return cost;
    let what: ObjectRef | null;
    let negate = false;
    // "If it's bargained" is asked while the spell is being cast; the
    // declared intention is already fixed then (CR 601.2b, 702.166b).
    if (c.word("it's") !== null) {
      what = { ref: "it" };
      negate = c.word("not") !== null;
      return c.word("bargained") !== null ? negated({ condition: "keyword-choice", keyword: "bargain", what }, negate) : null;
    }
    what = parseCastObject(c, g);
    if (!what) return null;
    if (c.word("escaped") !== null) return { condition: "keyword-choice", keyword: "escape", what };
    const was = parseWas(c);
    if (!was || c.word("bargained") === null) return null;
    return negated({ condition: "keyword-choice", keyword: "bargain", what }, was.negate);
  });
}

/** "The {2}{W}{W} cost was paid" names the alternative cost printed on this
 * same spell by its exact mana symbols. */
function parseAlternativeCostPaid(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    if (c.word("the") === null) return null;
    const symbols: string[] = [];
    while (c.peek()?.kind === "symbol") {
      const symbol = c.symbol();
      if (!symbol || !isManaSymbol(symbol)) return null;
      symbols.push(symbol);
    }
    if (!symbols.length || c.word("cost") === null) return null;
    const was = parseWas(c);
    if (!was || c.word("paid") === null) return null;
    return negated({ condition: "alternative-cost-paid", cost: [{ cost: "mana", symbols }] }, was.negate);
  });
}

const ORDINALS: Record<string, number> = {
  first: 1, second: 2, third: 3, fourth: 4, fifth: 5, sixth: 6, seventh: 7, eighth: 8, ninth: 9, tenth: 10,
};

function parseOrdinal(c: Cursor): number | null {
  const word = c.word(...Object.keys(ORDINALS));
  return word ? ORDINALS[word] : null;
}

const RESOLVED = ["time", "this", "ability", "has", "resolved", "this", "turn"];

/** The abbreviated "if it's the third time" repeats an earlier complete
 * clause of the same ability; without that clause it has no meaning here. */
function followsResolutionClause(c: Cursor): boolean {
  for (let start = 0; start + RESOLVED.length <= c.pos; start++) {
    if (RESOLVED.every((value, offset) => {
      const token = c.tokens[start + offset];
      return token.kind === "word" && !token.possessive && token.value === value;
    })) return true;
  }
  return false;
}

function parseResolutionOrdinal(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    if (c.words("this", "is", "the")) {
      const ordinals: number[] = [];
      for (;;) {
        const ordinal = parseOrdinal(c);
        if (ordinal === null) return null;
        ordinals.push(ordinal);
        if (c.word("or") === null) break;
      }
      if (!c.words(...RESOLVED)) return null;
      return { condition: "ability-resolution-ordinal", ordinals, during: "this-turn" };
    }
    if (c.word("it's") === null || c.word("the") === null) return null;
    const ordinal = parseOrdinal(c);
    if (ordinal === null || c.word("time") === null || !c.isPunct(",")) return null;
    const complete = followsResolutionClause(c);
    return complete ? { condition: "ability-resolution-ordinal", ordinals: [ordinal], during: "this-turn" }
      : c.fail("earlier complete resolution-count clause");
  });
}

function parseActivationCount(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    if (!c.words("this", "ability", "has", "been", "activated")) return null;
    const explicit = c.isWord("exactly");
    const comparison: Comparison | null = g.parseComparison(c);
    // A bare "four times" does not say whether more activations qualify.
    if (!comparison || (comparison.op === "eq" && !explicit) || c.word("time", "times") === null) return null;
    return c.words("this", "turn") ? { condition: "ability-activation-count", comparison, during: "this-turn" } : null;
  });
}

/** CR 118.12: only the bare clause directly before its comma. A restated
 * action ("if you can't sacrifice a creature") is a different question. */
function parseUnable(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    const who = g.parsePlayerRef(c);
    if (!who || c.word("can't") === null || !c.isPunct(",")) return null;
    return { condition: "preceding-action", who, outcome: "unable" };
  });
}

export function parseLinkedCondition(c: Cursor, g: ConditionGrammar): Condition | null {
  return parseKeywordChoice(c, g) ?? parseAlternativeCostPaid(c) ?? parseResolutionOrdinal(c)
    ?? parseActivationCount(c, g) ?? parseUnable(c, g);
}
