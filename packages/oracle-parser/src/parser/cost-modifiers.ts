// CR 118.7a-g preserves generic, colored, colorless and hybrid reductions;
// 601.2f applies increases, reductions, and minimums to the total payment.
// These effects never mutate a card's printed mana cost or mana value.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Condition, CostModification, CostModificationOccurrence, CostModificationScope, Duration, Effect, PlayerRef, StaticEffect, ZoneRef } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { isManaSymbol } from "../symbols.ts";
import { Cursor } from "./cursor.ts";
import { isPluralPlayerRef, isTargetedPlayerRef, parseAmount, parseCondition, parseCounterSpec, parseDuration, parseObjectRef, parsePlayerRef, parsePossessivePlayer, parseZoneRef } from "./refs.ts";
import { parseCommanderCastUnit, parseEnergySpentUnit, parseHistoryUnit, parseTimesCastUnit } from "./history-amounts.ts";
import { parseUnitQuantity } from "./quantities.ts";
import { parseAttackedPlayerUnit, parseOpponentUnit } from "./player-counts.ts";
import { parseTargetedUnit, parseTargetedValue } from "./cost-target-units.ts";
import { parseEligibleCardRef } from "./permissions.ts";
import { hasCombatHistory, parseAggregateCount } from "./aggregates.ts";

type Modifier = Extract<StaticEffect, { static: "cost-modification" }>;

function parseActor(c: Cursor): PlayerRef | null {
  return c.attempt(c => {
    if (c.words("your", "opponents") || c.word("opponents") !== null) return { player: "each-opponent" };
    if (c.words("a", "player") || c.word("players") !== null) return { player: "each-player" };
    return parsePlayerRef(c);
  });
}

/** Remove one explicit caster/activator clause while leaving all object
 * qualifiers in their original order. "From your graveyard" describes the
 * casting origin and must not become a spell-currently-in-graveyard filter. */
function extractActor(tokens: Token[], action: "cast" | "activate"): { tokens: Token[]; actor?: PlayerRef; from?: CastOrigin; notFrom?: ZoneRef;
  notOwned?: true; yourTurn?: true } | null {
  let result = [...tokens];
  let actor: PlayerRef | undefined;
  let from: CastOrigin | undefined;
  let notFrom: ZoneRef | undefined;
  let notOwned = false, yourTurn = false;
  for (let index = 0; index < result.length; index++) {
    const c = new Cursor(result.slice(index));
    const candidate = parseActor(c);
    if (!candidate || c.word(action, action === "cast" ? "casts" : "activates") === null) continue;
    if (actor || isTargetedPlayerRef(candidate)) return null;
    actor = candidate;
    if (action === "cast" && c.word("from") !== null) {
      // "From anywhere other than your hand" excludes one origin; it is not
      // a list of the remaining zones (Sage of the Beyond).
      const excluded = c.words("anywhere", "other", "than");
      const origin = parseZoneRef(c);
      if (!origin) return null;
      if (excluded) notFrom = origin;
      else {
        // "From graveyards or from exile": a union of origins.
        const origins = [origin];
        for (let more = c.attempt(c => c.words("or", "from") ? parseZoneRef(c) : null); more; more = c.attempt(c => c.words("or", "from") ? parseZoneRef(c) : null))
          origins.push(more);
        from = origins.length > 1 ? { anyOf: origins } : origin;
      }
    }
    // "Spells you cast but don't own": ownership of the spell, by the caster.
    if (action === "cast" && "player" in candidate && candidate.player === "you" && c.attempt(c => c.words("but", "don't", "own") ? true : null)) notOwned = true;
    // The spell is cast during the stated turn (CR 500.1): the same as a
    // leading "During your turn,".
    if (action === "cast" && c.attempt(c => c.words("during", "your", "turn") ? true : null)) yourTurn = true;
    result = [...result.slice(0, index), ...result.slice(index + c.pos)];
    index--;
  }
  return { tokens: result, ...(actor ? { actor } : {}), ...(from ? { from } : {}), ...(notFrom ? { notFrom } : {}),
    ...(notOwned ? { notOwned: true as const } : {}), ...(yourTurn ? { yourTurn: true as const } : {}) };
}

type CastOrigin = ZoneRef | { anyOf: ZoneRef[] };

const ORDINALS: Record<string, number> = { first: 1, second: 2, third: 3 };

/** "The first creature spell you cast each turn" limits the adjustment to one
 * ordinal among that actor's matching casts or activations in a turn. "The
 * next ..." exists only for a resolving effect, which it uses up. */
function extractOccurrence(tokens: Token[], resolving: boolean): { tokens: Token[]; occurrence?: CostModificationOccurrence; yourTurn?: true } | null {
  const [article, marker] = tokens;
  if (article?.kind !== "word" || article.value !== "the" || marker?.kind !== "word" || marker.possessive) return { tokens };
  if (marker.value === "next") return resolving ? { tokens: tokens.slice(2), occurrence: { next: true } } : null;
  const ordinal = ORDINALS[marker.value];
  if (!ordinal) return { tokens };
  // "... you cast during each of your turns": the ordinal is counted within
  // each turn and only turns of that player count (a condition on the turn).
  const ownTurns = tokens.slice(-5).map(token => token.kind === "word" && !token.possessive ? token.value : "").join(" ") === "during each of your turns";
  if (ownTurns && !resolving) return { tokens: tokens.slice(2, -5), occurrence: { ordinal, period: "turn" }, yourTurn: true };
  const each = tokens[tokens.length - 2], turn = tokens[tokens.length - 1];
  if (resolving || each?.kind !== "word" || each.value !== "each" || turn?.kind !== "word" || turn.value !== "turn") return null;
  return { tokens: tokens.slice(2, -2), occurrence: { ordinal, period: "turn" } };
}

/** A keyword's own cost can be an activation, alternative, additional, or
 * special-action cost (CR 118.8–118.9, 116.2b/m). Equip costs are activation
 * costs of equip abilities (702.6a), so they keep the activation scope. */
function parseKeywordCostScope(tokens: Token[]): CostModificationScope | null {
  const c = new Cursor(tokens);
  const all = c.word("all") !== null;
  const cost = c.word("equip", "flashback", "buyback", "dash", "blitz", "morph", "unlock");
  if (!cost || c.word("costs") === null) return null;
  let payer: PlayerRef | undefined;
  if (!c.done()) {
    const who = all ? null : parseActor(c);
    if (!who || isTargetedPlayerRef(who) || c.word("pay", "pays") === null || !c.done()) return null;
    payer = who;
  }
  if (cost === "equip") return { action: "activate", ...(payer ? { activator: payer } : {}), keyword: "equip",
    abilities: { ref: "each", filter: { cls: "ability" } } };
  return { action: "pay-cost", cost: cost as "flashback" | "buyback" | "dash" | "blitz" | "morph" | "unlock", ...(payer ? { payer } : {}) };
}

/** The same cast scope for a static grant to the ordinal spell of a turn. */
export function parseCastScope(tokens: Token[]): Extract<CostModificationScope, { action: "cast" }> | null {
  const scope = parseScope(tokens, "cast");
  return scope?.action === "cast" ? scope : null;
}

/** The same activation scope for an alternative activation cost. A caller
 * passing `turn` accepts "during each of your turns" as a your-turn condition. */
export function parseActivationScope(tokens: Token[], turn?: { yourTurn?: true }): Extract<CostModificationScope, { action: "activate" }> | null {
  const scope = parseScope(tokens, "activate", false, turn);
  return scope?.action === "activate" ? scope : null;
}

function parseScope(tokens: Token[], action: "cast" | "activate", resolving = false, turn?: { yourTurn?: true }): CostModificationScope | null {
  const ordered = extractOccurrence(tokens, resolving);
  const extracted = ordered && extractActor(ordered.tokens, action);
  if (!ordered || !extracted) return null;
  // An ordinal or "next" is counted among one stated actor's own actions.
  if (ordered.occurrence && !extracted.actor) return null;
  const occurrence = ordered.occurrence ? { occurrence: ordered.occurrence } : {};
  const c = new Cursor(extracted.tokens);
  if (extracted.yourTurn || ordered.yourTurn) {
    if (extracted.yourTurn && ordered.yourTurn) return null;
    // Reported to the caller as a condition on the adjustment; callers that
    // cannot carry one reject the scope.
    if (!turn) return null;
    turn.yourTurn = true;
  }
  if (action === "cast") {
    let spells = parseEligibleCardRef(c);
    if (!spells || !c.done() || spells.ref === "target") return null;
    if (extracted.notOwned) {
      if (spells.ref !== "each" || spells.filter.own) return null;
      spells = { ...spells, filter: { ...spells.filter, own: "not-you" } };
    }
    if (ordered.occurrence && spells.ref !== "each") return null;
    return { action: "cast", spells, ...(extracted.actor ? { caster: extracted.actor } : {}), ...(extracted.from ? { from: extracted.from } : {}),
      ...(extracted.notFrom ? { notFrom: extracted.notFrom } : {}), ...occurrence };
  }
  const scope: Extract<CostModificationScope, { action: "activate" }> = {
    action: "activate", ...(extracted.actor ? { activator: extracted.actor } : {}), ...occurrence,
  };
  // These names identify the activated ability, not a keyword possessed
  // by its source or by another ability (CR 702.6/.29/.49/.142/.177/.193).
  // Loyalty is an ability category (CR 606.2), never a keyword ability.
  const attached = c.peek()?.kind === "word" && (c.isWord("enchanted", "equipped"));
  const possessed = c.attempt(c => {
    const determiner = c.word("this", "enchanted", "equipped");
    if (determiner === null) return null;
    const noun = c.next();
    if (noun?.kind !== "word" || !noun.possessive) return null;
    const local = new Cursor([{ kind: "word", value: determiner, raw: determiner, possessive: false } as Token, { ...noun, possessive: false }]);
    const source = parseObjectRef(local);
    return source && local.done() ? source : null;
  });
  if (possessed) scope.source = possessed;
  const keyword = c.word("equip", "boast", "cycling", "ninjutsu", "exhaust", "power-up");
  if (keyword === "equip" || keyword === "boast" || keyword === "cycling" || keyword === "ninjutsu" ||
      keyword === "exhaust" || keyword === "power-up") scope.keyword = keyword;
  // "Enchanted artifact's activated abilities cost {2} less": the attached
  // permanent is the source of every activated ability the scope covers.
  else if (possessed && !(attached && c.isWord("activated"))) return null;
  else if (c.word("loyalty") !== null) scope.loyalty = true;
  else c.word("activated");
  if (!c.isWord("ability", "abilities")) return null;
  let abilities = parseObjectRef(c);
  if (!abilities) return null;
  // Shared refs now consumes "abilities of <source>". Keep that relation in
  // the activation scope, where keyword/loyalty categories are already stored;
  // do not drop it or silently override a possessive source declaration.
  if ("filter" in abilities && abilities.filter.abilitySource) {
    if (scope.source) return null;
    const { abilitySource, ...filter } = abilities.filter;
    scope.source = abilitySource;
    abilities = { ...abilities, filter };
  }
  if (!scope.keyword && !scope.loyalty && !possessed && abilities.ref === "each" && "filter" in abilities &&
      abilities.filter.cls === "ability" && Object.keys(abilities.filter).length === 1) {
    // The action discriminant already denotes all activated abilities.
  } else scope.abilities = abilities;
  if (c.word("of") !== null) {
    if (scope.source) return null;
    const source = parseEligibleCardRef(c);
    if (!source) return null;
    scope.source = source;
  }
  return c.done() ? scope : null;
}

type TargetCountOwner = Extract<Amount, { amount: "target-count" }>["of"];

/** Whose targets "for each target" counts: the one spell or ability the
 * sentence names, or each spell/ability of the group it adjusts. */
function targetCountOwner(scope: CostModificationScope): TargetCountOwner {
  const named = scope.action === "cast" ? scope.spells : scope.action === "activate" ? scope.abilities : undefined;
  return named?.ref === "this" ? named : { ref: "affected-object" };
}

/** A multiplier explicitly distinguishes objects, counters, and represented
 * types/colors. Explicit
 * repeated "each" groups add their cardinalities; a nominal eligibility
 * union counts each matching object once (Bedlam Reveler vs Gargantuan Leech).
 * Counter holders may be an entire specified set (Deepwood Denizen, CR 122.1). */
function parseEachMultiplier(c: Cursor, payee: TargetCountOwner, ownCost = false): Amount | null {
  return c.attempt(c => {
    // CR 601.2c/f: the targets announced for the spell or ability being paid
    // for. "Each target creature ..." is an object selection, never this.
    const targets = c.attempt((c): Amount | null => {
      if (!c.words("each", "target")) return null;
      const beyond = c.words("beyond", "the", "first");
      if (!c.done() && !c.isPunct(".")) return null;
      return { amount: "target-count", of: payee, ...(beyond ? { beyond: 1 } : {}) };
    });
    if (targets) return targets;
    // CR 700.8a: the game computes the size of a party (zero to four); it is
    // not a count of the Clerics, Rogues, Warriors, and Wizards you control.
    const party = c.attempt((c): Amount | null => c.words("each", "creature", "in", "your", "party")
      ? { amount: "party-size", who: { player: "you" } } : null);
    const aggregate = party ?? c.attempt(c => c.word("each") !== null
      ? parseAggregateCount(c, { parseObjectRef }, { singular: true }) : null);
    const counterCount = aggregate ? null : c.attempt((c): Amount | null => {
      if (c.word("each") === null || c.isWord("different", "kind", "type", "color")) return null;
      const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
      if (!counter || c.word("counter", "counters") === null) return null;
      if (c.words("you", "have")) return { amount: "counter-count", counter, on: { player: "you" } };
      // "for each counter removed this way": the counters an additional cost
      // of this same spell removed (CR 118.8, 601.2f).
      if (ownCost && c.words("removed", "this", "way")) return { amount: "event-quantity", event: "counter-removal", counter, scope: "this-way" };
      // "Each counter among <group>": one unit per counter over the whole
      // group (CR 122.1); counters sit on permanents and on players.
      if (c.word("among") !== null) {
        if (c.words("players", "and", "permanents")) return { amount: "plus", a: { amount: "counter-count", counter, on: { player: "each-player" }, total: true },
          b: { amount: "counter-count", counter, on: { ref: "each", filter: { cls: "permanent" } } } };
        const among = parseObjectRef(c);
        return among && (among.ref === "each" || among.ref === "all") ? { amount: "counter-count", counter, on: among } : null;
      }
      if (c.word("on") === null) return null;
      const on = parsePlayerRef(c) ?? parseObjectRef(c);
      return on ? { amount: "counter-count", counter, on } : null;
    });
    // Units that are not current objects: what players did this turn, life
    // gained or lost (one unit per 1 life), and the number of opponents.
    const history = aggregate || counterCount ? null : c.attempt((c): Amount | null => {
      if (c.word("each") === null) return null;
      if (c.words("opponent", "you", "have")) return { amount: "opponent-count", who: { player: "you" } };
      return parseAttackedPlayerUnit(c) ?? parseOpponentUnit(c, { parsePlayerRef, isPluralPlayerRef }) ?? parseHistoryUnit(c) ?? parseCommanderCastUnit(c) ?? parseEnergySpentUnit(c) ?? parseTimesCastUnit(c, false, { ref: "affected-object" }) ?? parseUnitQuantity(c, { parseObjectRef, parsePlayerRef, parsePossessivePlayer, isPluralPlayerRef });
    });
    const each = aggregate || counterCount || history ? null : parseEligibleCardRef(c, { distributive: true });
    let result: Amount;
    if (aggregate) result = aggregate;
    else if (counterCount) result = counterCount;
    else if (history) result = history;
    else if (each?.ref === "each" && !hasCombatHistory(each.filter)) result = { amount: "count", of: each.filter };
    else return null;
    if (c.attempt(c => c.word("and") !== null && c.isWord("each") ? true : null)) {
      const next = parseEachMultiplier(c, payee, ownCost);
      if (!next) return null;
      result = { amount: "plus", a: result, b: next };
    }
    return result;
  });
}

/** "for each [counter / object]" after a payment: how many times it is paid.
 * A count of targets belongs to a spell or ability being cast, never a cost. */
export function parseCostMultiplier(c: Cursor): Amount | null {
  return c.attempt(c => {
    if (c.word("for") === null || !c.isWord("each")) return null;
    const multiplier = parseEachMultiplier(c, { ref: "affected-object" });
    return multiplier && !JSON.stringify(multiplier).includes('"target-count"') ? multiplier : null;
  });
}

type SentenceMode = "static" | "activation" | "additional-cost" | "keyword-cast" | "effect";
interface ParsedSentence { effect: StaticEffect; duration?: Duration }

/** One sentence in four positions. A static ability names its own scope; an
 * activated ability's or additional-cost line's trailing instruction adjusts
 * only that ability or spell; a resolving effect creates a scoped adjustment
 * that needs a lifetime, which the caller verifies. */
function parseCostModificationSentence(c: Cursor, mode: SentenceMode): ParsedSentence | null {
  const instruction = mode === "activation" || mode === "additional-cost" || mode === "keyword-cast";
  return c.attempt(c => {
    let condition: Condition | undefined;
    if (mode !== "effect" && c.words("during", "your", "turn")) {
      if (!c.punct(",")) return null;
      condition = { condition: "your-turn" };
    } else if (mode !== "effect" && c.words("during", "turns", "other", "than", "yours")) {
      if (!c.punct(",")) return null;
      condition = { condition: "not-your-turn" };
    }
    const start = c.pos;
    let marker = start;
    while (marker < c.tokens.length) {
      const token = c.tokens[marker];
      if (token.kind === "word" && !token.possessive && ["cost", "costs"].includes(token.value) && c.tokens[marker + 1]?.kind === "symbol") break;
      // A resolving instruction is one sentence of a longer body.
      if (mode === "effect" && token.kind === "punct" && [".", ":", '"'].includes(token.value)) return null;
      marker++;
    }
    if (marker === c.tokens.length) return null;
    c.pos = marker + 1;
    const symbols: string[] = [];
    while (c.peek()?.kind === "symbol") {
      const token = c.peek();
      if (token?.kind !== "symbol" || !isManaSymbol(token.value)) return null;
      symbols.push(token.value);
      c.pos++;
    }
    const direction = c.word("less", "more");
    if (!direction) return null;
    const action = c.attempt(c => c.word("to") !== null ? c.word("cast", "activate") : null);
    // "... to activate unless they're mana abilities": the same payments as
    // "abilities that aren't mana abilities" (CR 605.1a, 602.1).
    const exceptMana = action === "activate" && mode !== "activation" &&
      c.attempt(c => c.words("unless", "they're", "mana", "abilities") ? true : null) === true;
    // "... to cast this way": only a cast made with the keyword just stated (Visions flashback).
    if (mode === "keyword-cast" && (action !== "cast" || !c.words("this", "way"))) return null;
    // "... cost {1} less to cast and can't be countered": the same spells are
    // also uncounterable: a second static rule of the same ability. It must
    // end the sentence so no later clause could qualify only the cost change.
    const uncounterable = mode === "static" && action === "cast" &&
      c.attempt(c => c.words("and", "can't", "be", "countered") && (c.done() || c.isPunct(".")) ? true : null) === true;
    let subject = c.tokens.slice(start, marker);
    // "Spells you cast this turn cost ..." states the lifetime of the
    // resolving effect inside its subject (CR 611.2a).
    let duration: Duration | undefined;
    if (mode === "effect") {
      const verb = subject.findIndex((token, index) => token.kind === "word" && !token.possessive &&
        ["cast", "casts"].includes(token.value) && [subject[index + 1], subject[index + 2]].every((next, offset) =>
          next?.kind === "word" && !next.possessive && next.value === ["this", "turn"][offset]));
      if (verb >= 0) {
        subject = [...subject.slice(0, verb + 1), ...subject.slice(verb + 3)];
        duration = { duration: "this-turn" };
      }
    }
    const self = new Cursor(subject);
    let scope: CostModificationScope | null;
    if (action !== "cast" && action !== "activate") scope = mode === "static" ? parseKeywordCostScope(subject) : null;
    else if (mode === "activation") scope = action === "activate" && self.words("this", "ability") && self.done()
      ? { action: "activate", abilities: { ref: "this", noun: "ability" } } : null;
    else if (mode === "additional-cost" || mode === "keyword-cast") scope = action === "cast" && self.words("this", "spell") && self.done()
      ? { action: "cast", spells: { ref: "this", noun: "spell" } } : null;
    else {
      const turn: { yourTurn?: true } = {};
      scope = parseScope(subject, action, mode === "effect", mode === "static" ? turn : undefined);
      if (scope && turn.yourTurn) {
        if (condition) return null;
        condition = { condition: "your-turn" };
      }
    }
    if (!scope) return null;
    if (exceptMana) {
      if (scope.action !== "activate" || scope.keyword || scope.loyalty) return null;
      const named = scope.abilities ?? { ref: "each" as const, filter: { cls: "ability" as const } };
      if (named.ref !== "each" || named.filter.manaAbility !== undefined) return null;
      scope.abilities = { ...named, filter: { ...named.filter, manaAbility: false } };
    }
    if (uncounterable && !(scope.action === "cast" && scope.spells.ref === "each" && !scope.occurrence && !scope.from && !scope.notFrom))
      return null;
    const effect: Modifier = { static: "cost-modification", scope,
      change: { direction: direction === "less" ? "reduce" : "increase", symbols } };
    for (;;) {
      if (!effect.multiplier && c.attempt(c => c.word("for") !== null && c.isWord("each") ? true : null)) {
        const multiplier = c.attempt(c => parseTargetedUnit(c, targetCountOwner(scope))) ?? parseEachMultiplier(c, targetCountOwner(scope), mode === "additional-cost");
        if (!multiplier) return null;
        effect.multiplier = multiplier;
        continue;
      }
      // A resolving sentence owns its "where X is" definition and any
      // condition, exactly as for every other resolving instruction.
      if (mode === "effect") break;
      const binding = c.attempt(c => {
        if (effect.bindings || !c.punct(",") || !c.words("where", "x", "is")) return null;
        return c.attempt(c => parseTargetedValue(c, targetCountOwner(scope))) ?? parseAmount(c);
      });
      if (binding) { effect.bindings = [{ variable: "x", value: binding }]; continue; }
      // "... cost {2} more to activate unless they're mana abilities": the
      // same abilities as "activated abilities that aren't mana abilities"
      // (CR 605.1a); an exception of the adjusted group, not a condition.
      if (scope.action === "activate" && c.attempt(c => c.words("unless", "they're", "mana", "abilities") ? true : null)) {
        const abilities = scope.abilities ?? { ref: "each" as const, filter: { cls: "ability" as const } };
        if (abilities.ref !== "each" || abilities.filter.manaAbility !== undefined) return null;
        scope.abilities = { ...abilities, filter: { ...abilities.filter, manaAbility: false } };
        continue;
      }
      const qualifier = c.word("if", "unless");
      if (qualifier) {
        const parsed = parseCondition(c);
        if (!parsed) return null;
        const next: Condition = qualifier === "unless" ? { condition: "not", of: parsed } : parsed;
        condition = condition ? { condition: "all-of", conditions: [condition, next] } : next;
        continue;
      }
      const ongoing = c.attempt(c => {
        if (!c.isWord("as", "for")) return null;
        const duration = parseDuration(c);
        return duration?.duration === "as-long-as" ? duration.condition : null;
      });
      if (ongoing) {
        condition = condition ? { condition: "all-of", conditions: [condition, ongoing] } : ongoing;
        continue;
      }
      if (c.words("during", "your", "turn")) {
        const next: Condition = { condition: "your-turn" };
        condition = condition ? { condition: "all-of", conditions: [condition, next] } : next;
        continue;
      }
      break;
    }
    if (mode === "effect") return effect.multiplier ? null : { effect, ...(duration ? { duration } : {}) };
    const minimum = c.attempt(c => {
      if (!c.punct(".") || !c.words("this", "effect", "can't", "reduce", "the", "mana", "in", "that", "cost", "to", "less", "than")) return null;
      const amount = parseAmount(c);
      return amount && c.word("mana") !== null ? amount : null;
    });
    if (minimum) effect.minimumTotalMana = minimum;
    // "This effect can't reduce the amount of mana this spell costs by more
    // than {5}": a cap on the size of the reduction itself (CR 601.2f).
    const maximum = minimum ? null : c.attempt(c => {
      if (!c.punct(".") || !c.words("this", "effect", "can't", "reduce", "the", "amount", "of", "mana", "this", "spell", "costs", "by", "more", "than")) return null;
      const symbol = c.peek();
      if (symbol?.kind !== "symbol" || !/^\d+$/.test(symbol.value)) return null;
      c.pos++;
      return { amount: "fixed", value: Number(symbol.value) } satisfies Amount;
    });
    if (maximum) effect.maximumReduction = maximum;
    if (c.attempt(c => c.punct(".") && c.words("this", "effect", "reduces", "only", "the", "amount", "of", "colored", "mana", "you", "pay") ? true : null))
      effect.coloredOnly = true;
    if ((effect.minimumTotalMana || effect.maximumReduction || effect.coloredOnly) && effect.change.direction !== "reduce") return null;
    const period = c.punct(".");
    if (!c.done() && !(instruction ? period : c.isWord("otherwise")))
      return c.fail("end of complete cost modification");
    if (uncounterable && scope.action === "cast" && scope.spells.ref === "each") {
      const uncountered: StaticEffect = { static: "cant", action: "be-countered", what: { ref: "each", filter: {
        ...scope.spells.filter, ...(scope.caster ? { cast: { by: scope.caster } } : {}) } } };
      const both: StaticEffect = { static: "group", effects: [effect, uncountered] };
      return { effect: condition ? { static: "conditional", condition, effect: both } : both };
    }
    return { effect: condition ? { static: "conditional", condition, effect } : effect };
  });
}

/** One statement that states several adjustments of the same spell or
 * ability: "This spell costs {1} less to cast if A and {1} less to cast if B"
 * or "... if A. It also costs {1} less to cast if B." Each adjustment applies
 * on its own (a ruling-confirmed stack: both reduce when both conditions
 * hold, CR 601.2f), so each is parsed as the complete sentence it abbreviates.
 * Returns the token run of each adjustment and where the statement ends. */
function splitStatedAdjustments(c: Cursor, mode: SentenceMode): { parts: Token[][]; end: number } | null {
  const tokens = c.tokens, start = c.pos;
  const word = (index: number) => { const t = tokens[index]; return t?.kind === "word" && !t.possessive ? t.value : undefined; };
  if (word(start) !== "this" || word(start + 1) !== (mode === "activation" ? "ability" : "spell") || word(start + 2) !== "costs") return null;
  const prefix = tokens.slice(start, start + 3);
  const stripPeriod = (run: Token[]) => { const last = run.at(-1); return last?.kind === "punct" && last.value === "." ? run.slice(0, -1) : run; };
  // A further adjustment starts at "and {N} less to cast" or "It also costs {N} less".
  const startsAdjustment = (index: number) => { const t = tokens[index]; return t?.kind === "symbol" && isManaSymbol(t.value); };
  const parts: Token[][] = [];
  let partStart = start;
  let index = start + 3;
  const finish = (until: number, tail: Token[]) => { parts.push([...tail, ...stripPeriod(tokens.slice(partStart, until))]); };
  let first = true;
  let end = tokens.length;
  for (; index < tokens.length; index++) {
    const token = tokens[index];
    if (token.kind === "punct" && token.value === ".") {
      if (mode !== "static" || word(index + 1) !== "it" || word(index + 2) !== "also" || word(index + 3) !== "costs" || !startsAdjustment(index + 4)) {
        end = index + 1;
        break;
      }
      finish(index, first ? [] : prefix);
      first = false;
      partStart = index + 4;
      index += 3;
    } else if (word(index) === "and") {
      let symbols = index + 1;
      while (startsAdjustment(symbols)) symbols++;
      if (symbols === index + 1 || !["less", "more"].includes(word(symbols) ?? "") || word(symbols + 1) !== "to") continue;
      finish(index, first ? [] : prefix);
      first = false;
      partStart = index + 1;
    }
  }
  if (first) return null;
  finish(end, prefix);
  return { parts, end };
}

/** "Spells your opponents cast cost {1} more to cast and abilities your
 * opponents activate cost {1} more to activate": two complete adjustments of
 * different subjects. A shared leading "During your turn," applies to both. */
function splitCoordinatedSubjects(c: Cursor): Token[][] | null {
  const tokens = c.tokens.slice(c.pos);
  const last = tokens.at(-1);
  const trimmed = last?.kind === "punct" && last.value === "." ? tokens.slice(0, -1) : tokens;
  const word = (index: number) => { const t = trimmed[index]; return t?.kind === "word" && !t.possessive ? t.value : undefined; };
  const costAt = (from: number, to: number) => {
    for (let i = from; i < to; i++) if ((word(i) === "cost" || word(i) === "costs") && trimmed[i + 1]?.kind === "symbol") return i;
    return -1;
  };
  let prefix = 0;
  if (word(0) === "during") {
    const comma = trimmed.findIndex(token => token.kind === "punct" && token.value === ",");
    if (comma < 0 || comma > 6) return null;
    prefix = comma + 1;
  }
  for (let index = prefix + 1; index < trimmed.length; index++) {
    if (word(index) !== "and") continue;
    if (costAt(prefix, index) < 0 || costAt(index + 1, trimmed.length) < 0) continue;
    // The second subject must itself be a complete subject, not a clause tail.
    return [trimmed.slice(0, index), [...trimmed.slice(0, prefix), ...trimmed.slice(index + 1)]];
  }
  return null;
}

export function parseCostModification(c: Cursor): StaticEffect | null {
  const subjects = c.attempt(c => {
    const parts = splitCoordinatedSubjects(c);
    if (!parts) return null;
    const effects: StaticEffect[] = [];
    for (const part of parts) {
      const sub = new Cursor(part);
      const parsed = parseCostModificationSentence(sub, "static")?.effect;
      if (!parsed || !sub.done()) return null;
      effects.push(parsed);
    }
    c.pos = c.tokens.length;
    return { static: "group", effects } satisfies StaticEffect;
  });
  if (subjects) return subjects;
  const stated = c.attempt(c => {
    const split = splitStatedAdjustments(c, "static");
    if (!split) return null;
    const effects: StaticEffect[] = [];
    for (const part of split.parts) {
      const sub = new Cursor(part);
      const parsed = parseCostModificationSentence(sub, "static")?.effect;
      if (!parsed || !sub.done()) return null;
      effects.push(parsed);
    }
    c.pos = split.end;
    return { static: "group", effects } satisfies StaticEffect;
  });
  return stated ?? parseCostModificationSentence(c, "static")?.effect ?? null;
}

/** A spell or ability can create a payment adjustment for a stated time,
 * e.g. "Artifact spells you cast this turn cost {1} less to cast" (CR 611.2a).
 * Without a lifetime the sentence is a static ability, never an instruction. */
export function parseCostModificationEffect(c: Cursor, leadingDuration?: Duration): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const parsed = parseCostModificationSentence(c, "effect");
    if (!parsed || parsed.effect.static !== "cost-modification" || parsed.effect.scope.action !== "cast") return null;
    const trailing = parseDuration(c);
    if ([parsed.duration, trailing, leadingDuration].filter(Boolean).length !== 1) return null;
    const duration = parsed.duration ?? trailing;
    const { static: _static, ...modification } = parsed.effect;
    return [{ effect: "cost-modification", ...modification, ...(duration ? { duration } : {}) }];
  });
}

/** CR 602.1b: a trailing "This ability costs ..." changes activation payment
 * and is never executed during resolution. Consume complete instruction
 * sentences only; the caller must still parse and validate the remaining
 * activation restrictions, permissions, and the end of the ability block. */
export function parseActivationCostModifications(c: Cursor): CostModification[] | null {
  return parseOwnCostModifications(c, "activation");
}

/** CR 118.8, 601.2f: "This spell costs ..." after a spell's own additional
 * cost adjusts that spell's total cost; it is part of the same payment
 * instruction and is never a resolving effect. */
export function parseAdditionalCostModifications(c: Cursor): CostModification[] | null {
  return parseOwnCostModifications(c, "additional-cost");
}

/** CR 702.34a, 118.9, 601.2f: after a keyword that supplies an alternative
 * cost (flashback), "This spell costs {X} less to cast this way" adjusts only
 * the total cost of a cast made using that keyword. */
export function parseKeywordCastCostModifications(c: Cursor): CostModification[] | null {
  return parseOwnCostModifications(c, "keyword-cast");
}

function parseOwnCostModifications(c: Cursor, mode: "activation" | "additional-cost" | "keyword-cast"): CostModification[] | null {
  return c.attempt(c => {
    const modifications: CostModification[] = [];
    while (c.attempt(c => {
      const start = c.pos;
      const prefix = c.words("this", mode === "activation" ? "ability" : "spell", "costs");
      c.pos = start;
      return prefix ? true : null;
    })) {
      // Several adjustments stated together each apply on their own.
      const split = splitStatedAdjustments(c, mode);
      const runs = split ? split.parts.map(part => new Cursor(part)) : [c];
      for (const run of runs) {
        const parsed = parseCostModificationSentence(run, mode)?.effect;
        if (!parsed || (split && !run.done())) return null;
        const condition = parsed.static === "conditional" ? parsed.condition : undefined;
        const effect = parsed.static === "conditional" ? parsed.effect : parsed;
        if (effect.static !== "cost-modification") return null;
        const { static: _static, scope: _scope, ...adjustment } = effect;
        modifications.push({ ...adjustment, ...(condition ? { condition } : {}) });
      }
      if (split) c.pos = split.end;
    }
    return modifications.length ? modifications : null;
  });
}
