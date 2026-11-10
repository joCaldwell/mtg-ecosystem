// CR 707.10, 707.10b-c, 115.7d-f: a stack copy is neither cast nor
// activated. It retains the original decisions and an ability's source;
// choosing new targets does not change modes or damage division.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Effect, ObjectFilter, ObjectRef, PlayerRef, Trigger } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import type { Token } from "../lexer.ts";
import { parseAmount, parseObjectRef, parsePlayerRef } from "./refs.ts";
import { parseStackCopyExceptions } from "./entry-copy.ts";

type CopyEffect = Extract<Effect, { effect: "copy-stack-object" }>;
type RetargetEffect = Extract<Effect, { effect: "choose-new-targets" }>;

/** Ephemeral lexical proof, never serialized. Matching the original token
 * identity confines it to this first instruction even through parser retries;
 * an identical-looking later pronoun or another body cannot reuse it. */
export interface StackCopyInputProof { readonly token: Token; readonly event: "cast" | "activate" }

function containsTarget(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(containsTarget);
  const node = value as Record<string, unknown>;
  return node.ref === "target" || node.ref === "any-target" || node.player === "target-player" || node.player === "target-opponent"
    || Object.values(node).some(containsTarget);
}
function explicitlyNonmana(filter: ObjectFilter): boolean {
  return filter.manaAbility === false || !!filter.allOf?.some(explicitlyNonmana)
    || !!filter.anyOf?.length && filter.anyOf.every(explicitlyNonmana);
}

/** CR 603.2c/707.10: one casting/activation occurrence has one stack object.
 * The caller supplies only a body with no intervening-if. A direct opening
 * "[you may] copy it" has that unique input; no preceding instruction,
 * alternative event, target declaration or plural occurrence may compete. */
export function uniqueStackTriggerEvent(trigger: Trigger): "cast" | "activate" | undefined {
  if (trigger.grouping || (trigger.trigger !== "cast" && trigger.trigger !== "activate") || containsTarget(trigger)) return undefined;
  if (trigger.trigger === "activate" && !explicitlyNonmana(trigger.what)) return undefined;
  if (trigger.trigger === "cast" && "ref" in trigger.what) {
    const what = trigger.what;
    const singular = what.ref === "self" || what.ref === "it"
      || ((what.ref === "this" || what.ref === "enchanted" || what.ref === "equipped") && (what.noun === "spell" || what.noun === "card"))
      || (what.ref === "that" && !what.plural && (stackFilter(what.filter) || what.filter.cls === "card"))
      || (what.ref === "filter" && what.count.amount === "fixed" && what.count.value === 1);
    if (!singular) return undefined;
  }
  return trigger.trigger;
}

export function prepareStackCopyInput(c: Cursor, trigger: Trigger): StackCopyInputProof | undefined {
  const event = uniqueStackTriggerEvent(trigger);
  if (!event) return undefined;
  const body = new Cursor(c.tokens.slice(c.pos));
  if (!body.words("you", "may")) body.word("you");
  if (body.word("copy") === null) return undefined;
  const token = body.peek();
  if (!token || body.word("it") === null) return undefined;
  return { token, event };
}

/** A copy of a card in exile/graveyard is a different operation (CR 707.12).
 * Untyped it/them cannot prove the stack domain and remain unsupported here.
 * Permanent SPELLS are eligible; permanent objects are not. */
function outsideStackScope(filter: ObjectFilter): boolean {
  return !!(filter.zone && filter.zone.zone !== "stack")
    || !!(filter.cls && filter.cls !== "spell" && filter.cls !== "ability")
    || filter.abilityKind === "static" || filter.abilityKind === "spell"
    || !!filter.allOf?.some(outsideStackScope) || !!filter.anyOf?.some(outsideStackScope);
}
function stackFilter(filter: ObjectFilter): boolean {
  if (outsideStackScope(filter)) return false;
  if (filter.cls === "spell" || filter.cls === "ability") return true;
  if (filter.allOf?.some(stackFilter)) return true;
  return !!filter.anyOf?.length && filter.anyOf.every(stackFilter);
}

function stackReference(ref: ObjectRef): boolean {
  return ref.ref === "trigger-event-object" || ref.ref === "this" && ref.noun === "spell"
    || ("filter" in ref && stackFilter(ref.filter));
}

function actor(c: Cursor, previous: PlayerRef | undefined, imperative: string, conjugated: string): { who: PlayerRef; optional?: true; decidedBy?: PlayerRef; explicitOptional: boolean } | null {
  return c.attempt(c => {
    const stated = parsePlayerRef(c);
    const optional = !!stated && c.word("may") !== null;
    const verb = c.word(imperative, conjugated);
    if (!verb || (optional && verb !== imperative)) return null;
    const who = stated ?? (verb === imperative && previous?.player !== "they" ? { player: "you" as const } : previous);
    if (!who) return null;
    // Another player's "may" is that player's decision (EffectBase.decidedBy).
    const decidedBy = optional && stated && stated.player !== "you" ? stated : undefined;
    return { who, ...(optional ? { optional: true as const } : {}), ...(decidedBy ? { decidedBy } : {}), explicitOptional: optional };
  });
}

/** "the copy", "that copy", "the copies", "those copies", "each copy". */
function copyResult(c: Cursor): Extract<ObjectRef, { ref: "copy-result" }> | null {
  return c.attempt(c => {
    if (c.words("the", "copy") || c.words("that", "copy")) return { ref: "copy-result" as const, number: "singular" as const };
    if (c.words("the", "copies") || c.words("those", "copies") || c.words("each", "copy")) return { ref: "copy-result" as const, number: "plural" as const };
    return null;
  });
}

/** Generic sentence code owns coordinated optional groups only for its
 * consumed "you may". An explicit different player's may needs its own group
 * scope before more coordinated instructions can safely be supported. */
function ambiguousOptionalTail(c: Cursor, explicitOptional: boolean): boolean {
  return explicitOptional && (c.isPunct(",") || c.isWord("and", "then"));
}

export function parseStackCopy(c: Cursor, previousActor?: PlayerRef, proof?: StackCopyInputProof): CopyEffect | null {
  return c.attempt(c => {
    const by = actor(c, previousActor, "copy", "copies");
    if (!by) return null;
    const inputToken = c.peek();
    let what = parseObjectRef(c);
    if (what?.ref === "it" && proof && proof.token === inputToken)
      what = { ref: "trigger-event-object", event: proof.event };
    if (!what || !stackReference(what)) return c.fail("explicit spell or proven trigger object to copy");
    let count: Amount = { amount: "fixed", value: 1 };
    if (c.word("twice") !== null) count = { amount: "fixed", value: 2 };
    else {
      const repetitions = c.attempt(c => {
        const amount = parseAmount(c);
        return amount && c.word("time", "times") !== null ? amount : null;
      });
      if (repetitions) count = repetitions;
    }
    if (count.amount === "fixed" && (!Number.isInteger(count.value) || count.value < 0)) return null;
    // ", except the copy isn't legendary": the exceptions end the instruction.
    const copyExceptions = c.attempt(parseStackCopyExceptions);
    const copied: CopyEffect = { effect: "copy-stack-object", what, who: by.who, count,
      ...(copyExceptions ? { copyExceptions } : {}),
      ...(by.optional ? { optional: true } : {}), ...(by.decidedBy ? { decidedBy: by.decidedBy } : {}) };
    // "… and may choose new targets for the copy": the copying player's own
    // option for each copy as it is created (CR 707.10c).
    const retarget = c.attempt(c => {
      if (!c.words("and", "may", "choose") || !(c.words("new", "targets", "for") || c.words("a", "new", "target", "for"))) return null;
      const what = copyResult(c);
      const after: Effect = { effect: "choose-new-targets", what: what ?? { ref: "it" }, who: by.who, timing: "copy-creation", optional: true };
      return what && foldCopyRetarget(copied, after);
    });
    if (retarget) return retarget;
    if (ambiguousOptionalTail(c, by.explicitOptional)) return null;
    return copied;
  });
}

function retargetReference(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    if (c.words("the", "copy")) return { ref: "copy-result", number: "singular" };
    if (c.words("the", "copies") || c.words("each", "copy")) return { ref: "copy-result", number: "plural" };
    const what = parseObjectRef(c);
    // Here the operation itself establishes the required domain without
    // asserting a copy origin: "it" remains an unbound printed reference.
    if (what && (what.ref === "it" || what.ref === "them" || stackReference(what))) return what;
    return null;
  });
}

export function parseChooseNewTargets(c: Cursor, previousActor?: PlayerRef): RetargetEffect | null {
  return c.attempt(c => {
    const by = actor(c, previousActor, "choose", "chooses");
    if (!by || !c.words("new", "targets", "for")) return null;
    const what = retargetReference(c);
    if (!what || ambiguousOptionalTail(c, by.explicitOptional)) return null;
    return { effect: "choose-new-targets", who: by.who, what,
      timing: what.ref === "copy-result" ? "copy-creation" : "resolution",
      ...(by.optional ? { optional: true } : {}) };
  });
}

/** This is a syntactic delayed-body dependency, not an antecedent binding.
 * An explicit copy-result phrase can continue the copy instruction; an "it"
 * or an independently selected target cannot prove that sentence boundary. */
export function isCopyRetargetContinuation(before: Effect, after: Effect): boolean {
  if (before.effect !== "copy-stack-object" || after.effect !== "choose-new-targets" || after.what.ref !== "copy-result") return false;
  const oneInput = before.what.ref === "trigger-event-object" || before.what.ref === "this" || (before.what.ref === "that" && !before.what.plural)
    || ("count" in before.what && before.what.count?.amount === "fixed" && before.what.count.value === 1 && !("upTo" in before.what && before.what.upTo));
  const oneResult = oneInput && before.count.amount === "fixed" && before.count.value === 1 && !before.forEach;
  return after.what.number === (oneResult ? "singular" : "plural");
}

/** Integrate only a direct, complete copy-result continuation. The caller
 * must prove that both instructions share a scope and no earlier copy group
 * competes for this printed reference. Unfolded references retain explicit
 * copy-creation timing and remain unresolved in binding analysis. */
export function foldCopyRetarget(before: Effect, after: Effect): CopyEffect | null {
  if (!isCopyRetargetContinuation(before, after) || before.effect !== "copy-stack-object"
    || after.effect !== "choose-new-targets" || before.retarget
    || after.timing !== "copy-creation" || after.forEach || after.duration) return null;
  return { ...before, retarget: { who: after.who, ...(after.optional ? { optional: true as const } : {}) } };
}
