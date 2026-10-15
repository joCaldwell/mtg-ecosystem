// Ability grants that are not a plain keyword or a quoted ability: a choice
// among keywords, the abilities of other objects, keyword costs stated in a
// following sentence, and "can't have" prohibitions (CR 113.10–113.11).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { AbilityRemoval, CounterSpec, Effect, GainedAbility, GrantedKeywordCost, KeywordInstance, ObjectRef, Sentence } from "../ast.ts";
import { isManaSymbol } from "../symbols.ts";
import type { Cursor } from "./cursor.ts";
import { parseCostList } from "./costs.ts";
import { isCostKeyword, parseKeyword, parseKeywordName, parseProtectionScopes } from "./keywords.ts";
import { parseCounterSpec, parseObjectRef } from "./refs.ts";

type KeywordGrant = Extract<GainedAbility, { gained: "keyword" }>;

/** "your choice of flying, first strike, or haste". The list must close with
 * "or"; the choice belongs to the instruction being applied (CR 608.2d), so
 * continuous static grants never offer it. */
export function parseKeywordChoiceGrant(c: Cursor): Extract<GainedAbility, { gained: "keyword-choice" }> | null {
  return c.attempt(c => {
    if (!c.words("your", "choice", "of")) return null;
    const first = parseKeyword(c);
    if (!first) return null;
    const options = [first];
    for (let closed = false; !closed;) {
      if (c.punct(",")) closed = c.word("or") !== null;
      else if (c.word("or") !== null) closed = true;
      else return c.fail("'or' closing the keyword choice");
      const next = parseKeyword(c);
      if (!next) return null;
      options.push(next);
    }
    return { gained: "keyword-choice", who: { player: "you" }, count: { amount: "fixed", value: 1 }, options, when: "granting-effect-applies" };
  });
}

/** "protection from colorless or from the color of your choice": one
 * protection ability whose quality the granting player chooses among the
 * printed alternatives while the instruction is applied (CR 702.16a, 608.2d). */
export function parseProtectionChoiceGrant(c: Cursor): Extract<GainedAbility, { gained: "keyword-choice" }> | null {
  return c.attempt(c => {
    if (c.word("protection") === null) return null;
    const options: KeywordInstance[] = [];
    for (;;) {
      const from = parseProtectionScopes(c, false, true);
      if (!from || from.length !== 1) return null;
      options.push({ keyword: "protection", from });
      const next = c.peek(1);
      if (!c.isWord("or") || next?.kind !== "word" || next.possessive || next.value !== "from") break;
      c.next();
    }
    if (options.length < 2) return null;
    // The fresh color choice is the granting player's own; a choice by
    // another player would be a second chooser this node cannot carry.
    if (options.some(option => option.from?.some(scope => scope.scope === "fresh-choice" && scope.who.player !== "you"))) return null;
    return { gained: "keyword-choice", who: { player: "you" }, count: { amount: "fixed", value: 1 }, options, when: "granting-effect-applies" };
  });
}

/** CR 702.62a: a card given suspend while in exile gets the bare keyword;
 * its hand-payment parameters are not part of what the effect grants. The
 * time counters of a preceding "exile … with N time counters on it" belong
 * to that exile instruction (`exile.counters`). */
export function parseBareSuspendGrant(c: Cursor): KeywordGrant | null {
  return c.attempt(c => {
    if (c.word("suspend") === null || c.peek()?.kind === "number" || c.isWord("x")) return null;
    return { gained: "keyword", keyword: { keyword: "suspend" } };
  });
}

function contains(value: unknown, test: (node: Record<string, unknown>) => boolean): boolean {
  if (Array.isArray(value)) return value.some(item => contains(item, test));
  if (!value || typeof value !== "object") return false;
  return test(value as Record<string, unknown>) || Object.values(value).some(item => contains(item, test));
}

/** "Exile X with N time counters on it. … it gains suspend." Where the exiled
 * object kept "with N time counters" as a requirement on itself, the
 * sentence that sets up the suspended card was not understood, so the bare
 * suspend grant must not pass. */
export function grantsSuspendAfterCounterFilter(sentences: Sentence[]): boolean {
  const filtered = sentences.findIndex(sentence => contains(sentence.steps, node => node.effect === "exile" &&
    contains(node.what, inner => Array.isArray(inner.counterConstraints) &&
      contains(inner.counterConstraints, constraint => (constraint.counter as { type?: string } | undefined)?.type === "time"))));
  if (filtered < 0) return false;
  return contains(sentences.slice(filtered), node => node.gained === "keyword" &&
    (node.keyword as KeywordInstance).keyword === "suspend" && !(node.keyword as KeywordInstance).cost);
}

const ABILITY_KINDS = ["activated", "triggered", "loyalty"] as const;

/** "all activated abilities of <objects>", "the loyalty abilities of ~".
 * "All" and the plural definite article both grant every such ability; the
 * objects whose abilities are granted keep their own reference grammar. */
export function parseAbilitiesOf(c: Cursor): Extract<GainedAbility, { gained: "abilities-of" }> | null {
  return c.attempt(c => {
    if (c.word("all", "the") === null) return null;
    const kinds: (typeof ABILITY_KINDS)[number][] = [];
    do {
      const kind = c.word(...ABILITY_KINDS) as (typeof ABILITY_KINDS)[number] | null;
      if (!kind || kinds.includes(kind)) return null;
      kinds.push(kind);
    } while (c.word("and") !== null);
    if (c.word("abilities") === null || c.word("of") === null) return null;
    const of = parseObjectRef(c);
    if (!of) return null;
    const exceptMana = c.attempt(c => c.words("except", "mana", "abilities") ? true : null);
    // CR 605.1a: only activated abilities can be the excluded mana abilities
    // of this grant; a mixed list would need per-kind exceptions.
    if (exceptMana && (kinds.length !== 1 || kinds[0] !== "activated")) return null;
    return { gained: "abilities-of", kinds, of, ...(exceptMana ? { except: "mana-abilities" as const } : {}) };
  });
}

function costSentenceStart(c: Cursor, keyword: string): boolean {
  if (c.word("the", "its") === null) return false;
  return parseKeywordName(c) === keyword && c.words("cost", "is", "equal", "to");
}

/** A cost keyword printed without its cost is granted only when the very
 * next sentence defines that cost; the caller must then fold that sentence.
 * CR 702 keyword definitions all take "[cost]" as part of the keyword. */
export function parseDeferredCostKeyword(c: Cursor): KeywordGrant | null {
  return c.attempt(c => {
    const keyword = parseKeywordName(c);
    if (!keyword || !isCostKeyword(keyword)) return null;
    let quoted = false;
    for (let index = c.pos; index < c.tokens.length; index++) {
      const token = c.tokens[index];
      if (token.kind === "punct" && token.value === '"') quoted = !quoted;
      if (quoted || token.kind !== "punct" || token.value !== ".") continue;
      const position = c.pos;
      c.pos = index + 1;
      const defined = costSentenceStart(c, keyword);
      c.pos = position;
      return defined ? { gained: "keyword", keyword: { keyword } } : c.fail(`sentence defining the ${keyword} cost`);
    }
    return c.fail(`sentence defining the ${keyword} cost`);
  });
}

/** "The flashback cost is equal to its mana cost [reduced by {2}] [plus …]".
 * The possessor is the object that has the keyword: a keyword's cost is paid
 * for the card it is on (e.g. CR 702.34a, 702.138a), so "its", "that card's"
 * and "the card's" name each recipient, never the granting source. Leaves the
 * sentence's final period for the caller. */
export function parseKeywordCostDefinition(c: Cursor): { keyword: string; cost: GrantedKeywordCost } | null {
  return c.attempt(c => {
    if (c.word("the", "its") === null) return null;
    const keyword = parseKeywordName(c);
    if (!keyword || !isCostKeyword(keyword) || !c.words("cost", "is", "equal", "to")) return null;
    const possessor = c.word("its") !== null ||
      (c.word("that", "the") !== null && c.possessive("card", "spell") !== null);
    if (!possessor || !c.words("mana", "cost")) return null;
    const cost: GrantedKeywordCost = { manaCostOf: "each-recipient" };
    if (c.words("reduced", "by")) {
      const symbols: string[] = [];
      for (;;) {
        const token = c.peek();
        if (token?.kind !== "symbol" || !isManaSymbol(token.value)) break;
        symbols.push(token.value);
        c.pos++;
      }
      if (!symbols.length) return c.fail("mana reduction");
      cost.reducedBy = symbols;
    }
    if (c.word("plus") !== null) {
      const additional = parseCostList(c);
      if (!additional) return null;
      cost.additional = additional;
    }
    return { keyword, cost };
  });
}

function unresolvedCostGrants(value: unknown, keyword: string | undefined, found: KeywordGrant[]): KeywordGrant[] {
  if (Array.isArray(value)) { for (const item of value) unresolvedCostGrants(item, keyword, found); return found; }
  if (!value || typeof value !== "object") return found;
  if ("gained" in value) {
    const grant = value as GainedAbility;
    // A quoted ability resolved its own sentences when it was parsed.
    if (grant.gained === "quoted") return found;
    if (grant.gained === "keyword" && isCostKeyword(grant.keyword.keyword) && !grant.keyword.cost &&
        !grant.keyword.additionalCosts && !grant.costDefinition &&
        (keyword === undefined || grant.keyword.keyword === keyword)) found.push(grant);
    return found;
  }
  for (const item of Object.values(value)) unresolvedCostGrants(item, keyword, found);
  return found;
}

/** Bind a cost sentence to the single grant of that keyword in the directly
 * preceding instruction. Returns false when no unique grant awaits it. */
export function attachKeywordCostDefinition(scope: unknown, definition: { keyword: string; cost: GrantedKeywordCost }): boolean {
  const grants = unresolvedCostGrants(scope, definition.keyword, []);
  if (grants.length !== 1) return false;
  grants[0].costDefinition = definition.cost;
  return true;
}

/** Safety net for every caller of the grant grammar: a cost keyword whose
 * cost sentence was never folded must not survive as a complete ability. */
export function hasUnresolvedKeywordCost(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(hasUnresolvedKeywordCost);
  if (!value || typeof value !== "object") return false;
  if ("gained" in value) {
    const grant = value as GainedAbility;
    if (grant.gained === "quoted") return hasUnresolvedKeywordCost(grant.ability);
    if (grant.gained !== "keyword") return false;
    return isCostKeyword(grant.keyword.keyword) && !grant.keyword.cost && !grant.keyword.additionalCosts && !grant.costDefinition;
  }
  return Object.values(value).some(hasUnresolvedKeywordCost);
}

/** CR 113.11: "can't have [or gain] <keyword>" removes the ability and stops
 * any effect or keyword counter from adding it. "Or" under the negation
 * prohibits every listed keyword. */
export function parseAbilityProhibition(c: Cursor): KeywordInstance[] | null {
  return c.attempt(c => {
    if (!c.words("can't", "have")) return null;
    c.words("or", "gain");
    const first = parseKeyword(c);
    if (!first) return null;
    const abilities = [first];
    for (;;) {
      const next = c.attempt(c => {
        const comma = c.punct(",");
        if (c.word("or") === null && !comma) return null;
        return parseKeyword(c);
      });
      if (!next) break;
      abilities.push(next);
    }
    return abilities;
  });
}

/** CR 113.6i: "can't have [kind] counters put on it/them". Nothing can put
 * such a counter on the object, by cost or effect, including as it enters;
 * counters already on it stay. */
export function parseCounterProhibition(c: Cursor): CounterSpec | "any" | null {
  return c.attempt((c): CounterSpec | "any" | null => {
    if (!c.words("can't", "have")) return null;
    const counter = c.isWord("counters") ? "any" : parseCounterSpec(c);
    if (!counter || c.word("counters") === null || !c.words("put", "on") || c.word("it", "them") === null) return null;
    return counter;
  });
}

/** The resolving form shares the grammatical subject of the preceding
 * coordinated modifier ("…lose hexproof and can't have hexproof"). */
export function parseAbilityProhibitionEffect(c: Cursor, previous: ObjectRef | undefined): Extract<Effect, { effect: "cant-have-abilities" }> | null {
  return c.attempt(c => {
    const what = c.attempt(parseObjectRef) ?? previous;
    if (!what) return null;
    const abilities = parseAbilityProhibition(c);
    return abilities && { effect: "cant-have-abilities", what, abilities };
  });
}

/** "all landwalk abilities", "all "bands with other" abilities": every
 * ability of a parameterized keyword family, written as the keyword without
 * its parameter (CR 702.14a; 702.22b). */
export function parseKeywordFamilyRemoval(c: Cursor): Extract<AbilityRemoval, { removal: "listed" }> | null {
  return c.attempt(c => {
    if (c.word("all") === null) return null;
    let keyword: string | null = null;
    if (c.word("landwalk") !== null) keyword = "landwalk";
    else if (c.punct("\"") && c.words("bands", "with", "other") && c.punct("\"")) keyword = "bands with other";
    if (!keyword || c.word("abilities") === null) return null;
    return { removal: "listed", abilities: [{ gained: "keyword", keyword: { keyword } }] };
  });
}
