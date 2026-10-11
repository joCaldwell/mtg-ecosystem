// Structured condition expressions. Reference parsers are passed in so refs.ts
// can delegate here without a runtime import cycle.
// CR 603.4 (intervening conditions), 702.33d–g (kicked and linked costs),
// 601.2i (casting events), 400.7d (originating spell), 508.1/508.4/509.1
// (declared combat actions), 608.2i (history), 700.4 (dies), and 119:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

import type { Amount, Comparison, Condition, Cost, CounterSpec, ObjectFilter, ObjectRef, PhaseName, PlayerRef, ZoneRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { isManaSymbol } from "../symbols.ts";
import { parseKeywordName } from "./keywords.ts";
import { parseDiceCondition } from "./dice.ts";
import { parseAggregateCount } from "./aggregates.ts";
import { parseControlledSubset } from "./selection-constraints.ts";
import { parseCharacteristicRelationCondition } from "./characteristic-relations.ts";
import { parseCurrentPhaseCondition } from "./temporal.ts";
import { parsePutActionOutcome } from "./action-outcomes.ts";

export interface ConditionGrammar {
  /** In an unless clause, bare present-tense actions describe optional
   * payments now (CR 118.12a), not a historical condition. */
  purpose?: "unless";
  parseAmount(c: Cursor): Amount | null;
  parseComparison(c: Cursor): Comparison | null;
  parseCount(c: Cursor): Amount | null;
  parseCounterSpec(c: Cursor): CounterSpec | null;
  parseFilter(c: Cursor): ObjectFilter | null;
  parseObjectRef(c: Cursor): ObjectRef | null;
  parsePlayerRef(c: Cursor): PlayerRef | null;
  parsePossessivePlayer?(c: Cursor): PlayerRef | null;
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
    return negated({ condition: "is-filter", what, filter: { zone } }, negate);
  });
}

/** Current cards in a personal zone (CR 400, 401, 402, 404), not a count of
 * cards drawn/discarded earlier. Keep the stated player as the zone owner. */
function parsePersonalZoneCount(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const who = g.parsePlayerRef(c);
    if (!who || !["you", "that-player", "they", "controller", "owner", "target-player", "target-opponent"].includes(who.player) ||
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

function hasZone(filter: ObjectFilter): boolean {
  return !!filter.zone || !!filter.allOf?.some(hasZone) || !!filter.anyOf?.some(hasZone);
}

/** "Twenty or more creature cards are in your graveyard" explicitly counts
 * the current zone contents. The copula cannot be discarded as filler in
 * historical clauses such as "were put into" or "have left". */
function parseCopularZoneCount(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const comparison = parseComparison(c, g);
    const of = comparison && g.parseFilter(c);
    if (!of || !comparison || hasZone(of) || c.word("are", "is") === null || c.word("in") === null) return null;
    const zone = g.parseZoneRef(c);
    if (!zone || zone.position || (["hand", "library", "graveyard"].includes(zone.zone) && zone.owner === undefined)) return null;
    return { condition: "count", of: { ...of, zone }, comparison };
  });
}

function parseQualifiedCount(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    if (c.words("there", "are") || c.words("there", "is")) {
      const comparison = parseComparison(c, g);
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
    const player = c.attempt((c): Condition | null => {
      const who = c.word("you've") !== null ? { player: "you" as const } : g.parsePlayerRef(c);
      if (!who) return null;
      const negate = parsePastNegation(c);
      if (c.word("attacked", ...(negate ? ["attack"] : [])) === null) return null;
      let withFilter: ObjectFilter | undefined;
      if (c.word("with") !== null) {
        c.word("a", "an");
        const filter = g.parseFilter(c);
        if (!filter) return null;
        withFilter = filter;
      }
      const during = parseHistoryPeriod(c);
      return during && negated({ condition: "attacked", who, during, ...(withFilter ? { with: withFilter } : {}) }, negate);
    });
    if (player) return player;
    const what = parseHistoricalObject(c, g);
    if (!what) return null;
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
    const during = parseHistoryPeriod(c);
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
      const mana: ManaHistory["mana"] = type as Exclude<ManaHistory["mana"], "any"> | null ?? "any";
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
  comparison: Comparison; quantity: "positive-event" | "total"; damage?: "combat" | "noncombat";
} | null {
  return c.attempt(c => {
    const comparison = parseComparison(c, g);
    const damage = c.word("combat", "noncombat") as "combat" | "noncombat" | null;
    if (c.word("damage") === null) return null;
    return { comparison: comparison ?? { op: "gt", value: zero }, quantity: comparison ? "total" : "positive-event",
      ...(damage ? { damage } : {}) };
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
    for (let part = 0; part < 2; part++) {
      if (!during && c.isWord("this")) during = parseHistoryPeriod(c);
      else if (!source && c.word("by") !== null) {
        const object = g.parseObjectRef(c);
        if (!object || object.ref === "each" || object.ref === "them" ||
          (object.ref === "filter" && (object.count?.amount !== "fixed" || object.count.value !== 1))) return null;
        source = object;
      } else break;
    }
    if (!during) return null;
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
    if (c.word("from") !== null) {
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
    return negated({ condition: "was-cast", what, ...(by ? { by } : {}), ...(from ? { from } : {}), ...(when ? { when } : {}) }, negate);
  });
}

function parseZoneHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    type History = Extract<Condition, { condition: "zone-change-history" }>;
    let subject: History["subject"];
    const what = parseHistoricalObject(c, g);
    if (what) subject = { match: "object", what };
    else {
      const article = c.word("a", "an") !== null;
      const exact = c.isWord("no", "exactly");
      let comparison: Comparison | null = article ? { op: "ge", value: one } : parseComparison(c, g);
      if (!comparison) return null;
      if (!article && !exact && comparison.op === "eq") comparison = { op: "ge", value: comparison.value };
      const filter = g.parseFilter(c);
      if (!filter) return null;
      subject = { match: "filter", filter, comparison };
    }
    c.word("has", "have");
    let event: History["event"];
    let from: History["from"];
    let to: ZoneRef | undefined;
    if (c.word("died") !== null) {
      event = "dies";
      from = { zone: "battlefield" }; to = { zone: "graveyard" };
    } else if (c.word("entered") !== null) {
      event = "enters";
      c.words("the", "battlefield") || c.word("battlefield");
      to = { zone: "battlefield" };
    } else if (c.word("left") !== null) {
      event = "leaves-battlefield";
      if (!c.words("the", "battlefield")) return null;
      from = { zone: "battlefield" };
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
    let controllerAtMove: PlayerRef | undefined;
    if (c.word("under") !== null) {
      const controller = parseHistoryOwner(c, g);
      if (!controller || c.word("control") === null) return null;
      controllerAtMove = controller;
    }
    const during = parseHistoryPeriod(c);
    return during && { condition: "zone-change-history", event, subject, ...(from ? { from } : {}), ...(to ? { to } : {}),
      ...(controllerAtMove ? { controllerAtMove } : {}), during };
  });
}

function parseLifeHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const subject = parseHistorySubject(c, g);
    if (!subject) return null;
    const change = c.word("gained", "lost");
    if (change !== "gained" && change !== "lost") return null;
    let alternative: "gained" | "lost" | undefined;
    if (c.word("or") !== null) {
      const other = c.word("gained", "lost");
      if (other !== "gained" && other !== "lost") return null;
      alternative = other;
    }
    const comparison = c.isWord("life") ? { op: "gt" as const, value: zero } : parseComparison(c, g);
    if (!comparison || c.word("life") === null || !c.words("this", "turn")) return null;
    const condition: Condition = { condition: "life-change", who: subject.who, change, comparison, during: "this-turn" };
    return negated(alternative ? { condition: "any-of", conditions: [condition,
      { condition: "life-change", who: subject.who, change: alternative, comparison, during: "this-turn" },
    ] } : condition, subject.negate);
  });
}

function parseCastHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const passive = c.attempt((c): Condition | null => {
      const exact = c.isWord("no", "exactly");
      const comparison = parseComparison(c, g);
      if (comparison?.op === "eq" && !exact) return null;
      const what = comparison && g.parseFilter(c);
      if (!comparison || !what || what.cls !== "spell" || hasFilterZone(what) ||
          c.word("was", "were") === null || c.word("cast") === null) return null;
      const during = c.words("this", "turn") ? "this-turn" : c.words("last", "turn") ? "last-turn" : null;
      return during && { condition: "cast-count", who: "all-players-combined", what, comparison, during };
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

function hasFilterZone(filter: ObjectFilter): boolean {
  return !!filter.zone || !!filter.anyOf?.some(hasFilterZone) || !!filter.allOf?.some(hasFilterZone);
}

/** Domain/delirium count the union of represented characteristics. Corrupted
 * instead tests a specific player's counters (CR 205.2b, 305.6, 122.1). */
function parseAggregateCondition(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    if (c.words("there", "are") || c.words("there", "is")) {
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
  const subset = parseControlledSubset(c, g);
  if (subset) return subset;
  const dice = parseDiceCondition(c, g);
  if (dice) return dice;
  const aggregate = parseAggregateCondition(c, g) ?? parsePlayerCounters(c, g);
  if (aggregate) return aggregate;
  return parsePutActionOutcome(c, g) ?? parseCharacteristicRelationCondition(c, g) ?? parseSearchHistory(c, g) ?? parseZoneState(c, g) ?? parseKicked(c, g) ?? parseHandCount(c, g) ?? parsePersonalZoneCount(c, g) ?? parseCopularZoneCount(c, g) ?? parseQualifiedCount(c, g)
    ?? parseHasKeyword(c, g) ?? parseManaSpent(c, g) ?? parseDamageHistory(c, g)
    ?? parseCombatHistory(c, g) ?? parseCastObjectHistory(c, g) ?? parseZoneHistory(c, g)
    ?? parseLifeHistory(c, g) ?? parseCastHistory(c, g) ?? g.parseAtomic(c);
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
