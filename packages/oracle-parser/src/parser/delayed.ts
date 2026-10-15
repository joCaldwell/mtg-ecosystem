// Delayed and reflexive triggered abilities created by a resolving effect.
// CR 603.7a–c and 603.12: creating an ability does not execute its body now;
// a delayed ability normally triggers once, while a reflexive ability checks
// the associated action that happened earlier in the same resolution.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

import type { Duration, Effect, ObjectRef, Sentence, Trigger } from "../ast.ts";
import { isCardCopyCast } from "./card-copy.ts";
import { Cursor } from "./cursor.ts";
import type { EffectContext } from "./effects.ts";
import { parseTrigger, parseTriggerLimit } from "./triggers.ts";
import { parseActivationRules } from "./restrictions.ts";
import { parseComparison, parseCondition, parseFilter, parseObjectRef, parsePlayerRef } from "./refs.ts";
import { parseReflexiveOutcome } from "./event-outcomes.ts";
import { parseTurnLinkedTrigger } from "./temporal.ts";
import { isCopyRetargetContinuation, prepareStackCopyInput, type StackCopyInputProof } from "./stack-copy.ts";

type BodyParser = (c: Cursor, ctx: EffectContext) => Sentence[] | null;

/** Unquoted delayed bodies have no universal sentence delimiter. Recognize
 * only explicit dependencies between sentences rather than swallowing every
 * remaining instruction. Time to Feed is the counterexample: its final fight
 * happens immediately, not after the chosen creature dies (CR 603.7a).
 * These productions also cover Teach by Example, Spinal Embrace, The Last
 * Ronin, Consuming Rage, and Waltz of Rage, checked in the Oracle corpus. */
function hasSupportedContinuations(sentences: Sentence[], reflexive = false): boolean {
  for (let i = 1; i < sentences.length; i++) {
    const previous = sentences[i - 1];
    const next = sentences[i];
    if (next.condition?.condition === "you-do" || next.condition?.condition === "you-dont") continue;
    if (reflexive && continuesReflexiveBody(previous, next)) continue;
    // "..., then exile … and copy them. You may cast the copies": the cast
    // refers to the copies the sentence's final step made.
    if (!next.condition && !next.otherwise && !previous.otherwise && previous.steps.length > 1 && next.steps.length === 1
      && isCardCopyCast(previous.steps[previous.steps.length - 1], next.steps[0])) continue;
    if (next.condition || next.otherwise || previous.otherwise || previous.steps.length !== 1 || next.steps.length !== 1) return false;
    const before = previous.steps[0];
    const after = next.steps[0];
    if (isCopyRetargetContinuation(before, after)) continue;
    if (isCardCopyCast(before, after)) continue;
    if (before.effect === "exile" && after.effect === "grant-play-permission") {
      const what = after.permission.what;
      if (what.ref === "it" || (what.ref === "that" && what.filter.cls === "card")) continue;
    }
    // "You may cast X. If that spell would be put into a graveyard, exile it
    // instead.": the replacement watches the spell the cast instruction makes.
    if (before.effect === "play-card" && before.instruction.action === "cast" && after.effect === "create-replacement"
      && after.replacement.event.event === "move-zone") {
      const what = after.replacement.event.what;
      if (what.ref === "that" && !what.plural && what.filter.cls === "spell" && Object.keys(what.filter).length === 1) continue;
    }
    if (before.effect === "put-counters" && before.on.ref === "it"
      && after.effect === "gain-abilities" && "ref" in after.what && after.what.ref === "it") continue;
    if (before.effect === "pump" && before.what.ref === "it" && after.effect === "create-delayed-trigger"
      && after.targets === "enclosing-ability" && after.trigger.trigger === "phase"
      && after.trigger.phase === "end-of-combat" && after.effects.length === 1) {
      const sentence = after.effects[0];
      const destruction = sentence.steps[0];
      if (!sentence.condition && !sentence.otherwise && sentence.steps.length === 1 && destruction.effect === "destroy"
        && destruction.what.ref === "that" && destruction.what.filter.types?.length === 1
        && destruction.what.filter.types[0] === "creature") continue;
    }
    return false;
  }
  return true;
}

function hasNode(value: unknown, test: (node: Record<string, unknown>) => boolean): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(item => hasNode(item, test));
  const node = value as Record<string, unknown>;
  return test(node) || Object.values(node).some(item => hasNode(item, test));
}

/** CR 603.12: a reflexive ability's targets are chosen, and its tokens and
 * choices made, only after the enclosing ability has finished resolving. A
 * plain following sentence that names one of them by pronoun or "that …"
 * can therefore only continue the reflexive ability (Inti, Seneschal of the
 * Sun: "… put a +1/+1 counter on target attacking creature. It gains trample
 * until end of turn."). A sentence with a fresh target or a condition, or one
 * after a body sentence that introduced nothing, proves no such dependency. */
function continuesReflexiveBody(previous: Sentence, next: Sentence): boolean {
  if (next.condition || next.otherwise || next.bindings || previous.otherwise) return false;
  // "When you do, if …, A. B.": the leading "if" is the reflexive ability's
  // intervening condition (603.4) and would govern B too; a sentence
  // condition on A alone cannot say that.
  if (previous.condition) return false;
  const introduces = hasTarget(previous.steps) ||
    hasNode(previous.steps, node => node.effect === "create-token" || node.effect === "choose-object");
  const anaphor = hasNode(next.steps, node => node.ref === "it" || node.ref === "them" || node.ref === "that");
  return introduces && anaphor && !hasTarget(next.steps);
}

function hasTarget(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(hasTarget);
  const node = value as Record<string, unknown>;
  if (node.ref === "target" || node.ref === "any-target" || node.player === "target-player" || node.player === "target-opponent") return true;
  return Object.values(node).some(hasTarget);
}

function parseBodyBeforeActivationRules(c: Cursor, ctx: EffectContext, parseBody: BodyParser, stackCopyInput?: StackCopyInputProof, reflexive = false): Sentence[] | null {
  // An enclosing proof is never inherited: a delayed header may establish a
  // fresh local occurrence, while a reflexive body has no such event input.
  const effects = parseBody(c, { modalOptions: ctx.modalOptions, stackCopyInput });
  if (!effects) return null;
  if (!hasSupportedContinuations(effects, reflexive)) return c.fail("explicit dependency between delayed-body sentences");
  if (c.done()) return effects;
  const tailStart = c.pos;
  const rules = parseActivationRules(c) ?? parseTriggerLimit(c);
  c.pos = tailStart;
  if (!rules) return null;
  // The remaining activation rules govern the enclosing ability, not the
  // newly created triggered ability. Let the enclosing sentence consume its
  // period before index.ts parses those rules.
  const last = c.tokens[tailStart - 1];
  if (last?.kind !== "punct" || last.value !== ".") return null;
  c.pos--;
  return effects;
}

/** Parse a trigger in the context of an ability being created now. The sole
 * normalization removes a represented "next" from an exact phase/event header.
 * Ordinary printed triggers continue to reject this delayed-only syntax. */
type DelayedHeader = { trigger: Trigger; duration?: Duration; occurrence: "next" | "each"; nextOnly?: true };

function parseDelayedTrigger(c: Cursor): DelayedHeader | null {
  return c.attempt(c => {
    const scoped = parseTurnLinkedTrigger(c);
    if (scoped) return { trigger: scoped, occurrence: "next", nextOnly: true };
    const start = c.pos;
    const tokens = c.tokens.slice(start);
    const word = (index: number, value: string): boolean => {
      const token = tokens[index];
      return token?.kind === "word" && !token.possessive && token.value === value;
    };
    let nextIndex = -1;
    if (word(0, "at") && word(1, "the") && word(2, "beginning") && word(3, "of")) {
      if (word(4, "next")) nextIndex = 4;
      // "their next upkeep": that player's (symbolic "they") next one.
      else if ((word(4, "the") || word(4, "your") || word(4, "their")) && word(5, "next")) nextIndex = 5;
    }
    if (word(0, "when") || word(0, "whenever")) {
      // Explicit "next" qualifies the represented event, never the object's
      // identity. Restrict this production to an immediate known event verb.
      const verbs = new Set(["cast", "casts", "activate", "activates", "attack", "attacks", "draw", "draws", "die", "dies", "leave", "leaves", "enter", "enters", "becomes", "become"]);
      for (let i = 1; i < tokens.length; i++) {
        const token = tokens[i];
        if (token.kind === "punct" && token.value === ",") break;
        const next = tokens[i + 1];
        if (word(i, "next") && next?.kind === "word" && verbs.has(next.value)) { nextIndex = i; break; }
      }
    }

    // The supported turn-qualified headers above have explicit TurnRefs.
    // All other possessives remain intact, so an unrepresented turn scope
    // cannot silently become an unrestricted next phase/step.
    let trigger: Trigger | null;
    if (nextIndex < 0) trigger = parseTrigger(c);
    else {
      const withoutNext = tokens.filter((_, index) => index !== nextIndex);
      const eventCursor = new Cursor(withoutNext);
      trigger = parseTrigger(eventCursor);
      if (!trigger || eventCursor.pos <= nextIndex || (word(0, "at") && trigger.trigger !== "phase"))
        return c.fail("explicit next delayed trigger");
      c.pos = start + eventCursor.pos + 1;
    }
    if (!trigger) return null;
    const duration: Duration | undefined = c.words("this", "turn") ? { duration: "this-turn" }
      : c.words("this", "combat") ? { duration: "this-combat" } : undefined;
    return { trigger, occurrence: duration && nextIndex < 0 ? "each" : "next",
      ...(duration ? { duration } : {}), ...(nextIndex >= 0 ? { nextOnly: true } : {}) };
  });
}

/** Prefixes own their body and explicitly dependent subsequent sentences.
 * Ambiguous remaining scope is rejected. A callback keeps this module free of a
 * runtime cycle with effects.ts; only the nested body's context is mutable. */
export function parseDelayedPrefix(c: Cursor, ctx: EffectContext, parseBody: BodyParser): Effect[] | null {
  return c.attempt(c => {
    const reflexive = c.words("when", "you", "do") ? true
      : c.words("when", "you", "don't") ? false : null;
    if (reflexive !== null) {
      if (!c.punct(",")) return null;
      const effects = parseBodyBeforeActivationRules(c, ctx, parseBody, undefined, true);
      if (!effects) return null;
      return [{ effect: "create-reflexive-trigger",
        event: { event: "preceding-action", who: { player: "you" }, performed: reflexive }, effects }];
    }

    // CR 603.12: the repeated payment of the preceding instruction was made.
    if (c.words("when", "you", "pay", "this", "cost", "one", "or", "more", "times")) {
      if (!c.punct(",")) return null;
      const effects = parseBodyBeforeActivationRules(c, ctx, parseBody, undefined, true);
      return effects && [{ effect: "create-reflexive-trigger", event: { event: "repeated-payment", who: { player: "you" } }, effects }];
    }

    const outcome = c.attempt(c => c.word("when") !== null
      ? parseReflexiveOutcome(c, { parseComparison, parseFilter, parseObjectRef, parsePlayerRef }) : null);
    if (outcome) {
      if (!c.punct(",")) return null;
      const effects = parseBodyBeforeActivationRules(c, ctx, parseBody);
      return effects && [{ effect: "create-reflexive-trigger", event: { event: "event-result", ...outcome }, effects }];
    }

    const connived = c.attempt((c): { event: "connived" | "regenerated"; what: ObjectRef } | null => {
      if (c.word("when") === null) return null;
      const what = parseObjectRef(c);
      const single = what && (what.ref === "it" || what.ref === "this" || what.ref === "self" || (what.ref === "that" && !what.plural));
      // CR 701.19a, 603.12: the regeneration shield this ability created is applied.
      const verb = single && c.word("connives", "regenerates");
      return verb && c.words("this", "way") ? { event: verb === "connives" ? "connived" : "regenerated", what } : null;
    });
    if (connived) {
      if (!c.punct(",")) return null;
      const effects = parseBodyBeforeActivationRules(c, ctx, parseBody);
      return effects && [{ effect: "create-reflexive-trigger", event: connived, effects }];
    }

    const header = parseDelayedTrigger(c);
    if (!header || !c.punct(",")) return null;
    // An intervening if has trigger-time and resolution-time semantics
    // (CR 603.4), unlike an ordinary conditional instruction in the body.
    const condition = c.attempt(c => {
      if (c.word("if") === null) return null;
      const condition = parseCondition(c);
      return condition && c.punct(",") ? condition : null;
    });
    if (c.isWord("if")) return c.fail("complete delayed intervening-if clause");
    // A target inside an intervening condition has its own announcement/
    // capture scope; neither body targets nor trigger-header targets records
    // that scope. Reject until a condition-target policy exists (CR 603.4).
    if (condition && hasTarget(condition)) return c.fail("represented delayed-condition target scope");
    const effects = parseBodyBeforeActivationRules(c, ctx, parseBody, condition ? undefined : prepareStackCopyInput(c, header.trigger));
    if (!effects) return null;
    return [{ effect: "create-delayed-trigger", ...header, effects,
      ...(hasTarget(header.trigger) ? { triggerTargets: "enclosing-ability" as const } : {}),
      ...(condition ? { condition } : {}), targets: "delayed-ability" }];
  });
}

/** A line beginning with When/Whenever may itself be a spell instruction
 * creating a bounded delayed ability (e.g. Ondu Rising). An explicitly
 * stated lifetime, a specified turn/phase occurrence, or an explicit "next"
 * (Pact of Negation: one trigger at the next such upkeep, CR 603.7b) permits
 * this fallback; malformed ordinary trigger clauses cannot silently become
 * spell effects. */
export function parseDelayedSpellPrefix(c: Cursor, ctx: EffectContext, parseBody: BodyParser): Effect[] | null {
  return c.attempt(c => {
    const effects = parseDelayedPrefix(c, ctx, parseBody);
    if (!effects || effects.length !== 1 || effects[0].effect !== "create-delayed-trigger") return null;
    const effect = effects[0];
    // A bare "next turn" header stays unsupported: turn-scoped occurrences
    // use the explicit TurnRef forms of parseTurnLinkedTrigger.
    const explicitNext = effect.nextOnly && !(effect.trigger.trigger === "phase" && effect.trigger.phase === "turn");
    if (!effect.duration && !explicitNext && effect.trigger.trigger !== "phase-in-turn" && effect.trigger.trigger !== "phase-instance") return null;
    return effects;
  });
}

/** Call after an individual instruction has its optional/duration riders,
 * before appending it to the enclosing sentence. Thus "Draw a card, then
 * sacrifice it at ..." schedules only the sacrifice. Suffix targets belong
 * to the enclosing spell/ability; they are not chosen anew when it triggers.
 * See CR 603.7c and 603.3d and Silent Assassin's Oracle wording. */
export function parseDelayedSuffix(c: Cursor, effects: Effect[]): Effect[] | null {
  return c.attempt(c => {
    if (effects.length === 0) return null;
    const header = parseDelayedTrigger(c);
    if (!header) return null;
    // Qualified trailing conditions and durations must not be partially
    // accepted as an unconditional next-occurrence trigger.
    if (c.isWord("if", "unless", "this", "during", "until"))
      return c.fail("unqualified delayed trigger");
    if (!c.done() && !c.isPunct(".")) return c.fail("end of delayed instruction");
    const following = c.peek(1);
    // "If X would …, … instead." is a replacement effect of its own (CR
    // 614.1a); it cannot be a condition or continuation of this instruction.
    const clause = c.tokens.slice(c.pos + 2);
    const comma = clause.findIndex(token => token.kind === "punct" && (token.value === "," || token.value === "."));
    const prospective = following?.kind === "word" && following.value === "if"
      && clause.slice(0, comma < 0 ? 0 : comma).some(token => token.kind === "word" && !token.possessive && token.value === "would");
    // "Flip a coin at the beginning of the next end step. If you lose the
    // flip, ...": the flip is made when the delayed ability resolves, so its
    // outcome clauses belong to that flip (CR 705.2, 603.7); the sentence
    // parser folds them into the delayed body.
    const startsWith = (...words: string[]): boolean =>
      words.every((word, k) => clause[k]?.kind === "word" && clause[k].value === word);
    const flipOutcome = effects.at(-1)?.effect === "flip-coin" && (startsWith("you", "win", "the", "flip")
      || startsWith("you", "lose", "the", "flip") || startsWith("it", "comes", "up"));
    if (c.isPunct(".") && following?.kind === "word" && ["if", "when", "then", "otherwise"].includes(following.value) && !prospective
        && !(following.value === "if" && flipOutcome))
      return c.fail("unambiguous delayed instruction scope");
    return [{ effect: "create-delayed-trigger", ...header,
      ...(hasTarget(header.trigger) ? { triggerTargets: "enclosing-ability" as const } : {}),
      effects: [{ steps: effects }], targets: "enclosing-ability" }];
  });
}
