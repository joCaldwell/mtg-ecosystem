// Static (continuous) abilities: anthems, CDAs, enters-tapped, can't-effects.

import type { Amount, Color, Condition, ObjectRef, StaticEffect, StaticModifier } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import type { Token } from "../lexer.ts";
import { bindStaticOtherExclusions } from "./other-exclusion.ts";
import { parseAbilityRemoval, parseBasePt, parseCantAction, parseGainedAbilities, parsePtModifier, parseSentences } from "./effects.ts";
import { parseStaticPrevention } from "./prevention.ts";
import { parseCostModification } from "./cost-modifiers.ts";
import { parseCostList } from "./costs.ts";
import { parseCombatEffect, parseCombatStatic } from "./combat.ts";
import { parseStaticReplacement } from "./replacement.ts";
import { parsePlayPermissionStatic } from "./permissions.ts";
import { parsePersistentRule } from "./persistent-rules.ts";
import { parseStaticSkip } from "./temporal.ts";
import { parseStaticUntapSuppression } from "./untap.ts";
import { parseEntryChoice, parseStaticEntryCounters } from "./entry.ts";
import { parseEntryCopy } from "./entry-copy.ts";
import { parseUnlessPayment } from "./payment-choices.ts";
import { parseStaticProhibition } from "./prohibitions.ts";
import { hasFreshProtectionChoice, parseProtectionAttachmentException } from "./protection.ts";
import { parseCharacteristicModifiers, parseStaticBasePt } from "./characteristics.ts";
import { intrinsicCharacteristicDefinition, ordinaryCharacteristicDefinition } from "./characteristic-origin.ts";
import { hasCombatHistory, parseAggregateCount } from "./aggregates.ts";
import {
  isTargetedPlayerRef, parseAmount, parseCondition, parseDuration,
  isPluralPlayerRef, parseCounterSpec, parseEqualTo, parseFilter, parseObjectRef, parsePlayerRef,
} from "./refs.ts";

function parseStaticSubject(c: Cursor): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    const generic = c.isWord("a", "an");
    const what = parseObjectRef(c);
    // An indefinite subject states a rule for every matching object; it is
    // not a choice of one object (CR 611.3a). Explicit counts remain counts.
    if (generic && what?.ref === "filter" && what.count?.amount === "fixed" && what.count.value === 1)
      return { ref: "each", filter: what.filter };
    return what;
  });
}

function parseStaticMultiplier(c: Cursor): Amount | null {
  return parseAggregateCount(c, { parseObjectRef }, { singular: true }) ?? c.attempt((c): Amount | null => {
    const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
    if (!counter || c.word("counter", "counters") === null) return null;
    if (c.word("on") !== null) {
      const on = parsePlayerRef(c) ?? parseObjectRef(c);
      if (on && "ref" in on && ["each", "all", "filter", "zone-cards"].includes(on.ref)) return null;
      if (on && "player" in on && isPluralPlayerRef(on)) return null;
      return on ? { amount: "counter-count", counter, on } : null;
    }
    const on = parsePlayerRef(c);
    // A sum across players or objects requires an explicit aggregate. The
    // currently singular counter-count node is not such an aggregate.
    if (!on || !["you", "enchanted-player", "controller", "owner", "that-player"].includes(on.player)
      || c.word("have", "has") === null) return null;
    return { amount: "counter-count", counter, on };
  });
}

/** Continuous modifiers retain separate multipliers and conditions. Unlike a
 * resolving pump, their values are recalculated as the game state changes.
 * CR 611.3: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseStaticModifiers(c: Cursor, what: ObjectRef, allowFreshChoice = false): {
  modifiers: StaticModifier[]; condition?: Condition; conditionScope?: "each-affected-object";
} | null {
  return c.attempt((c) => {
    const modifiers: StaticModifier[] = [];
    for (;;) {
      let modifier: StaticModifier;
      const characteristics = parseCharacteristicModifiers(c, what, parseGainedAbilities);
      const combat = c.attempt(c => {
        if (characteristics) return null;
        const effects = parseCombatEffect(c, what);
        const effect = effects?.[0];
        return effects?.length === 1 && effect?.effect === "combat-rule" && !effect.duration && JSON.stringify(effect.what) === JSON.stringify(what) ? effect : null;
      });
      if (characteristics) {
        if (!allowFreshChoice && characteristics.some(modifier => modifier.modifier === "abilities" && hasFreshProtectionChoice(modifier.abilities))) return null;
        modifiers.push(...characteristics.slice(0, -1));
        modifier = characteristics[characteristics.length - 1];
      } else if (combat) modifier = { modifier: "combat-rule", rule: combat.rule };
      else if (c.word("can't") !== null) {
        const action = parseCantAction(c);
        if (!action) return null;
        modifier = { modifier: "cant", action };
      } else {
        const verb = c.word("get", "gets", "have", "has", "gain", "gains", "lose", "loses");
        if (!verb) return null;
        if (verb.startsWith("get")) {
          const pt = parsePtModifier(c);
          if (!pt) return null;
          modifier = { modifier: "pt", power: pt.power, toughness: pt.toughness };
        } else if (verb === "lose" || verb === "loses") {
          const removal = parseAbilityRemoval(c);
          if (!removal) return null;
          modifier = { modifier: "lose-abilities", removal };
        } else {
          const pt = verb === "have" || verb === "has" ? parseStaticBasePt(c) ?? parseBasePt(c) : null;
          if (pt) modifier = { modifier: "base-pt", ...pt,
            ...(pt.power.amount !== "fixed" || pt.toughness.amount !== "fixed" ? { valueScope: "each-affected-object" as const } : {}) };
          else {
            const abilities = parseGainedAbilities(c);
            if (!abilities || !allowFreshChoice && hasFreshProtectionChoice(abilities)) return null;
            modifier = { modifier: "abilities", abilities };
          }
        }
      }
      for (;;) {
        if (!modifier.forEach && !modifier.multiplier && c.words("for", "each")) {
          const multiplier = modifier.modifier === "pt" ? parseStaticMultiplier(c) : null;
          if (multiplier) {
            modifier.multiplier = multiplier;
            modifier.valueScope = "each-affected-object";
            continue;
          }
          const forEach = parseFilter(c);
          if (!forEach || hasCombatHistory(forEach)) return null;
          modifier.forEach = forEach;
          continue;
        }
        if (!modifier.condition) {
          if (c.word("if") !== null) {
            const condition = parseCondition(c);
            if (!condition) return null;
            modifier.condition = condition;
            modifier.conditionScope = "each-affected-object";
            continue;
          }
          const duration = parseDuration(c);
          if (duration) {
            if (duration.duration !== "as-long-as") return null;
            modifier.condition = duration.condition;
            continue;
          }
        }
        break;
      }
      modifiers.push(modifier);
      const next = c.attempt((c) => {
        const comma = c.punct(",");
        const and = c.word("and") !== null;
        if (!comma && !and) return null;
        return c.isWord("is", "are", "isn't", "aren't", "get", "gets", "have", "has", "gain", "gains", "lose", "loses", "can't", "can", "must", "attacks", "attack", "blocks", "block") ? true : null;
      });
      if (!next) break;
    }
    const last = modifiers[modifiers.length - 1];
    // A single trailing condition qualifies the coordinated modifier group;
    // individually conditioned clauses, as on Tek, retain their own guards.
    if (last.condition && modifiers.slice(0, -1).every(modifier => !modifier.condition)) {
      const condition = last.condition;
      const conditionScope = last.conditionScope;
      delete last.condition;
      delete last.conditionScope;
      return { modifiers, condition, ...(conditionScope ? { conditionScope } : {}) };
    }
    return { modifiers };
  });
}

export function parseStatic(c: Cursor): StaticEffect | null {
  return c.attempt((c) => {
    const effect = parseStaticUnbound(c);
    if (!effect) return null;
    if (containsStaticTarget(effect)) return c.fail("static effects without announced targets");
    return bindStaticOtherExclusions(effect) ?? c.fail("unambiguous other-object exclusion");
  });
}

/** Static abilities do not announce targets (CR 115.1, 604.1). Targets in a
 * granted spell/activated/triggered ability belong to that separate scope.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function containsStaticTarget(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(containsStaticTarget);
  if ("kind" in value && typeof value.kind === "string" && ["spell", "static", "activated", "triggered", "loyalty", "keywords", "saga", "max-speed"].includes(value.kind)) return false;
  if (("ref" in value && (value.ref === "target" || value.ref === "any-target")) || ("to" in value && value.to === "any-target") ||
      ("player" in value && ["target-player", "target-opponent"].includes(String(value.player)))) return true;
  return Object.values(value).some(containsStaticTarget);
}

export function hasUnsupportedStaticTargets(c: Cursor): boolean {
  const start = c.pos;
  const effect = parseStaticUnbound(c);
  c.pos = start;
  return effect !== null && containsStaticTarget(effect);
}

/** A failed static binding must not turn into a one-shot spell on fallback. */
export function hasUnboundStaticOther(c: Cursor): boolean {
  const start = c.pos;
  const effect = parseStaticUnbound(c);
  c.pos = start;
  return effect !== null && bindStaticOtherExclusions(effect) === null;
}

/** A continuous "has/gets ... if" instruction that fails its complete static
 * grammar must not become a one-shot resolving grant on classifier fallback.
 * Explicit temporary durations remain resolution text. */
export function hasUnsupportedStaticConditional(c: Cursor): boolean {
  const start = c.pos;
  const freshObjectChoice = c.attempt(c => {
    const what = parseStaticSubject(c);
    if (!what || containsStaticTarget(what)) return null;
    const group = parseStaticModifiers(c, what, true);
    return group?.modifiers.some(modifier => modifier.modifier === "abilities" && hasFreshProtectionChoice(modifier.abilities)) ? true : null;
  });
  c.pos = start;
  if (freshObjectChoice) return true;
  const freshPlayerChoice = c.attempt(c => {
    const who = parsePlayerRef(c);
    if (!who || isTargetedPlayerRef(who) || c.word("have", "has") === null) return null;
    const abilities = parseGainedAbilities(c);
    if (!abilities || !hasFreshProtectionChoice(abilities)) return null;
    const duration = parseDuration(c);
    return !duration || duration.duration === "as-long-as" ? true : null;
  });
  c.pos = start;
  if (freshPlayerChoice) return true;
  // CR 508.1g-h / 509.1f-g: these payments are declaration costs, not
  // optional payments made while resolving an instruction (CR 118.12a).
  const declarationCost = c.attempt(c => {
    if (!parseStaticSubject(c) || c.word("can't") === null) return null;
    const action = parseCantAction(c);
    if (!["attack", "block", "attack-or-block"].includes(action ?? "") || c.word("unless") === null) return null;
    return parseUnlessPayment(c) ? true : null;
  });
  c.pos = start;
  if (declarationCost) return true;
  const matched = c.attempt(c => {
    const what = parseStaticSubject(c);
    if (!what || what.ref === "target") return null;
    const verb = c.word("has", "have", "get", "gets");
    if (!verb) return null;
    // Ignore durations inside the granted keyword/quoted ability; those do
    // not give this outer grant a resolution-time duration.
    if (verb === "get" || verb === "gets") {
      if (!parsePtModifier(c)) return null;
    } else if (!parseGainedAbilities(c)) return null;
    // Only an outer duration on this first instruction distinguishes a
    // resolving pump. A later sentence's temporary grant, or a duration in
    // a coordinated quoted ability, cannot give the opening static a lifetime.
    // CR 611.3a-b and 614.15 distinguish continually evaluated modifiers from
    // self-replacement effects of a resolving spell/ability.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    let quoted = false;
    const remaining = c.tokens.slice(c.pos).filter(token => {
      if (token.kind === "punct" && token.value === '"') {
        quoted = !quoted;
        return false;
      }
      return !quoted;
    });
    const isWord = (values: string[]) => (token: typeof remaining[number] | undefined) => token?.kind === "word" && !token.possessive && values.includes(token.value);
    const firstIf = remaining.findIndex(isWord(["if"]));
    if (firstIf < 0) return null;
    const firstPeriod = remaining.findIndex(token => token.kind === "punct" && token.value === ".");
    const beforeCondition = remaining.slice(0, firstPeriod < 0 ? firstIf : Math.min(firstIf, firstPeriod));
    const temporary = beforeCondition.some(isWord(["until"])) || beforeCondition.some((token, index) =>
      token.kind === "word" && token.value === "this" && isWord(["turn", "combat"])(beforeCondition[index + 1]));
    return !temporary ? true : null;
  });
  c.pos = start;
  return matched === true;
}

function parseStaticUnbound(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const prevention = parseStaticPrevention(c, parseSentences);
    if (prevention) return prevention;
    const permission = parsePlayPermissionStatic(c);
    if (permission) return permission;
    const untapSuppression = parseStaticUntapSuppression(c);
    if (untapSuppression) return untapSuppression;
    const skip = parseStaticSkip(c, { player: parsePlayerRef, condition: parseCondition });
    if (skip) return skip;
    const effect = parseStaticBase(c) ?? parseStaticGroup(c);
    if (!effect) return null;
    if (c.word("otherwise") === null) return effect;
    c.punct(",");
    const otherwise = parseStaticBase(c);
    if (!otherwise || !c.done()) return null;
    if (effect.static === "conditional") return { ...effect, otherwise };
    if (effect.static === "modify" && effect.condition) {
      if (effect.conditionScope) return c.fail("per-object static otherwise branch");
      const { condition, ...inner } = effect;
      return { static: "conditional", condition, effect: inner, otherwise };
    }
    return c.fail("condition for static otherwise branch");
  });
}

/** Parse independently continuous clauses joined by "and" or a sentence
 * boundary. This grammar never converts resolving Effect nodes into statics.
 * The conjunction inherits only its explicit preceding grammatical subject;
 * pronouns inside granted abilities retain their separate source scope.
 * CR 611.3: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseStaticGroup(c: Cursor): StaticEffect | null {
  return c.attempt(c => {
    const tokens = c.tokens.slice(c.pos);
    const subjectCursor = new Cursor(tokens);
    const subject = parseStaticSubject(subjectCursor);
    if (!subject || subject.ref === "target") return null;
    const subjectTokens = tokens.slice(0, subjectCursor.pos);
    let quoted = false;
    for (let index = subjectCursor.pos; index < tokens.length; index++) {
      const token = tokens[index];
      if (token.kind === "punct" && token.value === '"') { quoted = !quoted; continue; }
      if (quoted) continue;
      const isAnd = token.kind === "word" && !token.possessive && token.value === "and";
      const isPeriod = token.kind === "punct" && token.value === ".";
      if (!isAnd && !isPeriod) continue;
      let end = index;
      const previous = tokens[end - 1];
      if (previous?.kind === "punct" && previous.value === ",") end--;
      const leftCursor = new Cursor(tokens.slice(0, end));
      const left = parseStaticBase(leftCursor);
      if (!left || !leftCursor.done()) continue;
      // Separate entry replacement abilities can share a paragraph. Their
      // group represents the same entering source, not sequential resolving
      // tap/choose actions (CR 614.1c-d, 614.12, 616.1).
      const entry = isPeriod && (left.static === "enters-tapped" || left.static === "enters-with-counters") &&
        (subject.ref === "this" || subject.ref === "self");
      if (!entry && !isUngovernedStaticClause(left)) continue;
      let tail = tokens.slice(index + 1);
      if (entry && tail[0]?.kind === "word" && tail[0].value === "as" && tail[1]?.kind === "word" && tail[1].value === "it")
        tail = [tail[0], ...subjectTokens, ...tail.slice(2)];
      const remaining = staticContinuationTokens(tail, subject, subjectTokens, isAnd);
      if (!remaining) continue;
      const rightCursor = new Cursor(remaining);
      const right = parseStaticUnbound(rightCursor);
      if (!right || !rightCursor.done() || (entry ? right.static !== "as-enters" : !isUngovernedStaticClause(right))) continue;
      if (entry && right.static === "as-enters" && JSON.stringify(right.what) !== JSON.stringify(subject)) continue;
      // "..., where X is ..." can bind the preceding instruction as well.
      // Do not assign a final binding solely to the right clause when a left
      // clause also depends on that same free variable.
      if (containsField(right, "variable", "x") && containsField(left, "amount", "x")) continue;
      c.pos = c.tokens.length;
      return { static: "group", effects: [left, ...(right.static === "group" ? right.effects : [right])] };
    }
    return null;
  });
}

function containsField(value: unknown, key: string, expected: string): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(item => containsField(item, key, expected));
  if (key in value && (value as Record<string, unknown>)[key] === expected) return true;
  return Object.values(value).some(item => containsField(item, key, expected));
}

function isUngovernedStaticClause(effect: StaticEffect): boolean {
  if (effect.static === "group") return effect.effects.every(isUngovernedStaticClause);
  if (!["modify", "cant", "combat-rule", "untap-step-suppression", "player-abilities", "prohibit", "cast-limit"].includes(effect.static)) return false;
  // A trailing condition could govern the whole conjunction. Keep those
  // forms unsupported until their scope has an explicit representation.
  return !hasOuterCondition(effect);
}

function hasOuterCondition(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(hasOuterCondition);
  if ("gained" in value && value.gained === "quoted") return false;
  if ("condition" in value && value.condition) return true;
  if ("prohibition" in value && "when" in value && value.when) return true;
  return Object.values(value).some(hasOuterCondition);
}

function staticContinuationTokens(tokens: Token[], subject: ObjectRef, subjectTokens: Token[], conjunction: boolean): Token[] | null {
  const first = tokens[0];
  if (!first) return null;
  if (first.kind !== "word" || first.possessive) return tokens;
  if (first.value === "its" && ["self", "this", "enchanted", "equipped"].includes(subject.ref)) {
    const last = subjectTokens[subjectTokens.length - 1];
    if (last?.kind === "word" || last?.kind === "selfref")
      return [...subjectTokens.slice(0, -1), { ...last, possessive: true }, ...tokens.slice(1)];
  }
  if (first.value === "it" || first.value === "they") {
    const explicitlyEach = subjectTokens[0]?.kind === "word" && subjectTokens[0].value === "each";
    const plural = subject.ref === "each" ? !explicitlyEach : subject.ref === "them" ? true :
      "count" in subject && subject.count?.amount === "fixed" ? subject.count.value !== 1 :
      ["self", "this", "enchanted", "equipped", "it", "that"].includes(subject.ref) ? false : undefined;
    if (plural === undefined || plural !== (first.value === "they")) return null;
    return [...subjectTokens, ...tokens.slice(1)];
  }
  if (conjunction && ["get", "gets", "has", "have", "gain", "gains", "lose", "loses", "can't", "can", "doesn't", "don't", "must", "attack", "attacks", "block", "blocks"].includes(first.value))
    return [...subjectTokens, ...tokens];
  return tokens;
}

function parseStaticBase(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const entryCopy = parseEntryCopy(c, parseGainedAbilities);
    if (entryCopy) return entryCopy;
    const persistent = parsePersistentRule(c);
    if (persistent) return persistent;
    const prohibition = parseStaticProhibition(c);
    if (prohibition) return prohibition;
    const entryChoice = parseEntryChoice(c);
    if (entryChoice) return entryChoice;
    const alternativeCost = parseAlternativeCost(c);
    if (alternativeCost) return alternativeCost;
    const replacement = parseStaticReplacement(c, parseSentences);
    if (replacement) return replacement;
    const costModification = parseCostModification(c);
    if (costModification) return costModification;
    const combat = parseCombatStatic(c);
    if (combat) return combat;
    const untapSuppression = parseStaticUntapSuppression(c);
    if (untapSuppression) return untapSuppression;
    const prevention = parseStaticPrevention(c, parseSentences);
    if (prevention) return prevention;
    // "As long as <condition>, <static>" / entry replacement "If <condition>"
    const conditioned = c.attempt((c): StaticEffect | null => {
      if (!c.words("as", "long", "as") && c.word("if") === null) return null;
      const condition = parseCondition(c);
      if (!condition || !c.punct(",")) return null;
      const inner = parseStaticBase(c) ?? parseStaticGroup(c);
      if (!inner) return null;
      // Preserve both an outer condition and any nested/trailing condition.
      // A conditional characteristic-setting ability is not a CDA (604.3a).
      return { static: "conditional", condition, effect: ordinaryCharacteristicDefinition(inner) };
    });
    if (conditioned) return conditioned;

    // Choices made as a permanent enters are replacement instructions, not
    // triggered abilities (CR 614.1c). Keep the complete associated body.
    const asEnters = c.attempt((c): StaticEffect | null => {
      if (c.word("as") === null) return null;
      const what = parseStaticSubject(c);
      if (!what || c.word("enters", "enter") === null) return null;
      c.words("the", "battlefield");
      if (!c.punct(",")) return null;
      const effects = parseSentences(c, { modalOptions: [] });
      return effects && c.done() ? { static: "as-enters", what, effects } : null;
    });
    if (asEnters) return asEnters;

    // CR 614.1d / 614.12: these are replacement effects, and the condition is
    // checked as the permanent would enter, rather than being an ETB trigger.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    // "~ enters (the battlefield)? tapped (if/unless <condition>)?"
    const entersTapped = c.attempt((c): StaticEffect | null => {
      const what = parseStaticSubject(c);
      if (!what) return null;
      if (c.word("enters", "enter") === null) return null;
      c.words("the", "battlefield");
      if (c.word("tapped") === null) return null;
      const conditionKind = c.word("if", "unless") as "if" | "unless" | null;
      const condition = conditionKind ? parseCondition(c) : null;
      if (conditionKind && !condition) return null;
      c.punct(".");
      if (!c.done() && !c.isWord("otherwise")) return null;
      return { static: "enters-tapped", what,
        ...(condition && conditionKind ? { condition, conditionKind } : {}) };
    });
    if (entersTapped) return entersTapped;

    // "~ enters with N <kind> counters on it."
    const entersWithCounters = parseStaticEntryCounters(c);
    if (entersWithCounters) return entersWithCounters;

    // CDA: "~'s power and toughness are each equal to <amount>." /
    //      "~'s power is equal to <amount>."
    const cda = c.attempt((c): StaticEffect | null => {
      let what: ObjectRef | null = null;
      if (c.selfrefPossessive()) what = { ref: "self" };
      else if (c.word("its") !== null) what = { ref: "it" };
      else if (c.word("this") !== null) {
        const noun = c.possessive("creature", "token", "permanent", "artifact", "land", "enchantment");
        if (noun) what = { ref: "this", noun };
      }
      if (!what) return null;
      let stat: "power" | "toughness" | "both";
      if (c.words("power", "and", "toughness")) stat = "both";
      else {
        const w = c.word("power", "toughness");
        if (!w) return null;
        stat = w as "power" | "toughness";
      }
      if (c.word("is", "are") === null) return null;
      c.word("each");
      const value = parseEqualTo(c) ?? parseAmount(c);
      if (!value) return null;
      c.punct(".");
      if (!c.done() && !c.isWord("otherwise")) return null;
      return { static: "cda-pt", what, stat, value };
    });
    if (cda) return cda;

    // Self-defining color CDAs function in all zones (CR 105.2, 604.3a).
    // An anthem affecting other objects is a different kind of static effect.
    const colors = c.attempt((c): StaticEffect | null => {
      const what = parseStaticSubject(c);
      if (!what || (what.ref !== "self" && what.ref !== "this")) return null;
      if (c.word("is") === null) return null;
      let colors: Color[];
      if (c.words("all", "colors")) colors = ["white", "blue", "black", "red", "green"];
      else if (c.word("colorless") !== null) colors = [];
      else return null;
      c.punct(".");
      // A rider such as Fallaji Wayfarer's color-identity exception must not
      // disappear simply because its preceding color sentence was supported.
      if (!c.done() && !c.isWord("otherwise")) return null;
      return { static: "cda-colors", what, operation: "set", colors };
    });
    if (colors) return colors;

    // CR 113.1b: players can have abilities, too. "You" and "each player"
    // are player references, never capitalized creature subtypes.
    const playerAbilities = c.attempt((c): StaticEffect | null => {
      const who = parsePlayerRef(c);
      if (!who || c.word("have", "has") === null) return null;
      const abilities = parseGainedAbilities(c);
      if (!abilities || hasFreshProtectionChoice(abilities)) return null;
      const duration = parseDuration(c);
      if (duration && duration.duration !== "as-long-as") return null;
      c.punct(".");
      if (!c.done() && !c.isWord("otherwise")) return null;
      const effect: StaticEffect = { static: "player-abilities", who, abilities };
      return duration ? { static: "conditional", condition: duration.condition, effect } : effect;
    });
    if (playerAbilities) return playerAbilities;

    // "<subject> can't <action> (as long as <condition>)?"
    const cant = c.attempt((c): StaticEffect | null => {
      const what = parseStaticSubject(c);
      if (!what) return null;
      if (c.word("can't") === null) return null;
      const action = parseCantAction(c);
      if (!action) return null;
      let condition: Condition | undefined;
      const cond = c.attempt((c): Condition | null => {
        if (!c.words("as", "long", "as")) return null;
        return parseCondition(c);
      });
      if (cond) condition = cond;
      let conditionKind: "unless" | undefined;
      if (!condition && c.word("unless") !== null) {
        const unless = parseCondition(c);
        if (!unless) return null;
        condition = unless;
        conditionKind = "unless";
      }
      c.punct(".");
      if (!c.done() && !c.isWord("otherwise")) return null;
      return { static: "cant", what, action, condition, ...(conditionKind ? { conditionKind } : {}) };
    });
    if (cant) return cant;

    // "<subject> attacks each combat if able."
    const mustAttack = c.attempt((c): StaticEffect | null => {
      const what = parseStaticSubject(c);
      if (!what) return null;
      if (!c.words("attacks", "each", "combat", "if", "able")) return null;
      c.punct(".");
      if (!c.done() && !c.isWord("otherwise")) return null;
      return { static: "must-attack", what };
    });
    if (mustAttack) return mustAttack;

    // Anthem: "<subject> get(s)/have/gain(s) <modifiers> (as long as …)?"
    // A targeted subject or a one-shot duration ("until end of turn") means this
    // is resolution text, not a continuous ability — reject and let the spell
    // parser take it.
    const modify = c.attempt((c): StaticEffect | null => {
      const what = parseStaticSubject(c);
      if (!what || what.ref === "target") return null;
      const group = parseStaticModifiers(c, what);
      if (!group) return null;
      const binding = c.attempt((c) => {
        if (!c.punct(",") || !c.words("where", "x", "is")) return null;
        return parseAmount(c);
      });
      const protectionModifiers = parseProtectionAttachmentException(c, what, group.modifiers);
      c.punct(".");
      if (!c.done() && !c.isWord("otherwise")) return null;
      return intrinsicCharacteristicDefinition({ static: "modify", what, modifiers: protectionModifiers ?? group.modifiers, condition: group.condition, duration: undefined,
        ...(group.conditionScope ? { conditionScope: group.conditionScope } : {}),
        ...(binding ? { bindings: [{ variable: "x" as const, value: binding }] } : {}) });
    });
    if (modify) return modify;

    return c.fail("static ability");
  });
}

/** Alternative payment choices happen during casting (CR 118.9, 601.2b/f),
 * so the same sacrifice/exile wording must not become a resolving effect.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseAlternativeCost(c: Cursor): StaticEffect | null {
  return c.attempt(c => {
    const conditional = c.word("if") !== null;
    const condition = conditional ? parseCondition(c) : null;
    if (conditional && (!condition || !c.punct(","))) return null;
    if (!c.words("you", "may")) return null;
    const costs = parseCostList(c);
    if (!costs || !c.words("rather", "than", "pay")) return null;
    let spells: ObjectRef;
    if (c.words("the", "mana", "cost", "for")) {
      const what = parseObjectRef(c);
      if (!what) return null;
      spells = what;
    } else if (c.words("this") && c.possessive("spell") && c.words("mana", "cost")) {
      spells = { ref: "this", noun: "spell" };
    } else if (c.selfrefPossessive() && c.words("mana", "cost")) {
      spells = { ref: "self" };
    } else return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "alternative-cost", who: { player: "you" }, spells, costs, ...(condition ? { condition } : {}) };
  });
}
