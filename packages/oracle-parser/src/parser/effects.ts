// Effect sentence parsing: the imperative text of spells, trigger bodies,
// and activated-ability bodies.

import type {
  Ability, AbilityRemoval, Amount, CantAction, CounterSpec, DamageTarget, Duration, Effect, GainedAbility,
  ObjectFilter, ObjectRef, PlayerRef, Sentence, SignedAmount,
  ZoneRef, SpecifiedTokenSpec,
} from "../ast.ts";
import type { Token } from "../lexer.ts";
import { CARD_TYPES, COLORS, canonicalSubtypeName, singularize } from "../vocab.ts";
import { Cursor } from "./cursor.ts";
import { parseKeyword } from "./keywords.ts";
import { parseCostList } from "./costs.ts";
import { parseDelayedPrefix, parseDelayedSuffix } from "./delayed.ts";
import { parsePlayInstruction, parsePermissionSpellAbilities } from "./permissions.ts";
import { parseLookPermissionEffect } from "./persistent-rules.ts";
import { parseUntapSuppressionEffect } from "./untap.ts";
import { parseExileUntil } from "./linked-exile.ts";
import { parseAddManaEffect } from "./mana.ts";
import { parsePrevention, parsePreventionSentence } from "./prevention.ts";
import { parseCombatEffect } from "./combat.ts";
import { parseReplacementSentence } from "./replacement.ts";
import { parseTokenSpec } from "./token-spec.ts";
import { parseObjectActions } from "./object-actions.ts";
import { parseBattlefieldEntryDetails, hasExplicitSourceZone } from "./entry.ts";
import { parseUnlessPayment } from "./payment-choices.ts";
import { parseValueChoice } from "./choices.ts";
import { parseSelfReplacement } from "./self-replacement.ts";
import { parseModalEffect } from "./modal.ts";
import { parseProhibitionEffect } from "./prohibitions.ts";
import { foldCopyRetarget, parseStackCopy, parseChooseNewTargets, type StackCopyInputProof } from "./stack-copy.ts";
import { parseDiceRoll } from "./dice.ts";
import { parseSearchZones } from "./search.ts";
import { parseLibraryScan } from "./library-scan.ts";
import { parseForEach } from "./for-each.ts";
import { parseTemporal } from "./temporal.ts";
import { parseResolvingCharacteristics } from "./resolving-characteristics.ts";
import { parsePlayerGroupActor } from "./player-actors.ts";
import { hasCombatHistory, parseAggregateCount } from "./aggregates.ts";
import {
  parseAmount, parseAmountAdjustment, parseCondition, parseCount, parseCounterSpec,
  parseDuration, parseEqualTo, parseFilter, parseObjectRef, parseObjectSelection, parseCardExtraction, parsePlayerRef,
  parseZoneRef, isTargetedPlayerRef, isPluralPlayerRef,
} from "./refs.ts";

// Late-bound full-line parser, used for quoted abilities ("gains '{T}: …'").
// Registered by parser/index.ts to avoid a hard import cycle.
let lineParser: ((tokens: Token[], options: Token[][]) => Ability | null) | null = null;
export function registerLineParser(fn: (tokens: Token[], options: Token[][]) => Ability | null): void {
  lineParser = fn;
}

export interface EffectContext {
  /** Modal bullet options (token lists), when the line owns "Choose one —" bullets. */
  modalOptions: Token[][];
  /** Subject of the previous step, for "…and gains flying" with elided subject. */
  lastSubject?: ObjectRef;
  /** Actor of a coordinated conjugated verb, e.g. "opponent draws ... and discards". */
  lastPlayer?: PlayerRef;
  /** A token-specific immediate-trigger copy proof, never inherited by a
   * newly created/quoted body. The marker is immutable across backtracking. */
  stackCopyInput?: StackCopyInputProof;
  /** Only result-table row parsers set this; fresh nested bodies do not inherit it. */
  allowRollAgain?: boolean;
}

function containsEffect(value: unknown, kind: Effect["effect"]): boolean {
  if (Array.isArray(value)) return value.some(item => containsEffect(item, kind));
  if (!value || typeof value !== "object") return false;
  if ("effect" in value && value.effect === kind) return true;
  return Object.values(value).some(item => containsEffect(item, kind));
}

// ---------------------------------------------------------------------------
// Sentences
// ---------------------------------------------------------------------------

export function parseSentences(c: Cursor, ctx: EffectContext): Sentence[] | null {
  const previousSubject = ctx.lastSubject;
  const previousPlayer = ctx.lastPlayer;
  const result = c.attempt((c): Sentence[] | null => {
    const sentences: Sentence[] = [];
    while (!c.done()) {
      const lastSentence = sentences.at(-1);
      if (lastSentence) {
        const replacement = parseSelfReplacement(c, lastSentence, body => parseSentences(body, { modalOptions: ctx.modalOptions }));
        if (replacement) {
          lastSentence.steps = [replacement];
          continue;
        }
      }
      const previousStep = sentences[sentences.length - 1]?.steps.at(-1);
      if (containsEffect(previousStep, "grant-play-permission")) {
        const permission = previousStep?.effect === "grant-play-permission" ? parsePermissionSpellAbilities(c, previousStep.permission) : null;
        if (permission && previousStep?.effect === "grant-play-permission") {
          previousStep.permission = permission;
          continue;
        }
        const dependent = c.attempt(c => {
          if (!c.words("if", "you") || c.word("do", "don't") === null) return null;
          return true;
        });
        if (dependent) return c.fail("represented future permission-dependent instruction");
      }
      // "Otherwise, …" attaches to the previous sentence.
      if (sentences.length > 0 && c.attempt((c) => (c.word("otherwise") !== null ? true : null))) {
        const previous = sentences[sentences.length - 1];
        if (!previous.condition || previous.otherwise) return c.fail("one otherwise branch after a condition");
        c.punct(",");
        const steps = parseSteps(c, ctx);
        if (!steps) return null;
        if (!c.punct(".") && !c.done()) return null;
        const on = otherwiseScope(previous, steps);
        if (!on) return c.fail("unambiguous otherwise condition or action outcome");
        previous.otherwise = { on, steps };
        continue;
      }
      const previousSentence = sentences[sentences.length - 1];
      const previous = previousSentence?.steps.at(-1);
      const tokenDefinition = containsEffect(sentences, "create-token") && hasTokenDefinitionSubject(c);
      const beginsThen = c.isWord("then");
      const s = parseSentence(c, ctx);
      if (!s) break; // leave the cursor at the failed sentence; caller decides
      if (beginsThen && previousSentence?.condition && containsEffect(previousSentence.steps, "search") &&
          s.steps[0]?.effect === "shuffle" && s.steps[0].who.player === "that-player" && !s.condition) {
        if (!foldConditionalSearchShuffle(previousSentence, s)) return c.fail("represented conditional search/shuffle continuation");
        continue;
      }
      // CR 707.10c: choosing new targets for a just-created copy happens
      // before the copy is placed on the stack. Attach only an unambiguous
      // adjacent rider; keep other symbolic references explicitly unresolved.
      if (previousSentence?.steps.length === 1 && !previousSentence.condition && !previousSentence.otherwise && !previousSentence.bindings &&
          s.steps.length === 1 && !s.condition && !s.otherwise && !s.bindings &&
          !containsEffect(sentences.slice(0, -1), "copy-stack-object")) {
        const copy = foldCopyRetarget(previousSentence.steps[0], s.steps[0]);
        if (copy) { previousSentence.steps[0] = copy; continue; }
      }
      const regeneration = s.steps[0];
      if (regeneration?.effect === "cant" && regeneration.action === "be-regenerated") {
        if (!regeneration.duration) {
          if (previousSentence?.otherwise || s.steps.length !== 1 || s.condition || s.otherwise || s.bindings ||
              regeneration.optional || regeneration.forEach || !foldRegenerationRider(previous, regeneration.what))
            return c.fail("unambiguous preceding destruction for regeneration rider");
          continue;
        }
        if (containsEffect(previous, "destroy")) return c.fail("explicit scope of lasting regeneration prohibition");
      }
      if (tokenDefinition) {
        const definition = s.steps[0];
        // CR 111.3 / 707.2: "It has ..." completes the token's printed,
        // copiable characteristics; "It gains ..." is a resolving grant.
        // Keep temporary grants as effects. Unsupported definitions fail.
        if (definition?.effect === "gain-abilities" && definition.duration) {
          sentences.push(s);
          continue;
        }
        if (previous?.effect !== "create-token" || previousSentence?.steps.slice(0, -1).some(step => containsEffect(step, "create-token")) || previousSentence?.otherwise ||
            s.steps.length !== 1 || s.condition || s.otherwise || s.bindings ||
            definition?.effect !== "gain-abilities" ||
            definition.optional || definition.forEach || !("types" in previous.token))
          return c.fail("complete copiable token definition");
        if (!("ref" in definition.what) || !matchesCreatedTokens(definition.what, previous)) return c.fail("matching token-definition subject and cardinality");
        for (const ability of definition.abilities) {
          if (ability.gained === "keyword") previous.token.keywords.push(ability.keyword);
          else (previous.token.abilities ??= []).push(ability);
        }
        continue;
      }
      sentences.push(s);
    }
    return sentences.length ? sentences : null;
  });
  if (result === null) { ctx.lastSubject = previousSubject; ctx.lastPlayer = previousPlayer; }
  return result;
}

/** The searched player's shuffle completes this conditional instruction,
 * even when Oracle separates it with a full stop. Keep one condition check
 * around the entire sequence, not a reevaluation after changing the zones.
 * This is a dependent search-owner continuation, not a general `then` rule.
 * Deicide's official original wording and rulings:
 * https://magic.wizards.com/en/news/feature/release-notes-2014-04-23
 * CR608.2c: ordinary English requires reading the complete instruction. */
function foldConditionalSearchShuffle(previous: Sentence, next: Sentence): boolean {
  if (previous.conditionKind !== "if" || previous.otherwise || previous.bindings || previous.steps.length !== 2 ||
      next.condition || next.otherwise || next.bindings || next.steps.length !== 1) return false;
  const [search, exile] = previous.steps, shuffle = next.steps[0];
  if (search.effect !== "search" || search.optional || search.forEach || search.duration || search.zones.selection !== "all" ||
      exile.effect !== "exile" || exile.what.ref !== "them" || exile.optional || exile.forEach || exile.duration || exile.from ||
      shuffle.effect !== "shuffle" || shuffle.who.player !== "that-player" || shuffle.optional || shuffle.forEach || shuffle.duration ||
      shuffle.library.owner === undefined || JSON.stringify(shuffle.library.owner) !== JSON.stringify(shuffle.who)) return false;
  const library = search.zones.of.find(zone => zone.zone === "library");
  if (!library || !library.owner || typeof library.owner !== "object" || library.position ||
      search.zones.of.some(zone => zone.position || JSON.stringify(zone.owner) !== JSON.stringify(library.owner))) return false;
  previous.steps.push(shuffle);
  return true;
}

function hasTokenDefinitionSubject(c: Cursor): boolean {
  if (!c.isWord("it", "they", "that", "those")) return false;
  const offset = c.tokens.slice(c.pos).findIndex(t => t.kind === "word" && !t.possessive && ["has", "have"].includes(t.value));
  if (offset <= 0) return false;
  const subject = new Cursor(c.tokens.slice(c.pos, c.pos + offset));
  return parseObjectRef(subject) !== null && subject.done();
}

/** The plural set created for several players shares one token definition.
 * A noun identifying those tokens is checked against the stated token spec;
 * it cannot silently identify an unrelated existing permanent (CR 111.3). */
function matchesCreatedTokens(ref: ObjectRef, create: Extract<Effect, { effect: "create-token" }>): boolean {
  if (!("types" in create.token)) return false;
  const token = create.token;
  const singular = create.count.amount === "fixed" && create.count.value === 1 &&
    !(create.who && isPluralPlayerRef(create.who)) && !create.forEach;
  if (ref.ref === "it") return singular;
  if (ref.ref === "them") return !singular;
  if (ref.ref !== "that" || !!ref.plural === singular) return false;
  const f = ref.filter;
  return Object.keys(f).length > 0 && Object.keys(f).every(key => ["cls", "types", "subtypes"].includes(key)) &&
    (!f.cls || f.cls === "token" || f.cls === "permanent") &&
    (!f.types || f.types.every(type => token.types.includes(type))) &&
    (!f.subtypes || f.subtypes.every(type => token.subtypes.includes(type)));
}

/** Oracle's optional battlefield placement followed by another placement
 * tests whether the first action actually happened, not merely its condition.
 * Matter Reshaper explicitly covers failure for ANY reason; Aid from the
 * Cowl and Treasure Chest also cover declining an eligible placement.
 * https://magic.wizards.com/en/news/feature/oath-gatewatch-release-notes-2016-01-13
 * https://magic.wizards.com/en/news/feature/aether-revolt-release-notes-2017-01-06
 * https://magic.wizards.com/en/news/feature/adventures-forgotten-realms-release-notes-2021-07-09
 * Other optional/grouped actions require a separately established scope. */
function otherwiseScope(previous: Sentence, fallback: Effect[]): NonNullable<Sentence["otherwise"]>["on"] | null {
  const hasOptional = previous.steps.some(step => step.optional || step.effect === "optional-group");
  if (!hasOptional) return previous.conditionKind === "unless" ? "condition-true" : "condition-false";
  if (previous.conditionKind !== "if" || previous.steps.length !== 1 || fallback.length !== 1) return null;
  const action = previous.steps[0], alternative = fallback[0];
  if (action.effect !== "move-zone" || !action.optional || action.forEach || action.to.zone !== "battlefield" ||
      alternative.effect !== "move-zone" || alternative.forEach) return null;
  const cardAnaphor = (ref: ObjectRef) => ref.ref === "it" ||
    (ref.ref === "that" && !ref.plural && ref.filter.cls === "card" && Object.keys(ref.filter).length === 1);
  if (!cardAnaphor(action.what) || !cardAnaphor(alternative.what)) return null;
  return "action-not-performed";
}

/** CR 608.2c explicitly names this look-ahead interpretation. The ban applies
 * to the destruction event itself; it is not a later continuous effect.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function foldRegenerationRider(previous: Effect | undefined, what: ObjectRef): boolean {
  if (!previous) return false;
  const steps = previous.effect === "simultaneous" ? previous.steps : [previous];
  if (!steps.length || steps.some(step => step.effect !== "destroy")) return false;
  const destroys = steps as Extract<Effect, { effect: "destroy" }>[];
  const singular = (ref: ObjectRef): boolean => ["self", "this", "it", "enchanted", "equipped"].includes(ref.ref) ||
    (ref.ref === "that" && !ref.plural) || ("count" in ref && ref.count?.amount === "fixed" && ref.count.value === 1);
  const one = destroys.length === 1 && singular(destroys[0].what);
  let matches = what.ref === "it" ? one : what.ref === "them" ? !one : false;
  if (what.ref === "that" && destroys.length === 1 && !what.plural && one) {
    const base = destroys[0].what;
    const keys = Object.keys(what.filter);
    if (keys.length === 1 && what.filter.types?.length === 1) {
      const type = what.filter.types[0];
      matches = ("filter" in base && base.filter.types?.includes(type)) ||
        ("noun" in base && base.noun === type) || false;
    }
  }
  if (!matches) return false;
  for (const destroy of destroys) destroy.regenerationForbidden = true;
  return true;
}

function parseSentence(c: Cursor, ctx: EffectContext): Sentence | null {
  const previousSubject = ctx.lastSubject;
  const previousPlayer = ctx.lastPlayer;
  const result = c.attempt((c): Sentence | null => {
    // Elided subjects are local to a coordinated instruction, not a later
    // sentence that happens to have a compatible verb.
    ctx.lastSubject = undefined;
    ctx.lastPlayer = undefined;
    c.word("then");
    const sentence: Sentence = { steps: [] };

    // Leading duration: "Until end of turn, target creature gains flying."
    const leadingDuration = c.attempt((c) => {
      const d = parseDuration(c);
      if (!d || !c.punct(",")) return null;
      return d;
    });

    const prevention = parsePreventionSentence(c, ctx, parseSentences, { leadingDuration: leadingDuration ?? undefined });
    if (prevention) return prevention;
    const replacement = parseReplacementSentence(c, ctx, parseSentences, { leadingDuration: leadingDuration ?? undefined });
    if (replacement) return replacement;

    if (c.attempt((c) => (c.word("if") !== null ? true : null))) {
      const condition = parseConditionClause(c);
      if (!condition) return null;
      if (!c.punct(",")) return null;
      sentence.condition = condition;
      sentence.conditionKind = "if";
    }

    const steps = parseSteps(c, ctx, leadingDuration ?? undefined);
    if (!steps) return null;
    if (leadingDuration) {
      for (const s of steps) {
        if (!setDuration(s, leadingDuration)) return c.fail("duration on a continuous effect or permission");
      }
    }
    sentence.steps = steps;

    const binding = c.attempt((c) => {
      if (!c.punct(",") || !c.words("where", "x", "is")) return null;
      return parseAmount(c);
    });
    if (binding) sentence.bindings = [{ variable: "x", value: binding }];

    const tailCondition = c.word("unless", "if");
    if (tailCondition) {
      if (steps.length !== 1) return c.fail("unambiguous conditional instruction scope");
      const condition = (tailCondition === "unless" ? parseUnlessPayment(c) : null) ??
        parseCondition(c, tailCondition === "unless" ? { purpose: "unless" } : undefined);
      if (!condition) return null;
      if (sentence.condition) {
        sentence.condition = { condition: "all-of", conditions: [sentence.condition, tailCondition === "unless" ? { condition: "not", of: condition } : condition] };
        sentence.conditionKind = "if";
      } else {
        sentence.condition = condition;
        sentence.conditionKind = tailCondition as "unless" | "if";
      }
    }

    if (!c.punct(".") && !c.done()) return null;
    return sentence;
  });
  if (result === null) { ctx.lastSubject = previousSubject; ctx.lastPlayer = previousPlayer; }
  return result;
}

function parseConditionClause(c: Cursor) {
  return parseCondition(c);
}

function parseSteps(c: Cursor, ctx: EffectContext, leadingDuration?: Duration): Effect[] | null {
  const previousSubject = ctx.lastSubject;
  const previousPlayer = ctx.lastPlayer;
  const result = c.attempt((c): Effect[] | null => {
    const steps: Effect[] = [];
    const choices: { start: number; end: number }[] = [];
    let choice: { start: number; end: number } | undefined;
    let modifierGroupStart = 0;
    for (;;) {
      let sharedModifiers = false;
      let sharesChoice = false;
      let parsed: Effect[] | null;
      let optional: boolean;
      if (steps.length === 0) {
        optional = c.attempt((c) => (c.words("you", "may") ? true : null)) === true;
        parsed = parseStep(c, ctx, { optionalConsumed: optional, leadingDuration });
        if (!parsed) return null;
      } else {
        // Continuation requires a connector: ", (then|and)" / "then" / "and".
        const cont = c.attempt((c): { parsed: Effect[]; optional: boolean } | null => {
          if (c.isPunct(",")) {
            c.punct(",");
            if (c.word("then") === null) c.word("and");
          } else if (c.word("then") === null) {
            if (c.word("and") === null) return null;
            sharedModifiers = c.isWord("gain", "gains", "get", "gets", "has", "have", "lose", "loses", "become", "becomes");
          }
          const opt = c.attempt((c) => (c.words("you", "may") ? true : null)) === true;
          // A single "may" governs coordinated verbs with an elided actor.
          // A newly stated subject or another "may" begins a new instruction.
          sharesChoice = !opt && c.isWord("draw", "discard", "sacrifice", "pay", "gain", "lose", "create", "put", "return", "exile", "destroy", "counter", "tap", "untap", "search", "shuffle", "reveal", "add", "scry", "surveil", "mill", "transform", "convert", "attach", "unattach", "copy", "choose", "roll");
          const p = parseStep(c, ctx, { optionalConsumed: opt, leadingDuration });
          if (!p) return null;
          return { parsed: p, optional: opt };
        });
        if (!cont) break;
        parsed = cont.parsed;
        optional = cont.optional;
      }
      // In a lasting permission, "may" describes the permitted action; the
      // creation of that permission is mandatory (CR 611.2a).
      if (parsed.every(effect => effect.effect === "grant-play-permission" || effect.effect === "grant-look-permission")) optional = false;
      if (optional) {
        choice = { start: steps.length, end: steps.length };
        choices.push(choice);
      } else if (!sharesChoice) choice = undefined;
      if (!sharedModifiers) modifierGroupStart = steps.length;
      // A rider on a multi-object instruction applies to every emitted effect.
      // Elided coordinated modifiers share their trailing duration (CR 611.2a).
      // https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt
      const last = parsed[parsed.length - 1];
      attachRiders(c, parsed);
      for (const e of parsed) {
        if (optional && e.effect !== "grant-play-permission" && e.effect !== "grant-look-permission") e.optional = true;
        if (last.duration) e.duration = last.duration;
        if (last.forEach) e.forEach = last.forEach;
        if (sharedModifiers && last.duration) {
          for (let i = steps.length - 1; i >= modifierGroupStart; i--) {
            const previous = steps[i];
            if (!["pump", "gain-abilities", "player-gain-abilities", "lose-abilities", "set-pt", "become", "modify-characteristics", "combat-rule", "cant", "untap-step-suppression"].includes(previous.effect)) break;
            if (previous.duration) break;
            previous.duration = last.duration;
          }
        }
      }
      // A coordinated multi-object instruction is simultaneous (CR 608.2c);
      // separate instructions joined by "then" remain separate steps.
      if (parsed.length > 1) {
        const children = parsed.map(({ optional: _optional, ...step }) => step);
        parsed = [{ effect: "simultaneous", steps: children, ...(optional ? { optional: true as const } : {}) }];
      }
      const delayed = parseDelayedSuffix(c, parsed);
      if (delayed) parsed = delayed;
      steps.push(...parsed);
      if (choice) choice.end = steps.length;
    }
    // Preserve one choice for "may A and B", not independent optional A/B.
    for (const group of choices.reverse()) {
      if (group.end - group.start < 2) continue;
      const children = steps.slice(group.start, group.end).map(({ optional: _optional, ...step }) => step);
      steps.splice(group.start, group.end - group.start, { effect: "optional-group", who: { player: "you" }, steps: children });
    }
    return steps.length ? steps : null;
  });
  if (result === null) { ctx.lastSubject = previousSubject; ctx.lastPlayer = previousPlayer; }
  return result;
}

/** An aggregate scales a quantity within the same event. It must never turn
 * one damage/token/life instruction into a sequence of smaller events. Mana
 * whose type depends on an iterated color needs a value iterator, not this
 * scalar production (CR 608.2c/h). */
function canScaleQuantity(e: Effect): boolean {
  return ["draw", "mill", "gain-life", "lose-life", "damage", "create-token", "get-counters", "put-counters", "pump"].includes(e.effect);
}
function scaleQuantity(e: Effect, multiplier: Amount): void {
  const product = (amount: Amount): Amount => ({ amount: "product", factors: [amount, multiplier] });
  switch (e.effect) {
    case "draw": case "mill": case "gain-life": case "lose-life": case "damage": e.amount = product(e.amount); break;
    case "create-token": case "get-counters": case "put-counters": e.count = product(e.count); break;
    case "pump":
      e.power = { ...e.power, amount: product(e.power.amount) };
      e.toughness = { ...e.toughness, amount: product(e.toughness.amount) };
      break;
  }
}
function attachRiders(c: Cursor, effects: Effect[]): void {
  const e = effects[effects.length - 1];
  let scaled = false;
  for (;;) {
    if (!scaled && !e.forEach && effects.every(canScaleQuantity)) {
      const aggregate = c.attempt(c => c.words("for", "each")
        ? parseAggregateCount(c, { parseObjectRef }, { singular: true }) : null);
      if (aggregate) {
        effects.forEach(effect => scaleQuantity(effect, aggregate));
        scaled = true;
        continue;
      }
    }
    if (!scaled && !e.forEach) {
      const forEach = c.attempt((c) => {
        if (!c.words("for", "each")) return null;
        const filter = parseFilter(c);
        return filter && !hasCombatHistory(filter) ? filter : null;
      });
      if (forEach) {
        e.forEach = forEach;
        continue;
      }
    }
    if (!e.duration) {
      const duration = c.attempt(c => {
        const duration = parseDuration(c);
        return duration && setDuration(e, duration) ? duration : null;
      });
      if (duration) {
        e.duration = duration;
        continue;
      }
    }
    break;
  }
}

/** One-shot actions do not acquire continuous-effect semantics just because
 * a duration follows them. Delayed triggers with a stated lifetime recur
 * during that lifetime rather than firing only once (CR 603.7b, 611.2a).
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function setDuration(effect: Effect, duration: Duration): boolean {
  if (effect.effect === "simultaneous" || effect.effect === "optional-group") {
    const children = structuredClone(effect.steps);
    if (!children.every(step => setDuration(step, duration))) return false;
    effect.steps = children;
    return true;
  }
  const continuous = new Set(["pump", "set-pt", "gain-abilities", "player-gain-abilities", "lose-abilities", "gain-control", "become", "modify-characteristics", "cant", "combat-rule", "prevent-damage", "create-replacement", "untap-step-suppression", "grant-play-permission", "grant-look-permission", "prohibit", "cast-limit", "skip-time"]);
  if (effect.effect !== "create-delayed-trigger" && !continuous.has(effect.effect)) return false;
  if (effect.duration && JSON.stringify(effect.duration) !== JSON.stringify(duration)) return false;
  if (effect.effect === "create-delayed-trigger") effect.occurrence = effect.nextOnly ? "next" : "each";
  effect.duration = duration;
  return true;
}

// ---------------------------------------------------------------------------
// Individual effects
// ---------------------------------------------------------------------------

interface StepOptions { optionalConsumed?: boolean; leadingDuration?: Duration }
type StepParser = (c: Cursor, ctx: EffectContext, options?: StepOptions) => Effect[] | null;

/** Every step production has the same rollback contract, including context. */
function transactionalStep(parser: StepParser): StepParser {
  return (c, ctx, options) => {
    const previousSubject = ctx.lastSubject;
    const previousPlayer = ctx.lastPlayer;
    const result = c.attempt(() => parser(c, ctx, options));
    if (result === null) { ctx.lastSubject = previousSubject; ctx.lastPlayer = previousPlayer; }
    return result;
  };
}

function parseStep(c: Cursor, ctx: EffectContext, options?: StepOptions): Effect[] | null {
  const previousSubject = ctx.lastSubject;
  const previousPlayer = ctx.lastPlayer;
  const result = c.attempt((c): Effect[] | null => {
    if (options?.optionalConsumed) ctx.lastPlayer = { player: "you" };
    // Causative "may have [object] deal/become ..." has the same optional
    // resolution action; "have" is not a gained-ability instruction here.
    if (options?.optionalConsumed && c.word("have") !== null) {
      const subjectStart = c.pos;
      if (!parseObjectRef(c)) return null;
      c.pos = subjectStart;
    }
    for (const p of STEP_PARSERS) {
      const r = p(c, ctx, options);
      if (r) return r;
    }
    return c.fail("effect");
  });
  if (result === null) { ctx.lastSubject = previousSubject; ctx.lastPlayer = previousPlayer; }
  return result;
}

/** "target creature and target land" → list; single ref → [ref]. */
function parseObjectRefList(c: Cursor, parseSelection = parseObjectSelection): ObjectRef[] | null {
  return c.attempt((c): ObjectRef[] | null => {
    const first = parseSelection(c);
    if (!first) return null;
    const refs = [first];
    while (
      c.attempt((c) => {
        if (c.word("and") === null) return null;
        const nxt = parseSelection(c);
        if (!nxt) return null;
        refs.push(nxt);
        return true;
      })
    ) { /* keep going */ }
    // A slot group owns the entire coordinated card extraction. Combining
    // it with a second ordinary list would erase branch/source scope when
    // its parser rejected mixed conjunctions in the full noun phrase.
    if (refs.length > 1 && refs.some(ref => ref.ref === "card-extraction-group"))
      return c.fail("one complete coordinated card-extraction group");
    const cardFilters = refs.flatMap(ref => "filter" in ref && ref.filter.cls === "card" ? [ref.filter] : []);
    if (cardFilters.some(hasSelectionPool) && cardFilters.some(filter => !hasSelectionPool(filter)))
      return c.fail("unambiguous source scope for coordinated card selections");
    return refs;
  });
}

function hasSelectionPool(filter: ObjectFilter): boolean {
  return !!(filter.zone || filter.fromAmong || filter.relation ||
    filter.allOf?.some(hasSelectionPool) || filter.anyOf?.every(hasSelectionPool));
}

/** Imperative verbs act for you; coordinated conjugated verbs retain their
 * stated actor. A sentence boundary resets that elision context. */
function parseActor(c: Cursor, ctx: EffectContext, imperative: string, conjugated: string, allowGroup = false): PlayerRef | null {
  return c.attempt(c => {
    const explicit = (allowGroup ? parsePlayerGroupActor(c, parsePlayerRef) : null) ?? parsePlayerRef(c);
    const verb = c.word(imperative, conjugated);
    if (!verb) return null;
    const retainsPluralActor = ctx.lastPlayer?.player === "they" || ctx.lastPlayer?.player === "group";
    const who = explicit ?? (verb === imperative && !retainsPluralActor ? { player: "you" as const } : ctx.lastPlayer);
    if (!who) return c.fail("actor of conjugated effect");
    if (who.player === "group" && !allowGroup) return c.fail("supported grouped-player action");
    if (who.player === "group" && verb !== imperative) return c.fail("plural grouped-player verb");
    ctx.lastPlayer = who;
    return who;
  });
}

const parseDraw: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "draw", "draws", true);
  if (!who) return null;
  let amount = c.attempt(parseAmount);
  if (c.word("card", "cards") === null) return null;
  if (!amount) amount = parseEqualTo(c);
  if (!amount) return null;
  amount = parseAmountAdjustment(c, amount);
  if (!amount) return null;
  return [{ effect: "draw", who, amount }];
});

// Energy/ticket symbols represent counters on players (CR 107.14, 107.17,
// 122). Emblems live in the command zone with the stated abilities (CR 114).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const parseGet: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "get", "gets");
  if (!who) return null;
  const symbolCounters = c.attempt((c): Effect[] | null => {
    const symbol = c.symbol();
    if (symbol !== "E" && symbol !== "TK") return null;
    let count = 1;
    for (;;) {
      const next = c.peek();
      if (next?.kind !== "symbol" || next.value !== symbol) break;
      c.pos++; count++;
    }
    return [{ effect: "get-counters", who, counter: { type: symbol === "E" ? "energy" : "ticket" }, count: { amount: "fixed", value: count } }];
  });
  if (symbolCounters) return symbolCounters;
  if (c.words("an", "emblem", "with")) {
    const abilities = parseGainedAbilities(c, { allowFreshProtectionChoices: false });
    return abilities ? [{ effect: "get-emblem", who, abilities }] : null;
  }
  const count = parseCount(c);
  if (!count) return null;
  const counter = parseCounterSpec(c);
  if (!counter || c.word("counter", "counters") === null) return null;
  return [{ effect: "get-counters", who, counter, count }];
});

const parseChoose: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "choose", "chooses");
  if (!who) return null;
  const value = parseValueChoice(c);
  if (value) return [{ effect: "choose-value", who, ...value }];
  const what = c.attempt(parsePlayerRef) ?? parseObjectSelection(c);
  if (!what || ("ref" in what && what.ref === "each")) return null;
  const targeted = "ref" in what ? what.ref === "target" || what.ref === "any-target" : isTargetedPlayerRef(what);
  if (targeted && who.player !== "you") return null;
  return targeted ? [{ effect: "declare-target", what }] : [{ effect: "choose-object", who, what }];
});

const parseDamage: StepParser = transactionalStep((c) => {
  const source = parseObjectRef(c);
  if (!source) return null;
  if (c.word("deal", "deals") === null) return null;
  let amount = c.attempt(parseAmount);
  if (c.word("damage") === null) return null;
  if (!amount) {
    amount = parseEqualTo(c);
  }
  const divided = c.words("divided", "as", "you", "choose", "among");
  if (divided) {
    if (!amount) return null;
    const among = parseObjectRef(c);
    if (!among || (among.ref !== "target" && among.ref !== "any-target")) return null;
    return [{ effect: "damage", source, amount, to: { to: "divided", among } }];
  }
  if (c.word("to") === null) return null;
  const to = parseDamageTarget(c);
  if (!to) return null;
  if (!amount) amount = parseEqualTo(c);
  if (!amount) return null;
  const effects: Effect[] = [{ effect: "damage", source, amount, to }];
  // One source dealing different quantities to several recipients is one
  // simultaneous damage instruction, distinct from two sequential deals.
  for (;;) {
    const additional = c.attempt((c): Effect | null => {
      if (c.word("and") === null) return null;
      const amount = parseAmount(c);
      if (!amount || !c.words("damage", "to")) return null;
      const to = parseDamageTarget(c);
      return to ? { effect: "damage", source, amount, to } : null;
    });
    if (!additional) break;
    effects.push(additional);
  }
  return effects;
});

function parseDamageTarget(c: Cursor): DamageTarget | null {
  return c.attempt((c): DamageTarget | null => {
    const recipient = (c: Cursor): PlayerRef | ObjectRef | null =>
      c.attempt(c => {
        const player = parsePlayerRef(c);
        return player && !c.isWord("or") ? player : null;
      }) ?? parseObjectRef(c);
    const first = recipient(c);
    if (!first) return null;
    const refs = [first];
    for (;;) {
      const next = c.attempt(c => {
        if (c.word("and") === null) return null;
        const next = recipient(c);
        // An explicitly stated actor starts the following instruction, not
        // another damage recipient: "...to a creature and you gain 2 life."
        if (next && "player" in next && c.isWord("may", "gain", "gains", "lose", "loses", "draw", "draws", "discard", "discards", "sacrifice", "sacrifices", "create", "creates", "put", "puts", "exile", "exiles", "return", "returns", "tap", "taps", "untap", "untaps", "reveal", "reveals", "shuffle", "shuffles")) return null;
        return next;
      });
      if (!next) break;
      refs.push(next);
    }
    if (refs.length > 1) return { to: "each", refs };
    if ("player" in first) return { to: "player", ref: first };
    return first.ref === "any-target" && !first.count
      ? { to: "any-target", ...(first.selectionId ? { selectionId: first.selectionId } : {}) }
      : { to: "object", ref: first };
  });
}

const parseDestroy: StepParser = transactionalStep((c) => {
  if (c.word("destroy") === null) return null;
  const refs = parseObjectRefList(c);
  if (!refs) return null;
  return refs.map((what): Effect => ({ effect: "destroy", what }));
});

const parseExile: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "exile", "exiles");
  if (!who) return null;
  const actor = who.player === "you" ? {} : { who };
  const refs = parseObjectRefList(c);
  if (!refs) return null;
  const from = c.attempt(c => c.word("from") !== null ? parseZoneRef(c) : null);
  if (c.isWord("until")) {
    const linked = parseExileUntil(c, refs, from ?? undefined);
    return linked ? [{ ...linked, ...actor }] : null;
  }
  return refs.map((what): Effect => ({ effect: "exile", what, ...(from ? { from } : {}), ...actor }));
});

const parseCounterSpell: StepParser = transactionalStep((c) => {
  if (c.word("counter") === null) return null;
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "counter", what }];
});

const parseReturn: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "return", "returns");
  if (!who) return null;
  const actor = who.player === "you" ? {} : { who };
  const refs = parseObjectRefList(c, parseCardExtraction);
  if (!refs) return null;
  const from = c.attempt(c => c.word("from") !== null ? parseZoneRef(c) : null);
  if (c.word("to") === null) return null;
  const toHand = c.attempt((c): "your" | "owner" | null => {
    let to: "your" | "owner";
    if (c.word("your") !== null) to = "your";
    else {
      if (c.word("their", "its") === null || c.possessive("owner", "owners") === null) return null;
      to = "owner";
    }
    return c.word("hand", "hands") !== null ? to : null;
  });
  if (toHand) return refs.map((what): Effect => ({ effect: "return-to-hand", what, to: toHand, ...(from ? { from } : {}), ...actor }));
  if (c.words("the", "battlefield")) {
    const details = parseBattlefieldEntryDetails(c);
    if (!details || (details.from && (from || refs.some(hasExplicitSourceZone)))) return null;
    return refs.map((what): Effect => ({ effect: "move-zone", what, to: { zone: "battlefield" },
      ...details, ...(from ? { from } : {}), ...actor }));
  }
  return null;
});

const parseCreateToken: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "create", "creates", true);
  if (!who) return null;
  const literal = c.peek()?.kind === "card-name" ? c.next() : undefined;
  if (literal && !c.punct(",")) return null;
  const dynamic = c.words("a", "number", "of");
  let count: Amount | null = dynamic ? null : parseAmount(c);
  if (!dynamic && !count) return null;
  const token = parseTokenSpec(c, c => parseGainedAbilities(c, { allowFreshProtectionChoices: false }), parseTokenSpecLike);
  if (!token) return null;
  if (literal?.kind === "card-name") {
    if (!("types" in token) || token.name) return null;
    token.name = literal.value;
  }
  if (dynamic) count = parseEqualTo(c);
  if (!count) return null;
  return [{ effect: "create-token", count, token, ...(who.player !== "you" ? { who } : {}) }];
});

const parseTapUntap: StepParser = transactionalStep((c, ctx) => {
  const tapped = parseActor(c, ctx, "tap", "taps");
  const who = tapped ?? parseActor(c, ctx, "untap", "untaps");
  if (!who) return null;
  const refs = parseObjectRefList(c);
  if (!refs) return null;
  return refs.map((what): Effect => ({ effect: tapped ? "tap" : "untap", what, ...(who.player === "you" ? {} : { who }) }));
});

const parseSacrifice: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "sacrifice", "sacrifices");
  if (!who) return null;
  const what = parseObjectRef(c);
  if (!what) return null;
  const chosenBy = c.attempt((c): PlayerRef | null => {
    if (c.word("of") === null) return null;
    const whose = c.word("your", "their");
    if (!whose || c.word("choice") === null) return null;
    return whose === "your" ? { player: "you" } : who;
  });
  return [{ effect: "sacrifice", who, what, ...(chosenBy ? { chosenBy } : {}) }];
});

const parseDiscard: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "discard", "discards");
  if (!who) return null;
  if (c.attempt((c) => (c.words("your", "hand") || c.words("their", "hand") || c.words("their", "hands") ? true : null)))
    return [{ effect: "discard", who, what: "hand" }];
  const what = parseObjectRef(c);
  if (!what) return null;
  const random = c.attempt((c) => (c.words("at", "random") ? true : null)) === true;
  return [{ effect: "discard", who, what, random: random || undefined }];
});

const parseMill: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "mill", "mills");
  if (!who) return null;
  const amount = parseAmount(c);
  if (!amount) return null;
  if (c.word("card", "cards") === null) return null;
  return [{ effect: "mill", who, amount }];
});

const parseScrySurveil: StepParser = transactionalStep((c) => {
  const verb = c.word("scry", "surveil");
  if (!verb) return null;
  const amount = parseCount(c);
  if (!amount) return null;
  return [{ effect: verb as "scry" | "surveil", amount }];
});

// Keyword actions have closed, rules-defined semantics (CR 701). Keeping
// their names typed is analogous to keyword abilities, not a raw-text escape.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const parseKeywordAction: StepParser = transactionalStep((c) => {
  const plain = c.word("proliferate", "populate", "investigate");
  if (plain) return [{ effect: "keyword-action", action: plain as "proliferate" | "populate" | "investigate", who: { player: "you" } }];
  const counted = c.word("discover", "adapt", "monstrosity", "bolster", "support");
  if (counted) {
    const amount = parseCount(c);
    if (!amount) return null;
    if (counted === "discover") return [{ effect: "discover", who: { player: "you" }, amount }];
    return [{ effect: counted as "adapt" | "monstrosity" | "bolster" | "support", amount }];
  }
  if (c.word("amass") !== null) {
    const word = c.anyWord();
    const subtype = word && canonicalSubtypeName(word.raw);
    if (!subtype) return null;
    const amount = parseCount(c);
    return amount ? [{ effect: "amass", subtype, amount }] : null;
  }
  const what = parseObjectRef(c);
  if (!what) return null;
  const verb = c.word("explore", "explores", "connive", "connives");
  if (!verb) return null;
  return [{ effect: verb.startsWith("explor") ? "explore" : "connive", what }];
});

const parsePay: StepParser = transactionalStep((c) => {
  const who = c.attempt(parsePlayerRef) ?? { player: "you" as const };
  if (c.word("pay", "pays") === null) return null;
  const costs = parseCostList(c);
  if (costs) return [{ effect: "pay-cost", who, costs }];
  const amount = parseAmount(c);
  if (!amount || c.word("life") === null) return null;
  return [{ effect: "pay-cost", who, costs: [{ cost: "pay-life", amount }] }];
});

const parsePut: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "put", "puts");
  if (!who) return null;
  const actor = who.player === "you" ? {} : { who };

  // Each shared put instruction is simultaneous; individual counter kinds
  // and amounts remain distinct. CR 122.6 / 608.2c (see source below).
  const counters = parseCounterGroup(c);
  if (counters) {
    for (;;) {
      const next = c.attempt(c => {
        const comma = c.punct(",");
        const and = c.word("and") !== null;
        if (!comma && !and) return null;
        return parseCounterGroup(c);
      });
      if (!next) break;
      counters.push(...next);
    }
    return counters.map(counter => ({ ...counter, ...actor }));
  }

  // Shared put verb: each group has its own destination. All groups remain
  // one instruction, unlike an explicit "then put" (CR 608.2c).
  // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
  const first = parsePutGroup(c);
  if (!first) return null;
  const moves = first;
  for (;;) {
    const next = c.attempt(c => {
      c.punct(",");
      if (c.word("and") === null) return null;
      return parsePutGroup(c);
    });
    if (!next) break;
    moves.push(...next);
  }
  return moves.map(move => ({ ...move, ...actor }));
});

function parseCounterGroup(c: Cursor): Extract<Effect, { effect: "put-counters" }>[] | null {
  return c.attempt(c => {
    const dynamic = c.words("a", "number", "of");
    const spec = (c: Cursor): { counter: CounterSpec; count: Amount } | null => c.attempt(c => {
      // "another counter" adds one counter; it is not another selected object.
      const count: Amount | null = c.word("another") !== null ? { amount: "fixed", value: 1 } : parseAmount(c);
      const counter = count && parseCounterSpec(c);
      return count && counter && c.word("counter", "counters") !== null ? { counter, count } : null;
    });
    if (dynamic) {
      const counter = parseCounterSpec(c);
      if (!counter || c.word("counter", "counters") === null) return null;
      let count = parseEqualTo(c);
      if (c.word("on") === null) return null;
      const eachOf = c.words("each", "of");
      const recipients = eachOf ? parseObjectRefList(c) : (() => { const ref = parseObjectRef(c); return ref ? [ref] : null; })();
      if (!recipients) return null;
      count ??= parseEqualTo(c);
      return count ? recipients.map(on => ({ effect: "put-counters", on, count: count!, counter })) : null;
    }
    const first = spec(c);
    if (!first) return null;
    const specs = [first];
    for (;;) {
      const next = c.attempt(c => {
        const comma = c.punct(",");
        const and = c.word("and") !== null;
        if (!comma && !and) return null;
        return spec(c);
      });
      if (!next) break;
      specs.push(next);
    }
    if (c.word("on") === null) return null;
    const eachOf = c.words("each", "of");
    const recipients = eachOf ? parseObjectRefList(c) : (() => { const ref = parseObjectRef(c); return ref ? [ref] : null; })();
    if (!recipients) return null;
    return recipients.flatMap(on => specs.map(spec => ({ effect: "put-counters" as const, on, ...spec })));
  });
}

type ZoneMove = Extract<Effect, { effect: "move-zone" }>;
function parsePutGroup(c: Cursor): ZoneMove[] | null {
  return c.attempt(c => {
    const refs = parseObjectRefList(c, parseCardExtraction);
    if (!refs || c.word("onto", "into", "on", "in") === null) return null;
    const to = parseZoneRef(c);
    if (!to) return null;
    const details = parseZoneMoveDetails(c, to);
    if (!details || (details.from && refs.some(hasExplicitSourceZone))) return null;
    return refs.map(what => ({ effect: "move-zone", what, to, ...details }));
  });
}
function parseZoneMoveDetails(c: Cursor, to: ZoneRef): Pick<ZoneMove, "tapped" | "controller" | "order" | "from" | "entry"> | null {
  return c.attempt(c => {
    if (to.zone === "battlefield") return parseBattlefieldEntryDetails(c);
    const order = c.attempt(c => {
      if (c.word("in") === null) return null;
      if (c.words("any", "order")) return "any" as const;
      if (c.words("a", "random", "order") || c.words("random", "order")) return "random" as const;
      return null;
    });
    if (order && !to.position) return null;
    return { ...(order ? { order } : {}) };
  });
}

const parseAddMana: StepParser = transactionalStep((c, ctx) => {
  const result = parseAddManaEffect(c, ctx.lastPlayer);
  if (!result) return null;
  ctx.lastPlayer = result.actor;
  return result.effects;
});

const parseSearch: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "search", "searches");
  if (!who) return null;
  const zones = parseSearchZones(c);
  if (!zones) return null;
  if (c.word("for") === null) return null;
  const what = parseCardExtraction(c);
  if (!what) return null;
  return [{ effect: "search", who, zones, for: what }];
});

const parseShuffle: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "shuffle", "shuffles");
  if (!who) return null;
  const into = c.attempt((c): Effect | null => {
    const first = parseObjectRef(c);
    if (!first) return null;
    const what = [first];
    for (;;) {
      const next = c.attempt((c): ObjectRef | null => {
        const comma = c.punct(",");
        const and = c.word("and") !== null;
        if (!comma && !and) return null;
        const explicit = parseObjectRef(c);
        if (explicit) return explicit;
        // Shared possessive in "their hand and graveyard" applies to both
        // zones. Do not infer an owner for an unrelated bare card selection.
        const previous = what.at(-1)!;
        if (previous.ref !== "zone-cards") return null;
        const zone = c.word("hand", "graveyard") as "hand" | "graveyard" | null;
        return zone ? { ref: "zone-cards", zone, owner: previous.owner } : null;
      });
      if (!next) break;
      what.push(next);
    }
    if (c.word("into") === null) return null;
    const library = parseZoneRef(c);
    if (library?.zone !== "library" || library.position || !library.owner) return null;
    const last = c.tokens[c.pos - 1];
    if (last?.kind === "word" && last.value === "libraries") return null; // per-owner grouping remains unsupported
    return { effect: "shuffle-into-library", who, what, library };
  });
  if (into) return [into];
  const library = parseZoneRef(c) ?? { zone: "library" as const, owner: who };
  if (library.zone !== "library" || library.position) return null;
  return [{ effect: "shuffle", who, library }];
});

const parseLifeGainLoss: StepParser = transactionalStep((c, ctx) => {
  const gained = c.attempt(c => parseActor(c, ctx, "gain", "gains", true));
  const who = gained ?? parseActor(c, ctx, "lose", "loses", true);
  if (!who) return null;
  let amount = c.attempt(parseAmount);
  if (c.word("life") === null) return null;
  if (!amount) {
    amount = parseEqualTo(c);
    if (!amount) return null;
  }
  const effect = gained ? "gain-life" : "lose-life";
  return [{ effect, who, amount }];
});

const parseGainControl: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "gain", "gains");
  if (!who) return null;
  if (!c.words("control", "of")) return null;
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "gain-control", who, what }];
});

function parseSignedAmount(c: Cursor): SignedAmount | null {
  return c.attempt((c): SignedAmount | null => {
    const sign = c.punct("+") ? 1 : c.punct("-") ? -1 : null;
    if (sign === null) return null;
    const amount = parseAmount(c);
    if (!amount) return null;
    return { sign: sign as 1 | -1, amount };
  });
}

/** "+1/+1", "-2/-0", "+X/+X" */
export function parsePtModifier(c: Cursor): { power: SignedAmount; toughness: SignedAmount } | null {
  return c.attempt((c) => {
    const power = parseSignedAmount(c);
    if (!power || !c.punct("/")) return null;
    const toughness = parseSignedAmount(c);
    if (!toughness) return null;
    return { power, toughness };
  });
}

/** keyword-or-quoted ability list after gains/has. */
export function parseGainedAbilities(c: Cursor, options: { allowFreshProtectionChoices: boolean } = { allowFreshProtectionChoices: true }): GainedAbility[] | null {
  const parseOne = (c: Cursor): GainedAbility | null => c.attempt(c => {
    if (c.punct('"')) {
      const inner: Token[] = [];
      while (!c.done() && !c.isPunct('"')) inner.push(c.next()!);
      if (!c.punct('"') || !lineParser) return null;
      const ability = lineParser(inner, []);
      return ability ? { gained: "quoted", ability } : null;
    }
    const keyword = parseKeyword(c, options);
    return keyword ? { gained: "keyword", keyword } : null;
  });
  return c.attempt(c => {
    const first = parseOne(c);
    if (!first) return null;
    const list = [first];
    for (;;) {
      const next = c.attempt(c => {
        if (c.punct(",")) c.word("and");
        else if (c.word("and") === null) return null;
        return parseOne(c);
      });
      if (!next) break;
      list.push(next);
    }
    return list;
  });
}

export function parseBasePt(c: Cursor): { power: Amount; toughness: Amount } | null {
  return c.attempt(c => {
    if (!c.words("base", "power", "and", "toughness")) return null;
    const power = parseAmount(c);
    if (!power || !c.punct("/")) return null;
    const toughness = parseAmount(c);
    return toughness ? { power, toughness } : null;
  });
}

/** CR 613.1f: removing all abilities or specified abilities, not suppressing
 * their effects retroactively. Choices are made by the stated player.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export function parseAbilityRemoval(c: Cursor): AbilityRemoval | null {
  return c.attempt((c): AbilityRemoval | null => {
    if (c.words("all", "abilities")) return { removal: "all" };
    if (c.words("your", "choice", "of")) {
      const first = parseKeyword(c);
      if (!first) return null;
      const options = [first];
      let final = false;
      while (!final) {
        if (c.punct(",")) final = c.word("or") !== null;
        else if (c.word("or") !== null) final = true;
        else return null;
        const next = parseKeyword(c);
        if (!next) return null;
        options.push(next);
      }
      return { removal: "choice", who: { player: "you" }, count: { amount: "fixed", value: 1 }, options };
    }
    const abilities = parseGainedAbilities(c, { allowFreshProtectionChoices: false });
    return abilities ? { removal: "listed", abilities } : null;
  });
}

/** Player ability grants are distinct from life gain and object modifiers.
 * Require the player to be printed (or the explicit "you may" already read by
 * the sentence parser); an elided plural "gain" may refer to objects. */
const parsePlayerGainAbilities: StepParser = transactionalStep((c, ctx, options) => {
  const explicit = parsePlayerRef(c);
  const verb = c.word("gain", "gains");
  if (!verb || (!explicit && (!options?.optionalConsumed || verb !== "gain"))) return null;
  const who = explicit ?? { player: "you" as const };
  const abilities = parseGainedAbilities(c);
  if (!abilities) return null;
  if (who.player === "they") {
    // A bare pronoun cannot choose a player/object semantic domain. Preserve
    // it for binding; neither the last noun nor parser ordering proves it.
    // CR 109.1, 113.1: players are not objects and both may have abilities.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    delete ctx.lastSubject;
    delete ctx.lastPlayer;
    return [{ effect: "gain-abilities", what: { recipient: "they" }, abilities }];
  }
  ctx.lastPlayer = who;
  return [{ effect: "player-gain-abilities", who, abilities }];
});

/** Subject-first characteristic modifiers, with a shared elided subject. */
const parsePumpOrGain: StepParser = transactionalStep((c, ctx) => {
  let what = c.attempt(parseObjectRef);
  if (!what) {
    // elided subject: "…and gains flying until end of turn"
    if (!ctx.lastSubject) return null;
    what = ctx.lastSubject;
  }
  const verb = c.word("get", "gets", "gain", "gains", "has", "have", "lose", "loses");
  if (!verb) return null;
  if (verb.startsWith("get")) {
    const pt = parsePtModifier(c);
    if (!pt) return null;
    ctx.lastSubject = what;
    return [{ effect: "pump", what, power: pt.power, toughness: pt.toughness }];
  }
  if (verb === "has" || verb === "have") {
    const pt = parseBasePt(c);
    if (pt) {
      ctx.lastSubject = what;
      return [{ effect: "set-pt", what, ...pt }];
    }
  }
  if (verb === "lose" || verb === "loses") {
    const removal = parseAbilityRemoval(c);
    if (!removal) return null;
    ctx.lastSubject = what;
    return [{ effect: "lose-abilities", what, removal }];
  }
  const abilities = parseGainedAbilities(c);
  if (!abilities) return null;
  ctx.lastSubject = what;
  return [{ effect: "gain-abilities", what, abilities }];
});

const parseBecome: StepParser = transactionalStep((c, ctx) => {
  const characteristic = parseResolvingCharacteristics(c, ctx.lastSubject, parseGainedAbilities);
  if (characteristic) {
    ctx.lastSubject = characteristic.what;
    return [characteristic];
  }
  const what = parseObjectRef(c) ?? ctx.lastSubject;
  if (!what) return null;
  if (c.word("becomes", "become") === null) return null;
  c.word("a", "an");
  if (c.attempt((c) => (c.words("copy", "of") ? true : null))) {
    const copyOf = parseObjectRef(c);
    if (!copyOf) return null;
    ctx.lastSubject = what;
    return [{ effect: "become", what, spec: { copyOf } }];
  }
  // "a 4/4 green Bear creature (in addition to its other types)"
  const spec = parseTokenSpecLike(c);
  if (!spec) return null;
  const inAddition = c.attempt((c) => {
    if (!c.words("in", "addition", "to") || c.word("its", "their") === null || c.word("other") === null) return null;
    return c.word("types", "colors") as "types" | "colors" | null;
  });
  if (inAddition === "types" && !spec.types?.length && !spec.subtypes?.length) return null;
  if (inAddition === "colors" && !spec.colors?.length) return null;
  ctx.lastSubject = what;
  return [{ effect: "become", what, spec: { ...spec, ...(inAddition ? { inAddition } : {}) } }];
});

function parseTokenSpecLike(c: Cursor): { types?: string[]; subtypes?: string[]; colors?: SpecifiedTokenSpec["colors"]; power?: Amount; toughness?: Amount } | null {
  return c.attempt((c) => {
    const out: { types: string[]; subtypes: string[]; colors: SpecifiedTokenSpec["colors"]; power?: Amount; toughness?: Amount } = {
      types: [], subtypes: [], colors: [],
    };
    c.attempt((c) => {
      const p = c.number();
      if (p === null || !c.punct("/")) return null;
      const t = c.number();
      if (t === null) return null;
      out.power = { amount: "fixed", value: p };
      out.toughness = { amount: "fixed", value: t };
      return true;
    });
    let consumed = !!out.power;
    for (;;) {
      const t = c.peek();
      if (!t || t.kind !== "word" || t.possessive) break;
      if (COLORS.has(t.value)) { c.pos++; out.colors.push(t.value as SpecifiedTokenSpec["colors"][number]); consumed = true; continue; }
      const singular = singularize(t.value);
      if (CARD_TYPES.has(singular)) { c.pos++; out.types.push(singular); consumed = true; continue; }
      if (c.words("time", "lord")) { out.subtypes.push("Time Lord"); consumed = true; continue; }
      const subtype = canonicalSubtypeName(t.raw);
      if (subtype) { c.pos++; out.subtypes.push(subtype); consumed = true; continue; }
      if (t.value === "and") {
        const next = c.tokens[c.pos + 1];
        if (next?.kind === "word" && (COLORS.has(next.value) || CARD_TYPES.has(singularize(next.value)) || canonicalSubtypeName(next.raw))) { c.pos++; continue; }
      }
      break;
    }
    // "with base power and toughness 4/4"
    c.attempt((c) => {
      if (!c.words("with", "base", "power", "and", "toughness")) return null;
      const p = c.number();
      if (p === null || !c.punct("/")) return null;
      const t = c.number();
      if (t === null) return null;
      out.power = { amount: "fixed", value: p };
      out.toughness = { amount: "fixed", value: t };
      return true;
    });
    if (!consumed) return null;
    // Unmentioned characteristics remain unchanged (CR 205.1, 105.3).
    // Absence of a color change is distinct from explicitly becoming colorless.
    return {
      ...(out.types.length ? { types: out.types } : {}),
      ...(out.subtypes.length ? { subtypes: out.subtypes } : {}),
      ...(out.colors.length ? { colors: out.colors } : {}),
      ...(out.power ? { power: out.power } : {}),
      ...(out.toughness ? { toughness: out.toughness } : {}),
    };
  });
}

const parseReveal: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "reveal", "reveals");
  if (!who) return null;
  const hand = c.attempt(c => {
    const zone = parseZoneRef(c);
    return zone?.zone === "hand" && zone.owner ? zone : null;
  });
  if (hand) return [{ effect: "reveal", who, what: { ref: "all", filter: { cls: "card", zone: hand } } }];
  const what = parseCardExtraction(c);
  if (!what) return null;
  return [{ effect: "reveal", who, what }];
});

const parseLook: StepParser = transactionalStep((c, ctx) => {
  const who = parseActor(c, ctx, "look", "looks");
  if (!who) return null;
  if (c.word("at") === null) return null;
  const top = c.attempt((c): Effect[] | null => {
    if (!c.words("the", "top")) return null;
    const topN = c.attempt(parseAmount) ?? { amount: "fixed" as const, value: 1 };
    c.word("card", "cards");
    if (c.word("of") === null) return null;
    const of = parseZoneRef(c);
    if (!of) return null;
    return [{ effect: "look-at", who, what: { top: topN, of } }];
  });
  if (top) return top;
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "look-at", who, what }];
});

const parseStackCopyStep: StepParser = transactionalStep((c, ctx) => {
  const effect = parseStackCopy(c, ctx.lastPlayer, ctx.stackCopyInput);
  if (!effect) return null;
  ctx.lastPlayer = effect.who;
  return [effect];
});

const parseChooseNewTargetsStep: StepParser = transactionalStep((c, ctx) => {
  const effect = parseChooseNewTargets(c, ctx.lastPlayer);
  if (!effect) return null;
  ctx.lastPlayer = effect.who;
  return [effect];
});

const parseRemoveCounters: StepParser = transactionalStep((c) => {
  if (c.word("remove", "removes") === null) return null;
  const count: Amount | "all" | null = c.word("all") !== null ? "all" : parseAmount(c);
  if (!count) return null;
  const counter = parseCounterSpec(c);
  if (!counter) return null;
  if (c.word("counter", "counters") === null) return null;
  if (c.word("from") === null) return null;
  const from = parseObjectRef(c);
  if (!from) return null;
  return [{ effect: "remove-counters", counter, count, from }];
});

const parseRegenerate: StepParser = transactionalStep((c) => {
  if (c.word("regenerate", "regenerates") === null) return null;
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "regenerate", what }];
});

const parseFight: StepParser = transactionalStep((c) => {
  const a = parseObjectRef(c);
  if (!a) return null;
  if (c.word("fight", "fights") === null) return null;
  const b = parseObjectRef(c);
  if (!b) return null;
  return [{ effect: "fight", a, b }];
});

const parseCant: StepParser = transactionalStep((c) => {
  const indefinite = c.isWord("a", "an");
  let what = parseObjectRef(c);
  if (!what) return null;
  if (c.word("can't") === null) return null;
  const action = parseCantAction(c);
  if (!action) return null;
  // A generic restriction applies to every matching object (e.g. each
  // creature damaged this way after redirection), not one resolution choice.
  // Explicit targets and definite event references retain their cardinality.
  if (indefinite && what.ref === "filter" && what.count.amount === "fixed" && what.count.value === 1)
    what = { ref: "each", filter: what.filter };
  return [{ effect: "cant", what, action }];
});

export function parseCantAction(c: Cursor): CantAction | null {
  return c.attempt((c): CantAction | null => {
    if (c.attempt((c) => (c.words("attack", "or", "block") ? true : null))) return "attack-or-block";
    if (c.word("attack") !== null) return "attack";
    if (c.word("block") !== null) return "block";
    if (c.attempt((c) => (c.words("be", "blocked") ? true : null))) return "be-blocked";
    if (c.attempt((c) => (c.words("be", "countered") ? true : null))) return "be-countered";
    if (c.words("be", "regenerated")) return "be-regenerated";
    if (c.word("untap") !== null) return "untap";
    return c.fail("can't-action");
  });
}

const parseModal: StepParser = transactionalStep((c, ctx) => {
  const modal = parseModalEffect(c, ctx.modalOptions, body => parseSentences(body, { modalOptions: [] }), tokens => lineParser?.(tokens, []) ?? null);
  return modal ? [modal] : null;
});

const parseGameOutcome: StepParser = transactionalStep((c, ctx) => {
  const wins = c.attempt(c => {
    const who = parseActor(c, ctx, "win", "wins");
    return who && c.words("the", "game") ? who : null;
  });
  if (wins) return [{ effect: "win-game", who: wins }];
  const who = parseActor(c, ctx, "lose", "loses");
  return who && c.words("the", "game") ? [{ effect: "lose-game", who }] : null;
});

const parseRoll: StepParser = transactionalStep((c, ctx) => {
  const effect = parseDiceRoll(c, { parseAmount, parseCount, parsePlayerRef }, { previousActor: ctx.lastPlayer, allowRollAgain: ctx.allowRollAgain });
  if (!effect) return null;
  if ("who" in effect) ctx.lastPlayer = effect.who;
  return [effect];
});

// Order matters: subject-first parsers that begin with parseObjectRef go
// after verb-first parsers that could be mistaken for filters.
const STEP_PARSERS: StepParser[] = [
  transactionalStep((c, ctx) => parseTemporal(c, {
    actor: (c, imperative, conjugated) => parseActor(c, ctx, imperative, conjugated), amount: parseAmount,
  })),
  transactionalStep(c => {
    const effect = parseForEach(c, body => parseSentences(body, { modalOptions: [] }));
    return effect ? [effect] : null;
  }),
  transactionalStep((c, ctx) => parseDelayedPrefix(c, ctx, parseSentences)),
  transactionalStep((c, _ctx, options) => parsePlayInstruction(c, options)),
  transactionalStep((c, _ctx, options) => parseLookPermissionEffect(c, options)),
  transactionalStep((c, ctx) => parseUntapSuppressionEffect(c, ctx.lastSubject)),
  transactionalStep((c, ctx) => parseProhibitionEffect(c, ctx.lastSubject)),
  transactionalStep((c, ctx) => {
    const effects = parseObjectActions(c, ctx.lastSubject, ctx.lastPlayer);
    const effect = effects?.[0];
    if (effect && "what" in effect && typeof effect.what === "object" && "ref" in effect.what) ctx.lastSubject = effect.what;
    if (effect && "who" in effect && effect.who) ctx.lastPlayer = effect.who;
    return effects;
  }),
  transactionalStep((c, ctx) => {
    const effects = parseCombatEffect(c, ctx.lastSubject);
    const first = effects?.[0];
    if (first?.effect === "combat-rule") ctx.lastSubject = first.what;
    return effects;
  }),
  parseModal,
  parseGameOutcome,
  parseRoll,
  parseChoose,
  transactionalStep((c, ctx) => parseLibraryScan(c, {
    actor: (c, imperative, conjugated) => parseActor(c, ctx, imperative, conjugated),
    count: parseCount, filter: parseFilter, player: parsePlayerRef, zone: parseZoneRef,
  })),
  parseGet,
  parseDraw,
  parseDestroy,
  parseExile,
  parseCounterSpell,
  parseReturn,
  parseCreateToken,
  parseTapUntap,
  parseSacrifice,
  parseDiscard,
  parseMill,
  parseScrySurveil,
  parseKeywordAction,
  parsePay,
  parsePut,
  parseRemoveCounters,
  parseAddMana,
  parseSearch,
  parseShuffle,
  parseLifeGainLoss,
  parseGainControl,
  parseReveal,
  parseLook,
  parseStackCopyStep,
  parseChooseNewTargetsStep,
  parseRegenerate,
  transactionalStep(c => parsePrevention(c)),
  parseDamage,
  parsePlayerGainAbilities,
  parsePumpOrGain,
  parseBecome,
  parseFight,
  parseCant,
];
