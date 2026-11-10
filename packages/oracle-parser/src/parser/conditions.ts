// Structured condition expressions. Reference parsers are passed in so refs.ts
// can delegate here without a runtime import cycle.
// CR 603.4 (intervening conditions), 702.33d–g (kicked and linked costs),
// 601.2i (casting events), 400.7d (originating spell), 508.1/508.4/509.1
// (declared combat actions), 608.2i (history), 700.4 (dies), and 119:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

import type { Amount, Comparison, Condition, Cost, CounterSpec, GroupConstraint, ObjectFilter, ObjectRef, PhaseName, PlayerRef, ZoneRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { isManaSymbol } from "../symbols.ts";
import { parseKeywordName } from "./keywords.ts";
import { parseDiceCondition } from "./dice.ts";
import { parseAggregateCount } from "./aggregates.ts";
import { parseControlledSubset, parseGroupConstraint } from "./selection-constraints.ts";
import { parseCharacteristicRelationCondition } from "./characteristic-relations.ts";
import { parseCurrentPhaseCondition } from "./temporal.ts";
import { parsePutActionOutcome } from "./action-outcomes.ts";
import { parseEventOutcome } from "./event-outcomes.ts";
import { parseDesignationCondition } from "./condition-designations.ts";
import { parseLinkedCondition } from "./condition-links.ts";
import { parsePlayerCondition } from "./condition-players.ts";
import { parseComparativeCondition } from "./comparatives.ts";
import { parseEventCondition } from "./condition-events.ts";
import { parsePairedCondition } from "./soulbond.ts";
import { parseMostCommonColorCondition } from "./color-majority.ts";
import { parseExcludedOrigin } from "./zone-origins.ts";
import { parseCastOrdinal } from "./cast-ordinals.ts";
import { parseFirstEvent } from "./event-firsts.ts";
import { parseAttributeComparison } from "./attribute-comparisons.ts";
import { parseGraveyardPosition } from "./graveyard-position.ts";

export interface ConditionGrammar {
  /** In an unless clause, bare present-tense actions describe optional
   * payments now (CR 118.12a), not a historical condition. */
  purpose?: "unless";
  /** Set for conditions of resolving instructions only: a "this way" outcome
   * test looks back at an earlier instruction of the same sequence. */
  outcomes?: true;
  parseAmount(c: Cursor): Amount | null;
  parseComparison(c: Cursor): Comparison | null;
  parseCount(c: Cursor): Amount | null;
  parseCounterSpec(c: Cursor): CounterSpec | null;
  parseFilter(c: Cursor): ObjectFilter | null;
  parseObjectRef(c: Cursor): ObjectRef | null;
  parsePlayerRef(c: Cursor): PlayerRef | null;
  parsePossessivePlayer?(c: Cursor): PlayerRef | null;
  parsePossessiveObject?(c: Cursor): ObjectRef | null;
  parseZoneRef(c: Cursor): ZoneRef | null;
  /** Existing atomic condition productions; must not recurse to this entry. */
  parseAtomic(c: Cursor): Condition | null;
}

const one: Amount = { amount: "fixed", value: 1 };
const zero: Amount = { amount: "fixed", value: 0 };
const negated = (condition: Condition, negate: boolean): Condition => negate ? { condition: "not", of: condition } : condition;

function parseComparison(c: Cursor, g: ConditionGrammar): Comparison | null {
  return c.attempt(c => {
    if (c.word("no") !== null) return { op: "eq", value: zero };
    const bound = c.words("at", "least") ? "ge" : c.words("at", "most") ? "le" : null;
    if (bound) {
      const value = g.parseAmount(c);
      return value && { op: bound, value };
    }
    if (c.words("more", "than")) {
      const value = g.parseAmount(c);
      return value && { op: "gt", value };
    }
    if (c.words("fewer", "than")) {
      const value = g.parseAmount(c);
      return value && { op: "lt", value };
    }
    return g.parseComparison(c);
  });
}

function parseKicked(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const what = g.parseObjectRef(c);
    if (!what) return null;
    let negate = false;
    if (c.word("wasn't", "isn't") !== null) negate = true;
    else if (c.word("was", "is") !== null) negate = c.word("not") !== null;
    else return null;
    if (c.word("kicked") === null) return null;
    let comparison: Comparison = { op: "ge", value: one };
    if (c.word("twice") !== null) comparison = { op: "eq", value: { amount: "fixed", value: 2 } };
    else if (c.word("once") !== null) comparison = { op: "eq", value: one };
    else {
      const times = c.attempt(c => {
        const amount = parseComparison(c, g);
        return amount && c.word("time", "times") !== null ? amount : null;
      });
      if (times) comparison = times;
    }
    let kickerCost: Cost[] | undefined;
    if (c.word("with") !== null) {
      if (c.word("its") === null) return null;
      const symbols: string[] = [];
      while (c.peek()?.kind === "symbol") {
        const symbol = c.symbol();
        if (!symbol || !isManaSymbol(symbol)) return null;
        symbols.push(symbol);
      }
      if (symbols.length === 0 || c.word("kicker") === null) return null;
      kickerCost = [{ cost: "mana", symbols }];
    }
    return negated({ condition: "kicked", what, comparison, ...(kickerCost ? { kickerCost } : {}) }, negate);
  });
}

function parseHandCount(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const who = g.parsePlayerRef(c);
    if (!who || c.word("have", "has") === null) return null;
    const article = c.word("a", "an") !== null;
    const comparison = article ? { op: "ge" as const, value: one } : parseComparison(c, g);
    if (!comparison || c.word("card", "cards") === null || c.word("in") === null) return null;
    if (c.word("your") !== null && who.player !== "you") return null;
    if (c.word("hand") === null) return null;
    return { condition: "cards-in-hand", who, comparison };
  });
}

function parseSearchHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const who = g.parsePlayerRef(c);
    if (!who || c.word("search", "searched", "searches") === null) return null;
    const zone = g.parseZoneRef(c);
    if (!zone || zone.position || !c.words("this", "way")) return null;
    if (["hand", "library", "graveyard"].includes(zone.zone) && zone.owner === undefined) return null;
    return { condition: "searched-zone", who, zone, scope: "this-way" };
  });
}

/** A current location predicate, not a zone-change event or a promise that
 * an object stays there. CR 400.1 defines the zones; CR 602.1b/c makes this
 * predicate part of activation legality when used in an activation limit. */
function parseZoneState(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    let what: ObjectRef | null;
    let negate = false;
    if (c.word("it's") !== null) what = { ref: "it" };
    else {
      const offset = c.tokens.slice(c.pos).findIndex(t => t.kind === "word" && !t.possessive &&
        ["is", "are", "isn't", "aren't"].includes(t.value));
      if (offset <= 0) return null;
      const subject = new Cursor(c.tokens.slice(c.pos, c.pos + offset));
      what = g.parseObjectRef(subject);
      if (!what || !subject.done()) return null;
      // A fresh quantified population needs a count predicate, not a location
      // test of a previously identified object or object group.
      if (what.ref === "filter" || what.ref === "each") return null;
      c.pos += offset;
      const verb = c.word("is", "are", "isn't", "aren't");
      negate = verb === "isn't" || verb === "aren't";
      if (!negate) negate = c.word("not") !== null;
    }
    if (c.word("in", "on") === null) return null;
    const zone = g.parseZoneRef(c);
    if (!zone || zone.position || (["hand", "library", "graveyard"].includes(zone.zone) && zone.owner === undefined)) return null;
    // "in the command zone or on the battlefield": membership of either zone.
    // Only a repeated preposition marks a second zone (Eminence).
    const second = c.attempt((c): ZoneRef | null => {
      if (c.word("or") === null || c.word("in", "on") === null) return null;
      const other = g.parseZoneRef(c);
      return other && !other.position && !(["hand", "library", "graveyard"].includes(other.zone) && other.owner === undefined) ? other : null;
    });
    return negated({ condition: "is-filter", what, filter: second ? { anyOf: [{ zone }, { zone: second }] } : { zone } }, negate);
  });
}

/** Current cards in a personal zone (CR 400, 401, 402, 404), not a count of
 * cards drawn/discarded earlier. Keep the stated player as the zone owner. */
function parsePersonalZoneCount(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const who = g.parsePlayerRef(c);
    if (!who || !["you", "that-player", "they", "controller", "owner", "target-player", "target-opponent", "an-opponent", "defending-player"].includes(who.player) ||
        c.word("have", "has") === null) return null;
    const comparison = c.word("a", "an") !== null ? { op: "ge" as const, value: one } : parseComparison(c, g);
    const of = comparison && g.parseFilter(c);
    if (!of || !comparison || of.cls !== "card" || !of.zone ||
        !["library", "hand", "graveyard"].includes(of.zone.zone) || of.zone.position) return null;
    const owner = of.zone.owner;
    if (owner === "your" ? who.player !== "you" : owner === "their" ? who.player === "you" : owner !== undefined) return null;
    return { condition: "count", of: { ...of, zone: { ...of.zone, owner: who } }, comparison };
  });
}

/** "You own a card in exile": exile is a shared zone (CR 406.1), so the
 * cards counted are those in it that the player owns (108.3). */
function parseOwnedInExile(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    if (!c.words("you", "own")) return null;
    const comparison = c.word("a", "an") !== null ? { op: "ge" as const, value: one } : parseComparison(c, g);
    const of = comparison && g.parseFilter(c);
    if (!of || !comparison || of.cls !== "card" || of.zone?.zone !== "exile" || of.zone.owner || of.zone.position || of.own || of.ownedBy) return null;
    return { condition: "count", of: { ...of, own: "you" }, comparison };
  });
}

/** "A graveyard has twenty or more cards in it": one player's zone holds
 * that many cards; the cards of different players are never pooled. */
function parseZoneSize(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    if (c.word("a") === null) return null;
    const zone = c.word("graveyard", "library", "hand") as "graveyard" | "library" | "hand" | null;
    if (!zone || c.word("has") === null) return null;
    const comparison = parseComparison(c, g);
    if (!comparison || c.word("card", "cards") === null || !c.words("in", "it")) return null;
    return { condition: "count", of: { cls: "card", zone: { zone, owner: "any-single" } }, comparison };
  });
}

function hasZone(filter: ObjectFilter): boolean {
  return !!filter.zone || !!filter.allOf?.some(hasZone) || !!filter.anyOf?.some(hasZone);
}

/** "Twenty or more creature cards are in your graveyard" explicitly counts
 * the current zone contents. The copula cannot be discarded as filler in
 * historical clauses such as "were put into" or "have left". */
function parseCopularZoneCount(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    // "An artifact card is in your graveyard" is existential, not exactly one.
    const existential = c.word("a", "an") !== null;
    const comparison = existential ? { op: "ge" as const, value: one } : parseComparison(c, g);
    const of = comparison && g.parseFilter(c);
    if (!of || !comparison || hasZone(of) || c.word("are", "is") === null) return null;
    // "Four or more creatures are attacking": the current number of
    // attacking creatures (CR 506.4), as "there are … attacking creatures".
    // Existential "a creature is attacking" keeps its status condition.
    if (!existential && !of.status && c.word("attacking") !== null && !c.isWord("you", "a", "an", "that", "this", "planeswalkers", "battles", "it", "them"))
      return { condition: "count", of: { ...of, status: ["attacking"] }, comparison };
    if (c.words("on", "the", "battlefield")) return { condition: "count", of: { ...of, zone: { zone: "battlefield" } }, comparison };
    if (c.word("in") === null) return null;
    const zone = g.parseZoneRef(c);
    if (!zone || zone.position || (["hand", "library", "graveyard"].includes(zone.zone) && zone.owner === undefined)) return null;
    return { condition: "count", of: { ...of, zone }, comparison };
  });
}

/** "There are two or more ki counters on this creature" states the same
 * fact as "this creature has two or more ki counters on it" (CR 122.1). */
function parseCountersOn(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const explicit = c.isWord("no", "exactly");
    const comparison = c.word("a", "an") !== null ? { op: "ge" as const, value: one } : parseComparison(c, g);
    if (!comparison || (comparison.op === "eq" && !explicit)) return null;
    const counter = c.isWord("counter", "counters") ? "any" : g.parseCounterSpec(c);
    if (!counter || c.word("counter", "counters") === null) return null;
    // "Counters among permanents you control" is the total over that group
    // (CR 122.1), the same value "the number of counters among" states.
    if (c.word("among") !== null) {
      const on = g.parseObjectRef(c);
      return on && (on.ref === "each" || on.ref === "all") ? { condition: "compare", value: { amount: "counter-count", counter, on }, comparison } : null;
    }
    if (c.word("on") === null) return null;
    const what = parseHistoricalObject(c, g);
    if (!what || what.ref === "them" || (what.ref === "that" && what.plural)) return null;
    return { condition: "is-filter", what, filter: { counterConstraints: [{ counter, comparison }] } };
  });
}

function parseQualifiedCount(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    if (c.words("there", "are") || c.words("there", "is") || c.word("there's") !== null) {
      const counters = parseCountersOn(c, g);
      if (counters) return counters;
      // "There is a land card in your graveyard" is existential, not exactly one.
      const comparison = c.word("a", "an") !== null ? { op: "ge" as const, value: one } : parseComparison(c, g);
      const of = comparison && g.parseFilter(c);
      return of && comparison ? { condition: "count", of, comparison } : null;
    }
    const who = g.parsePlayerRef(c);
    if (!who || c.word("control", "controls") === null) return null;
    // Bare "you control a creature" retains its existing existential node;
    // here the explicit numeric qualifiers determine comparison semantics.
    if (!c.isWord("no", "exactly", "at", "more", "fewer")) return null;
    const comparison = parseComparison(c, g);
    const of = comparison && g.parseFilter(c);
    return of && comparison ? { condition: "control-count", who, of, comparison } : null;
  });
}

function parseHistorySubject(c: Cursor, g: ConditionGrammar): { who: PlayerRef; negate: boolean } | null {
  return c.attempt(c => {
    if (c.word("you've") !== null) return { who: { player: "you" }, negate: false };
    const who = g.parsePlayerRef(c);
    if (!who) return null;
    const negate = c.word("haven't", "hasn't", "didn't") !== null || c.words("have", "not") || c.words("has", "not") || c.words("did", "not");
    if (!negate) c.word("have", "has", "did");
    return { who, negate };
  });
}

function parseHistoryPeriod(c: Cursor): "this-turn" | "this-combat" | null {
  return c.attempt(c => {
    if (c.words("this", "turn")) return "this-turn";
    if (c.words("this", "combat")) return "this-combat";
    return null;
  });
}

/** "Last turn" is the whole turn before the current one, whoever's it was;
 * it is not "your last turn" or "since your last turn". */
function parseLookbackPeriod(c: Cursor): "this-turn" | "this-combat" | "last-turn" | null {
  return c.attempt(c => c.words("last", "turn") && !c.isWord("ended") ? "last-turn" as const : null) ?? parseHistoryPeriod(c);
}

function parseHistoricalObject(c: Cursor, g: ConditionGrammar): ObjectRef | null {
  return c.attempt(c => {
    const what = g.parseObjectRef(c);
    // Generic quantified nouns belong to event-count predicates. They cannot
    // silently become one particular historical object.
    return what && ["self", "this", "it", "them", "that", "enchanted", "equipped"].includes(what.ref) ? what : null;
  });
}

function parsePastNegation(c: Cursor): boolean {
  if (c.word("didn't", "hasn't", "haven't") !== null) return true;
  c.word("did", "has", "have");
  return c.word("not") !== null;
}

function parseCombatHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    // CR 508.1b: passive "you've been attacked this step" (declared as an attacker's target).
    if (c.words("you've", "been", "attacked", "this", "step")) return { condition: "was-attacked", who: { player: "you" }, during: "this-step" };
    const player = c.attempt((c): Condition | null => {
      const who = c.word("you've") !== null ? { player: "you" as const } : g.parsePlayerRef(c);
      if (!who) return null;
      const negate = parsePastNegation(c);
      if (c.word("attacked", ...(negate ? ["attack"] : [])) === null) return null;
      let withFilter: ObjectFilter | undefined;
      let groupConstraints: GroupConstraint[] | undefined;
      let attackerCount: Comparison | undefined;
      if (c.word("with") !== null) {
        const article = c.word("a", "an") !== null;
        // "attacked with three or more creatures": a count of the attackers (CR 508.1a).
        const count = article || negate ? null : c.attempt(c => {
          const comparison = parseComparison(c, g);
          return comparison && (comparison.op === "ge" || comparison.op === "gt" || comparison.op === "le") ? comparison : null;
        });
        if (count) attackerCount = count;
        const filter = g.parseFilter(c);
        if (!filter) return null;
        withFilter = filter;
        // "attacked with creatures with total power 6 or greater" (pack
        // tactics): a total over the creatures attacked with, not each one.
        const total = !article && !negate && c.isWord("with") ? parseGroupConstraint(c, c => parseComparison(c, g)) : null;
        if (total?.constraint === "attribute-sum") groupConstraints = [total];
        else if (total) return null;
      }
      const during = parseHistoryPeriod(c);
      return during && negated({ condition: "attacked", who, during, ...(withFilter ? { with: withFilter } : {}),
        ...(attackerCount ? { count: attackerCount } : {}),
        ...(groupConstraints ? { groupConstraints } : {}) }, negate);
    });
    if (player) return player;
    const what = parseHistoricalObject(c, g);
    if (!what) return null;
    const passive = c.attempt((c): Condition | null => {
      const copula = c.word("was", "wasn't");
      if (!copula) return null;
      const negate = copula === "wasn't" || c.word("not") !== null;
      if (c.word("blocked") === null) return null;
      const during = parseHistoryPeriod(c);
      return during && negated({ condition: "combat-action-history", what, action: "was-blocked", during }, negate);
    });
    if (passive) return passive;
    const negate = parsePastNegation(c);
    const verb = c.word("attacked", "blocked", ...(negate ? ["attack", "block"] : []));
    if (!verb) return null;
    const action = verb === "attack" ? "attacked" : verb === "block" ? "blocked" : verb;
    if (action !== "attacked" && action !== "blocked") return null;
    let join: "and" | "or" | undefined;
    let second: "attacked" | "blocked" | undefined;
    if (c.isWord("and", "or")) {
      const parsed = c.attempt((c): { conjunction: "and" | "or"; action: string } | null => {
        const conjunction = c.word("and", "or");
        const other = c.word("attacked", "blocked", ...(negate ? ["attack", "block"] : []));
        if (!other || (conjunction !== "and" && conjunction !== "or")) return null;
        return { conjunction, action: other === "attack" ? "attacked" as const : other === "block" ? "blocked" as const : other };
      });
      if (parsed) {
        if (parsed.action !== "attacked" && parsed.action !== "blocked") return null;
        if (negate && parsed.conjunction === "and") return null;
        join = parsed.conjunction;
        second = parsed.action;
      }
    }
    const during = parseHistoryPeriod(c) ?? c.attempt(c => {
      if (c.word("during") === null) return null;
      const lastTurnOf = parseHistoryOwner(c, g);
      return lastTurnOf && c.words("last", "turn") ? { lastTurnOf } : null;
    });
    if (!during) return null;
    const first: Condition = { condition: "combat-action-history", what, action, during };
    return negated(second ? { condition: join === "and" ? "all-of" : "any-of", conditions: [first,
      { condition: "combat-action-history", what, action: second, during },
    ] } : first, negate);
  });
}

function parseHistoryOwner(c: Cursor, g: ConditionGrammar): PlayerRef | null {
  return c.attempt(c => c.word("your") !== null ? { player: "you" } : g.parsePossessivePlayer?.(c) ?? null);
}

/** Keywords are a closed registry; a named counter is not a keyword ability.
 * Keep each predicate's subject outside the filter's own relative clauses. */
function parseHasKeyword(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const offset = c.tokens.slice(c.pos).findIndex(token => token.kind === "word" && !token.possessive &&
      ["has", "have", "doesn't", "don't"].includes(token.value));
    if (offset <= 0) return null;
    const subject = new Cursor(c.tokens.slice(c.pos, c.pos + offset));
    const what = g.parseObjectRef(subject);
    if (!what || !subject.done()) return null;
    c.pos += offset;
    const negate = c.word("doesn't", "don't") !== null;
    if (negate ? c.word("have") === null : c.word("has", "have") === null) return null;
    const keyword = parseKeywordName(c);
    if (!keyword) return null;
    return negated({ condition: "has-keyword", what, keyword }, negate);
  });
}

/** CR 106.1–3, 107.4h, 609.4b: payment history preserves actual mana, including
 * source provenance. Hybrid and Phyrexian cost symbols cannot identify the
 * mana actually spent and are intentionally not accepted as history. */
function parseManaSpent(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    type ManaHistory = Extract<Condition, { condition: "mana-spent-to-cast" }>;
    const requirements: { mana: ManaHistory["mana"]; comparison: Comparison }[] = [];
    let source: ObjectFilter | undefined;
    if (c.peek()?.kind === "symbol") {
      const types: Record<string, ManaHistory["mana"]> = { W: "white", U: "blue", B: "black", R: "red", G: "green", C: "colorless", S: "snow" };
      const counts = new Map<ManaHistory["mana"], number>();
      while (c.peek()?.kind === "symbol") {
        const symbol = c.symbol();
        const mana = symbol && types[symbol];
        if (!mana) return null;
        counts.set(mana, (counts.get(mana) ?? 0) + 1);
      }
      // One snow-colored unit can satisfy two independent predicates, but
      // cannot pay two separate symbols. Mixed snow/type allocation needs a
      // payment-matching node rather than intersecting minima.
      if (counts.has("snow") && counts.size > 1) return null;
      for (const [mana, value] of counts) requirements.push({ mana, comparison: { op: "ge", value: { amount: "fixed", value } } });
    } else {
      const comparison = c.isWord("mana") ? { op: "gt" as const, value: zero } : parseComparison(c, g);
      if (!comparison) return null;
      const type = c.word("colored", "white", "blue", "black", "red", "green", "colorless", "snow");
      if (c.word("mana") === null) return null;
      const sameColor = !type && c.words("of", "the", "same", "color");
      const mana: ManaHistory["mana"] = sameColor ? "same-color" : type as Exclude<ManaHistory["mana"], "any" | "same-color"> | null ?? "any";
      requirements.push({ mana, comparison });
      if (c.word("from") !== null) {
        c.word("a", "an");
        const filter = g.parseFilter(c);
        if (!filter) return null;
        source = filter;
      }
    }
    const negate = c.word("wasn't", "weren't") !== null;
    if (!negate && c.word("was", "were") === null) return null;
    if (!c.words("spent", "to", "cast")) return null;
    const what = parseHistoricalObject(c, g);
    if (!what) return null;
    const conditions: Condition[] = requirements.map(requirement => ({ condition: "mana-spent-to-cast", what, ...requirement,
      ...(source ? { source } : {}) }));
    return negated(conditions.length === 1 ? conditions[0]! : { condition: "all-of", conditions }, negate);
  });
}

function parseDamageRecipient(c: Cursor, g: ConditionGrammar): PlayerRef | ObjectRef | null {
  return c.attempt(c => {
    const player = g.parsePlayerRef(c);
    if (player) {
      // An existential recipient is one player, not the summed damage of all
      // players matching a collective description.
      return ["each-player", "each-opponent", "all-opponents", "opponents"].includes(player.player) ? null : player;
    }
    const what = g.parseObjectRef(c);
    if (!what || what.ref === "each" || what.ref === "them") return null;
    if (what.ref === "filter" && (what.count?.amount !== "fixed" || what.count.value !== 1)) return null;
    return what;
  });
}

function parseDamageQuantity(c: Cursor, g: ConditionGrammar): {
  comparison: Comparison; quantity: "positive-event" | "total"; damage?: "combat" | "noncombat"; excess?: true;
} | null {
  return c.attempt(c => {
    const comparison = parseComparison(c, g);
    // CR 120.10: "excess damage" is a property of a damage event to a
    // creature, planeswalker or battle; only the bare existential form.
    const excess = !comparison && c.word("excess") !== null;
    const damage = c.word("combat", "noncombat") as "combat" | "noncombat" | null;
    if (c.word("damage") === null) return null;
    return { comparison: comparison ?? { op: "gt", value: zero }, quantity: comparison ? "total" : "positive-event",
      ...(damage ? { damage } : {}), ...(excess ? { excess: true as const } : {}) };
  });
}

/** CR 120.3–4, 120.8, 608.2i: damage history does not test life totals or
 * marked damage. Prefix amounts sum damage to/from one matched referent. */
function parseDamageHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    type History = Extract<Condition, { condition: "damage-history" }>;
    let source: ObjectRef | undefined;
    let to: ObjectRef | PlayerRef | undefined;
    let negate = false;
    let quantity: ReturnType<typeof parseDamageQuantity>;
    const front = c.attempt(c => {
      const amount = parseDamageQuantity(c, g);
      if (!amount || c.word("was") === null || !c.words("dealt", "to")) return null;
      const recipient = parseDamageRecipient(c, g);
      return recipient && { amount, recipient };
    });
    if (front) { quantity = front.amount; to = front.recipient; }
    else {
      const active = c.attempt(c => {
        const what = parseHistoricalObject(c, g);
        if (!what || what.ref === "them") return null;
        const negative = parsePastNegation(c);
        if (c.word("dealt", ...(negative ? ["deal"] : [])) === null) return null;
        return { what, negative };
      });
      if (active) { source = active.what; negate = active.negative; }
      else {
        if (c.word("you've") !== null) {
          to = { player: "you" };
          if (c.word("been") === null) return null;
        } else {
          const recipient = parseDamageRecipient(c, g);
          if (!recipient) return null;
          to = recipient;
          if (c.word("wasn't", "weren't") !== null) negate = true;
          else if (c.word("was", "were") !== null) negate = c.word("not") !== null;
          else if (c.word("hasn't", "haven't") !== null) { negate = true; if (c.word("been") === null) return null; }
          else if (c.word("has", "have") !== null) { negate = c.word("not") !== null; if (c.word("been") === null) return null; }
          else return null;
        }
        if (c.word("dealt") === null) return null;
      }
      quantity = parseDamageQuantity(c, g);
      if (!quantity) return null;
      if (source && c.word("to") !== null) {
        const recipient = parseDamageRecipient(c, g);
        if (!recipient) return null;
        to = recipient;
      }
    }
    let during: History["during"] | null = null;
    // "Hasn't dealt damage yet": no period, the source's whole existence as
    // this object (CR 400.7). Only the printed negative active form.
    if (source && negate && quantity.quantity === "positive-event" && !quantity.damage && !to && c.word("yet") !== null) during = "object-lifetime";
    for (let part = 0; part < 2 && during !== "object-lifetime"; part++) {
      if (!during && c.isWord("this", "last")) during = parseLookbackPeriod(c);
      else if (!source && c.word("by") !== null) {
        const object = g.parseObjectRef(c);
        if (!object || object.ref === "each" || object.ref === "them" ||
          (object.ref === "filter" && (object.count?.amount !== "fixed" || object.count.value !== 1))) return null;
        source = object;
      } else break;
    }
    if (!during || (quantity.excess && (!to || "player" in to))) return null;
    return negated({ condition: "damage-history", ...quantity, ...(source ? { source } : {}), ...(to ? { to } : {}), during }, negate);
  });
}

function parseCastObjectHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const active = c.attempt(c => {
      const contractedPerfect = c.word("you've") !== null;
      const who = contractedPerfect ? { player: "you" as const } : g.parsePlayerRef(c);
      if (!who) return null;
      const hasPastAuxiliary = contractedPerfect || c.isWord("didn't", "hasn't", "haven't", "did", "has", "have");
      const negate = parsePastNegation(c);
      if (c.word("cast") === null) return null;
      const what = parseHistoricalObject(c, g);
      if (what && g.purpose === "unless" && !hasPastAuxiliary)
        return c.fail("represented optional casting action");
      return what && { what, by: who, negate };
    });
    let what: ObjectRef;
    let by: PlayerRef | undefined;
    let negate: boolean;
    if (active) ({ what, by, negate } = active);
    else {
      const object = parseHistoricalObject(c, g);
      if (!object) return null;
      what = object;
      if (c.word("wasn't", "weren't") !== null) negate = true;
      else if (c.word("was", "were") !== null) negate = c.word("not") !== null;
      else return null;
      if (c.word("cast") === null) return null;
      if (c.word("by") !== null) {
        const actor = g.parsePlayerRef(c);
        if (!actor) return null;
        by = actor;
      }
    }
    let from: ZoneRef | undefined;
    const notFrom = parseExcludedOrigin(c, g.parseZoneRef);
    if (!notFrom && c.word("from") !== null) {
      const origin = g.parseZoneRef(c);
      if (!origin) return null;
      from = origin;
    }
    let when: { phase: PhaseName; whose: PlayerRef } | undefined;
    if (c.word("during") !== null) {
      const whose = parseHistoryOwner(c, g);
      if (!whose) return null;
      const phase = c.words("main", "phase") ? "main"
        : c.words("precombat", "main", "phase") || c.words("first", "main", "phase") ? "precombat-main"
        : c.words("second", "main", "phase") ? "second-main"
        : c.words("postcombat", "main", "phase") ? "postcombat-main"
        : c.word("turn") !== null ? "turn" : null;
      if (!phase) return null;
      when = { phase, whose };
    }
    return negated({ condition: "was-cast", what, ...(by ? { by } : {}), ...(from ? { from } : {}), ...(notFrom ? { notFrom } : {}), ...(when ? { when } : {}) }, negate);
  });
}

function parseZoneHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    type History = Extract<Condition, { condition: "zone-change-history" }>;
    let subject: History["subject"];
    let controllerAtMove: PlayerRef | undefined;
    let negate = false;
    // "You had a land enter the battlefield under your control" is the same
    // event as "a land entered the battlefield under your control".
    const possessor = c.attempt(c => {
      const who = g.parsePlayerRef(c);
      return who && c.word("had") !== null ? who : null;
    });
    const what = possessor ? null : parseHistoricalObject(c, g);
    if (what) subject = { match: "object", what };
    else {
      const article = c.word("a", "an") !== null;
      const another = !article && c.word("another") !== null;
      const exact = c.isWord("no", "exactly");
      let comparison: Comparison | null = article || another ? { op: "ge", value: one } : parseComparison(c, g);
      if (!comparison) return null;
      if (!article && !another && !exact && comparison.op === "eq") comparison = { op: "ge", value: comparison.value };
      const filter = g.parseFilter(c);
      if (!filter) return null;
      if (another) {
        if (filter.other) return null;
        filter.other = true;
      }
      // Past-tense control belongs to the movement, not the object's present.
      if (!possessor && c.words("you", "controlled")) controllerAtMove = { player: "you" };
      subject = { match: "filter", filter, comparison };
    }
    let event: History["event"];
    let from: History["from"];
    let to: ZoneRef | undefined;
    if (what && c.word("didn't") !== null) negate = true;
    else if (!possessor) c.word("has", "have");
    if (possessor) {
      if (!c.words("enter", "the", "battlefield", "under") ||
          c.word(possessor.player === "you" ? "your" : "their") === null || c.word("control") === null) return null;
      event = "enters";
      to = { zone: "battlefield" };
      controllerAtMove = possessor;
    } else if (negate) {
      if (c.word("die") !== null) { event = "dies"; from = { zone: "battlefield" }; to = { zone: "graveyard" }; }
      else if (c.words("enter", "the", "battlefield")) { event = "enters"; to = { zone: "battlefield" }; }
      else if (c.words("leave", "the", "battlefield")) { event = "leaves-battlefield"; from = { zone: "battlefield" }; }
      else return null;
    } else if (c.word("died") !== null) {
      event = "dies";
      from = { zone: "battlefield" }; to = { zone: "graveyard" };
    } else if (c.word("entered") !== null) {
      event = "enters";
      c.words("the", "battlefield") || c.word("battlefield");
      to = { zone: "battlefield" };
    } else if (c.word("left") !== null) {
      if (c.words("the", "battlefield")) {
        event = "leaves-battlefield";
        from = { zone: "battlefield" };
      } else {
        // CR 603.10a: leaving a graveyard is its own looked-back event.
        const origin = g.parseZoneRef(c);
        if (!origin || origin.position || origin.zone === "battlefield" ||
            (["hand", "library", "graveyard"].includes(origin.zone) && origin.owner === undefined)) return null;
        event = "leaves-zone";
        from = origin;
      }
    } else if (c.word("was", "were") !== null && c.words("put", "into")) {
      event = "put-into-zone";
      const destination = g.parseZoneRef(c);
      if (!destination) return null;
      to = destination;
    } else return null;
    if (c.word("from") !== null) {
      if (from) return null;
      const origin = c.word("anywhere") !== null ? "anywhere" : g.parseZoneRef(c);
      if (!origin) return null;
      from = origin;
    }
    if (!possessor && c.word("under") !== null) {
      const controller = parseHistoryOwner(c, g);
      if (controllerAtMove || !controller || c.word("control") === null) return null;
      controllerAtMove = controller;
    }
    const during = parseLookbackPeriod(c);
    return during && negated({ condition: "zone-change-history", event, subject, ...(from ? { from } : {}), ...(to ? { to } : {}),
      ...(controllerAtMove ? { controllerAtMove } : {}), during }, negate);
  });
}

/** CR 700.11: a player descended if a permanent card was put into that
 * player's graveyard from anywhere this turn; it need not still be there. */
function parseDescended(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    let negate = false;
    if (c.word("you've") === null) {
      if (c.word("you") === null) return null;
      negate = c.word("haven't") !== null;
    }
    if (c.word("descended") === null || !c.words("this", "turn")) return null;
    return negated({ condition: "zone-change-history", event: "put-into-zone",
      subject: { match: "filter", filter: { cls: "card", permanent: true }, comparison: { op: "ge", value: one } },
      from: "anywhere", to: { zone: "graveyard", owner: "your" }, during: "this-turn" }, negate);
  });
}

/** "No creatures attacked" counts declared attackers of every player in
 * the window (CR 508.1); see combat-history-count for what is counted. */
function parseNoAttackers(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    if (!c.words("no", "creatures", "attacked")) return null;
    const during = parseHistoryPeriod(c);
    return during && { condition: "compare", value: { amount: "combat-history-count", action: "attacked", during,
      distinct: "object-incarnations" }, comparison: { op: "eq", value: zero } };
  });
}

function parseLifeHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const subject = parseHistorySubject(c, g);
    if (!subject) return null;
    const verb = c.word("gained", "lost", ...(subject.negate ? ["gain", "lose"] : []));
    const change = verb === "gain" ? "gained" : verb === "lose" ? "lost" : verb;
    if (change !== "gained" && change !== "lost") return null;
    let alternative: "gained" | "lost" | undefined;
    const join = c.word("or", "and");
    if (join !== null) {
      const other = c.word("gained", "lost");
      if ((other !== "gained" && other !== "lost") || other === change) return null;
      // "Didn't gain and lose" has no single reading; only positive "and".
      if (join === "and" && subject.negate) return null;
      alternative = other;
    }
    const comparison = c.isWord("life") ? { op: "gt" as const, value: zero } : parseComparison(c, g);
    if (!comparison || c.word("life") === null) return null;
    const during = c.words("this", "turn") ? "this-turn" as const : c.words("last", "turn") ? "last-turn" as const : null;
    if (!during) return null;
    const condition: Condition = { condition: "life-change", who: subject.who, change, comparison, during };
    return negated(alternative ? { condition: join === "and" ? "all-of" : "any-of", conditions: [condition,
      { condition: "life-change", who: subject.who, change: alternative, comparison, during },
    ] } : condition, subject.negate);
  });
}

/** "You've paid or lost four or more {E} this turn" (CR 107.14): the energy
 * counters paid or lost, compared as a number. */
function parseEnergyHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    if (!c.words("you've", "paid", "or", "lost")) return null;
    const comparison = parseComparison(c, g);
    const symbol = c.peek();
    if (!comparison || symbol?.kind !== "symbol" || symbol.value !== "E") return null;
    c.pos++;
    if (!c.words("this", "turn")) return null;
    return { condition: "compare", value: { amount: "energy-paid-or-lost", who: { player: "you" }, during: "this-turn" }, comparison };
  });
}

function parseCastHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const passive = c.attempt((c): Condition | null => {
      const exact = c.isWord("no", "exactly");
      // "a spell was warped this turn": at least one such spell (CR 702.185c).
      const comparison = c.word("a") !== null ? { op: "ge" as const, value: one } : parseComparison(c, g);
      if (comparison?.op === "eq" && !exact) return null;
      const what = comparison && g.parseFilter(c);
      if (!comparison || !what || what.cls !== "spell" || hasFilterZone(what) || c.word("was", "were") === null) return null;
      // CR 702.185c: "a spell was warped this turn" — cast for its warp cost.
      const verb = c.word("cast", "warped");
      if (!verb) return null;
      const during = c.words("this", "turn") ? "this-turn" : c.words("last", "turn") ? "last-turn" : null;
      return during && { condition: "cast-count", who: "all-players-combined",
        what: verb === "warped" ? { ...what, castForKeywordCost: "warp" } : what, comparison, during };
    });
    if (passive) return passive;
    const subject = parseHistorySubject(c, g);
    if (!subject || c.word("cast") === null) return null;
    const explicit = c.isWord("no", "exactly", "at", "more", "fewer");
    let comparison = parseComparison(c, g);
    if (!comparison) comparison = { op: "ge", value: one };
    else if (comparison.op === "eq" && !explicit && !(comparison.value.amount === "fixed" && comparison.value.value === 0))
      comparison = { op: "ge", value: comparison.value };
    const filterStart = c.pos;
    const filter = g.parseFilter(c);
    if (!filter || filter.cls !== "spell") return null;
    // A spell cast "from your hand" has a historical origin, not a current
    // zone requirement. Preserve it separately from spell characteristics.
    let what = filter;
    let from: ZoneRef | undefined;
    if (hasFilterZone(filter)) {
      if (!filter.zone || filter.anyOf?.some(hasFilterZone) || filter.allOf?.some(hasFilterZone)) return null;
      // As with cast-event headers, only one complete trailing origin is
      // lifted out. Two source clauses must not become one historical origin
      // plus a current-zone predicate on the spell already on the stack.
      const tokens = c.tokens.slice(filterStart, c.pos);
      const origins = tokens.flatMap((token, index) => token.kind === "word" && !token.possessive && token.value === "from" ? [index] : []);
      if (origins.length !== 1) return null;
      const source = new Cursor(tokens.slice(origins[0] + 1));
      const origin = g.parseZoneRef(source);
      if (!origin || !source.done() || JSON.stringify(origin) !== JSON.stringify(filter.zone)) return null;
      const { zone, ...rest } = filter;
      from = zone;
      what = rest;
    }
    const during = c.words("this", "turn") ? "this-turn" : c.words("last", "turn") ? "last-turn" : null;
    if (!during) return null;
    return negated({ condition: "cast-count", who: subject.who, what, comparison, during, ...(from ? { from } : {}) }, subject.negate);
  });
}

/** "You've cast both a creature spell and a noncreature spell this turn":
 * one spell of each kind. Only kinds that no single spell can both match
 * (a card type and its negation) make two independent counts exact. */
function parseCastPair(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    if (c.word("you've") === null || c.word("cast") === null) return null;
    c.word("both");
    const kinds: ObjectFilter[] = [];
    for (const join of [null, "and"] as const) {
      if (join && c.word(join) === null) return null;
      if (c.word("a", "an") === null) return null;
      const what = g.parseFilter(c);
      if (!what || what.cls !== "spell" || hasFilterZone(what)) return null;
      kinds.push(what);
    }
    const [first, second] = kinds;
    const disjoint = first.types?.some(type => second.nonTypes?.includes(type)) || second.types?.some(type => first.nonTypes?.includes(type));
    if (!disjoint || !c.words("this", "turn")) return null;
    return { condition: "all-of", conditions: kinds.map(what =>
      ({ condition: "cast-count", who: { player: "you" }, what, comparison: { op: "ge", value: one }, during: "this-turn" })) };
  });
}

function hasFilterZone(filter: ObjectFilter): boolean {
  return !!filter.zone || !!filter.anyOf?.some(hasFilterZone) || !!filter.allOf?.some(hasFilterZone);
}

/** Domain/delirium count the union of represented characteristics. Corrupted
 * instead tests a specific player's counters (CR 205.2b, 305.6, 122.1). */
function parseAggregateCondition(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    if (c.words("there", "are") || c.words("there", "is") || c.word("there's") !== null) {
      const comparison = parseComparison(c, g);
      if (!comparison) return null;
      const value = parseAggregateCount(c, g);
      return value ? { condition: "compare", value, comparison } : null;
    }
    const among = g.parseObjectRef(c);
    if (!among || c.word("have", "has") === null) return null;
    c.word("a");
    if (c.word("total") === null) return null;
    const attribute = c.words("mana", "value") ? "mana-value" : c.word("power", "toughness");
    if (attribute !== "mana-value" && attribute !== "power" && attribute !== "toughness") return null;
    const comparison = parseComparison(c, g);
    return comparison ? { condition: "compare", value: { amount: "attribute-sum", attribute, among }, comparison } : null;
  });
}

function parsePlayerCounters(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const who = g.parsePlayerRef(c);
    if (!who || c.word("have", "has") === null) return null;
    const comparison = parseComparison(c, g);
    if (!comparison) return null;
    const counter = c.isWord("counter", "counters") ? "any" : g.parseCounterSpec(c);
    if (!counter || c.word("counter", "counters") === null) return null;
    return { condition: "player-counters", who, counter, comparison };
  });
}

function parseAtom(c: Cursor, g: ConditionGrammar): Condition | null {
  const currentPhase = parseCurrentPhaseCondition(c);
  if (currentPhase) return currentPhase;
  // Fixed reference players reuse the ordinary count/life/hand conditions with
  // the other player's measurement as the compared value; quantified reference
  // players ("than each other player") need player-comparison below.
  const comparative = parseComparativeCondition(c, g);
  if (comparative) return comparative;
  const own = parseDesignationCondition(c, g) ?? parseLinkedCondition(c, g) ?? parsePlayerCondition(c, g)
    ?? parseDescended(c) ?? parseNoAttackers(c) ?? parseEventCondition(c, g) ?? parseCastOrdinal(c, g) ?? parseFirstEvent(c, g);
  if (own) return own;
  const paired = parsePairedCondition(c, g) ?? parseMostCommonColorCondition(c, g);
  if (paired) return paired;
  const subset = parseControlledSubset(c, g);
  if (subset) return subset;
  const dice = parseDiceCondition(c, g);
  if (dice) return dice;
  const aggregate = parseAggregateCondition(c, g) ?? parsePlayerCounters(c, g);
  if (aggregate) return aggregate;
  return parsePutActionOutcome(c, g) ?? parseEventOutcome(c, g) ?? parseAttributeComparison(c, g) ?? parseGraveyardPosition(c, g) ?? parseCharacteristicRelationCondition(c, g) ?? parseSearchHistory(c, g) ?? parseZoneState(c, g) ?? parseKicked(c, g) ?? parseHandCount(c, g) ?? parsePersonalZoneCount(c, g) ?? parseOwnedInExile(c, g) ?? parseZoneSize(c, g) ?? parseCopularZoneCount(c, g) ?? parseQualifiedCount(c, g)
    ?? parseHasKeyword(c, g) ?? parseManaSpent(c, g) ?? parseDamageHistory(c, g)
    ?? parseCombatHistory(c, g) ?? parseCastObjectHistory(c, g) ?? parseZoneHistory(c, g)
    ?? parseLifeHistory(c, g) ?? parseEnergyHistory(c, g) ?? parseCastPair(c, g) ?? parseCastHistory(c, g) ?? g.parseAtomic(c);
}

/** Repeated complete clauses can form an unambiguous boolean expression.
 * Object-filter alternatives remain inside parseFilter. Mixed AND/OR clauses
 * without an explicit grouping syntax fail instead of inventing precedence. */
export function parseConditionExpression(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const first = parseAtom(c, g);
    if (!first) return null;
    const conditions = [first];
    let operator: "and" | "or" | undefined;
    for (;;) {
      const next = c.attempt((c): { join: "and" | "or"; condition: Condition } | null => {
        const join = c.word("and", "or");
        if (join !== "and" && join !== "or") return null;
        c.word("if");
        const condition = parseAtom(c, g);
        return condition && { join, condition };
      });
      if (!next) break;
      if (operator && operator !== next.join) return c.fail("unambiguous condition grouping");
      operator = next.join;
      conditions.push(next.condition);
    }
    return conditions.length === 1 ? first : { condition: operator === "and" ? "all-of" : "any-of", conditions };
  });
}
