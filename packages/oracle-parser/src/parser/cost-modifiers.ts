// CR 118.7a-g preserves generic, colored, colorless and hybrid reductions;
// 601.2f applies increases, reductions, and minimums to the total payment.
// These effects never mutate a card's printed mana cost or mana value.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Condition, CostModification, CostModificationOccurrence, CostModificationScope, Duration, Effect, PlayerRef, StaticEffect, ZoneRef } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { isManaSymbol } from "../symbols.ts";
import { Cursor } from "./cursor.ts";
import { isTargetedPlayerRef, parseAmount, parseCondition, parseCounterSpec, parseDuration, parseObjectRef, parsePlayerRef, parseZoneRef } from "./refs.ts";
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
function extractActor(tokens: Token[], action: "cast" | "activate"): { tokens: Token[]; actor?: PlayerRef; from?: ZoneRef; notFrom?: ZoneRef } | null {
  let result = [...tokens];
  let actor: PlayerRef | undefined;
  let from: ZoneRef | undefined;
  let notFrom: ZoneRef | undefined;
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
      if (excluded) notFrom = origin; else from = origin;
    }
    result = [...result.slice(0, index), ...result.slice(index + c.pos)];
    index--;
  }
  return { tokens: result, ...(actor ? { actor } : {}), ...(from ? { from } : {}), ...(notFrom ? { notFrom } : {}) };
}

const ORDINALS: Record<string, number> = { first: 1, second: 2, third: 3 };

/** "The first creature spell you cast each turn" limits the adjustment to one
 * ordinal among that actor's matching casts or activations in a turn. "The
 * next ..." exists only for a resolving effect, which it uses up. */
function extractOccurrence(tokens: Token[], resolving: boolean): { tokens: Token[]; occurrence?: CostModificationOccurrence } | null {
  const [article, marker] = tokens;
  if (article?.kind !== "word" || article.value !== "the" || marker?.kind !== "word" || marker.possessive) return { tokens };
  if (marker.value === "next") return resolving ? { tokens: tokens.slice(2), occurrence: { next: true } } : null;
  const ordinal = ORDINALS[marker.value];
  if (!ordinal) return { tokens };
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
  const cost = c.word("equip", "flashback", "buyback", "dash", "morph", "unlock");
  if (!cost || c.word("costs") === null) return null;
  let payer: PlayerRef | undefined;
  if (!c.done()) {
    const who = all ? null : parseActor(c);
    if (!who || isTargetedPlayerRef(who) || c.word("pay", "pays") === null || !c.done()) return null;
    payer = who;
  }
  if (cost === "equip") return { action: "activate", ...(payer ? { activator: payer } : {}), keyword: "equip",
    abilities: { ref: "each", filter: { cls: "ability" } } };
  return { action: "pay-cost", cost: cost as "flashback" | "buyback" | "dash" | "morph" | "unlock", ...(payer ? { payer } : {}) };
}

function parseScope(tokens: Token[], action: "cast" | "activate", resolving = false): CostModificationScope | null {
  const ordered = extractOccurrence(tokens, resolving);
  const extracted = ordered && extractActor(ordered.tokens, action);
  if (!ordered || !extracted) return null;
  // An ordinal or "next" is counted among one stated actor's own actions.
  if (ordered.occurrence && !extracted.actor) return null;
  const occurrence = ordered.occurrence ? { occurrence: ordered.occurrence } : {};
  const c = new Cursor(extracted.tokens);
  if (action === "cast") {
    const spells = parseEligibleCardRef(c);
    if (!spells || !c.done() || spells.ref === "target") return null;
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
  const possessed = c.attempt(c => {
    if (c.word("this") === null) return null;
    const noun = c.next();
    if (noun?.kind !== "word" || !noun.possessive) return null;
    const local = new Cursor([c.tokens[0], { ...noun, possessive: false }]);
    const source = parseObjectRef(local);
    return source && local.done() ? source : null;
  });
  if (possessed) scope.source = possessed;
  const keyword = c.word("equip", "boast", "cycling", "ninjutsu", "exhaust", "power-up");
  if (keyword === "equip" || keyword === "boast" || keyword === "cycling" || keyword === "ninjutsu" ||
      keyword === "exhaust" || keyword === "power-up") scope.keyword = keyword;
  else if (possessed) return null;
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

/** A multiplier explicitly distinguishes objects, counters, and represented
 * types/colors. Explicit
 * repeated "each" groups add their cardinalities; a nominal eligibility
 * union counts each matching object once (Bedlam Reveler vs Gargantuan Leech).
 * Counter holders may be an entire specified set (Deepwood Denizen, CR 122.1). */
function parseEachMultiplier(c: Cursor): Amount | null {
  return c.attempt(c => {
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
      if (c.word("on") === null) return null;
      const on = parsePlayerRef(c) ?? parseObjectRef(c);
      return on ? { amount: "counter-count", counter, on } : null;
    });
    const each = aggregate || counterCount ? null : parseEligibleCardRef(c, { distributive: true });
    let result: Amount;
    if (aggregate) result = aggregate;
    else if (counterCount) result = counterCount;
    else if (each?.ref === "each" && !hasCombatHistory(each.filter)) result = { amount: "count", of: each.filter };
    else return null;
    if (c.attempt(c => c.word("and") !== null && c.isWord("each") ? true : null)) {
      const next = parseEachMultiplier(c);
      if (!next) return null;
      result = { amount: "plus", a: result, b: next };
    }
    return result;
  });
}

type SentenceMode = "static" | "activation" | "additional-cost" | "effect";
interface ParsedSentence { effect: StaticEffect; duration?: Duration }

/** One sentence in four positions. A static ability names its own scope; an
 * activated ability's or additional-cost line's trailing instruction adjusts
 * only that ability or spell; a resolving effect creates a scoped adjustment
 * that needs a lifetime, which the caller verifies. */
function parseCostModificationSentence(c: Cursor, mode: SentenceMode): ParsedSentence | null {
  const instruction = mode === "activation" || mode === "additional-cost";
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
    else if (mode === "additional-cost") scope = action === "cast" && self.words("this", "spell") && self.done()
      ? { action: "cast", spells: { ref: "this", noun: "spell" } } : null;
    else scope = parseScope(subject, action, mode === "effect");
    if (!scope) return null;
    const effect: Modifier = { static: "cost-modification", scope,
      change: { direction: direction === "less" ? "reduce" : "increase", symbols } };
    for (;;) {
      if (!effect.multiplier && c.attempt(c => c.word("for") !== null && c.isWord("each") ? true : null)) {
        const multiplier = parseEachMultiplier(c);
        if (!multiplier) return null;
        effect.multiplier = multiplier;
        continue;
      }
      // A resolving sentence owns its "where X is" definition and any
      // condition, exactly as for every other resolving instruction.
      if (mode === "effect") break;
      const binding = c.attempt(c => {
        if (effect.bindings || !c.punct(",") || !c.words("where", "x", "is")) return null;
        return parseAmount(c);
      });
      if (binding) { effect.bindings = [{ variable: "x", value: binding }]; continue; }
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
    if (c.attempt(c => c.punct(".") && c.words("this", "effect", "reduces", "only", "the", "amount", "of", "colored", "mana", "you", "pay") ? true : null))
      effect.coloredOnly = true;
    if ((effect.minimumTotalMana || effect.coloredOnly) && effect.change.direction !== "reduce") return null;
    const period = c.punct(".");
    if (!c.done() && !(instruction ? period : c.isWord("otherwise")))
      return c.fail("end of complete cost modification");
    return { effect: condition ? { static: "conditional", condition, effect } : effect };
  });
}

export function parseCostModification(c: Cursor): StaticEffect | null {
  return parseCostModificationSentence(c, "static")?.effect ?? null;
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

function parseOwnCostModifications(c: Cursor, mode: "activation" | "additional-cost"): CostModification[] | null {
  return c.attempt(c => {
    const modifications: CostModification[] = [];
    while (c.attempt(c => {
      const start = c.pos;
      const prefix = c.words("this", mode === "activation" ? "ability" : "spell", "costs");
      c.pos = start;
      return prefix ? true : null;
    })) {
      const parsed = parseCostModificationSentence(c, mode)?.effect;
      if (!parsed) return null;
      const condition = parsed.static === "conditional" ? parsed.condition : undefined;
      const effect = parsed.static === "conditional" ? parsed.effect : parsed;
      if (effect.static !== "cost-modification") return null;
      const { static: _static, scope: _scope, ...adjustment } = effect;
      modifications.push({ ...adjustment, ...(condition ? { condition } : {}) });
    }
    return modifications.length ? modifications : null;
  });
}
