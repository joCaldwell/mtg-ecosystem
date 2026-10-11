// CR 118.7a-g preserves generic, colored, colorless and hybrid reductions;
// 601.2f applies increases, reductions, and minimums to the total payment.
// These effects never mutate a card's printed mana cost or mana value.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Condition, CostModification, CostModificationScope, PlayerRef, StaticEffect, ZoneRef } from "../ast.ts";
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
function extractActor(tokens: Token[], action: "cast" | "activate"): { tokens: Token[]; actor?: PlayerRef; from?: ZoneRef } | null {
  let result = [...tokens];
  let actor: PlayerRef | undefined;
  let from: ZoneRef | undefined;
  for (let index = 0; index < result.length; index++) {
    const c = new Cursor(result.slice(index));
    const candidate = parseActor(c);
    if (!candidate || c.word(action, action === "cast" ? "casts" : "activates") === null) continue;
    if (actor || isTargetedPlayerRef(candidate)) return null;
    actor = candidate;
    if (action === "cast" && c.word("from") !== null) {
      const origin = parseZoneRef(c);
      if (!origin) return null;
      from = origin;
    }
    result = [...result.slice(0, index), ...result.slice(index + c.pos)];
    index--;
  }
  return { tokens: result, ...(actor ? { actor } : {}), ...(from ? { from } : {}) };
}

function parseScope(tokens: Token[], action: "cast" | "activate"): CostModificationScope | null {
  const extracted = extractActor(tokens, action);
  if (!extracted) return null;
  const c = new Cursor(extracted.tokens);
  if (action === "cast") {
    const spells = parseEligibleCardRef(c);
    if (!spells || !c.done() || spells.ref === "target") return null;
    return { action: "cast", spells, ...(extracted.actor ? { caster: extracted.actor } : {}), ...(extracted.from ? { from: extracted.from } : {}) };
  }
  const scope: Extract<CostModificationScope, { action: "activate" }> = {
    action: "activate", ...(extracted.actor ? { activator: extracted.actor } : {}),
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
    const aggregate = c.attempt(c => c.word("each") !== null
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

function parseCostModificationSentence(c: Cursor, activationInstruction: boolean): StaticEffect | null {
  return c.attempt(c => {
    let condition: Condition | undefined;
    if (c.words("during", "your", "turn")) {
      if (!c.punct(",")) return null;
      condition = { condition: "your-turn" };
    } else if (c.words("during", "turns", "other", "than", "yours")) {
      if (!c.punct(",")) return null;
      condition = { condition: "not-your-turn" };
    }
    const start = c.pos;
    let marker = start;
    while (marker < c.tokens.length) {
      const token = c.tokens[marker];
      if (token.kind === "word" && !token.possessive && ["cost", "costs"].includes(token.value) && c.tokens[marker + 1]?.kind === "symbol") break;
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
    if (!direction || c.word("to") === null) return null;
    const action = c.word("cast", "activate");
    if (action !== "cast" && action !== "activate") return null;
    const subject = c.tokens.slice(start, marker);
    const self = new Cursor(subject);
    const scope: CostModificationScope | null = activationInstruction
      ? action === "activate" && self.words("this", "ability") && self.done()
        ? { action: "activate", abilities: { ref: "this", noun: "ability" } } : null
      : parseScope(subject, action);
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
    if (!c.done() && !(activationInstruction ? period : c.isWord("otherwise")))
      return c.fail("end of complete cost modification");
    return condition ? { static: "conditional", condition, effect } : effect;
  });
}

export function parseCostModification(c: Cursor): StaticEffect | null {
  return parseCostModificationSentence(c, false);
}

/** CR 602.1b: a trailing "This ability costs ..." changes activation payment
 * and is never executed during resolution. Consume complete instruction
 * sentences only; the caller must still parse and validate the remaining
 * activation restrictions, permissions, and the end of the ability block. */
export function parseActivationCostModifications(c: Cursor): CostModification[] | null {
  return c.attempt(c => {
    const modifications: CostModification[] = [];
    while (c.attempt(c => {
      const start = c.pos;
      const prefix = c.words("this", "ability", "costs");
      c.pos = start;
      return prefix ? true : null;
    })) {
      const parsed = parseCostModificationSentence(c, true);
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
