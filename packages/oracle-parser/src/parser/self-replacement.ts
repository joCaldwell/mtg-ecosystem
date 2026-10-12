// Some later clauses replace an earlier instruction of the SAME resolving
// spell/ability. CR 608.2c, 614.15 and 616.1a distinguish these from later
// ordinary actions and from continuous event replacements.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, Effect, Sentence } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { parseAmount, parseCondition, parseObjectRef, parsePlayerRef } from "./refs.ts";

type BodyParser = (c: Cursor) => Sentence[] | null;

/** Targets of quoted or newly created triggered abilities belong to those
 * future ability instances, not the spell/ability being resolved here. */
function containsExplicitTargets(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(containsExplicitTargets);
  if (!value || typeof value !== "object") return false;
  if (("gained" in value && value.gained === "quoted") ||
      ("effect" in value && value.effect === "create-reflexive-trigger")) return false;
  if ("effect" in value && value.effect === "create-delayed-trigger") {
    const delayed = value as Extract<Effect, { effect: "create-delayed-trigger" }>;
    // A suffix action's targets are chosen now; a prefix delayed body's
    // targets are chosen when that ability triggers. A target in the event
    // being watched also belongs to the creating spell (CR 603.7c).
    return (delayed.targets === "enclosing-ability" && containsExplicitTargets(delayed.effects)) ||
      (delayed.triggerTargets === "enclosing-ability" && containsExplicitTargets(delayed.trigger));
  }
  if (("ref" in value && ["target", "any-target"].includes(String(value.ref))) ||
      ("player" in value && ["target-player", "target-opponent"].includes(String(value.player))) ||
      ("to" in value && value.to === "any-target")) return true;
  return Object.values(value).some(containsExplicitTargets);
}

/** The classifier only permits this policy on a spell ability: kicker-linked
 * ETB triggers do not inherit the spell's target announcement exception. */
export function hasAlternativeKickerTargets(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(hasAlternativeKickerTargets);
  if (!value || typeof value !== "object") return false;
  if ("effect" in value && value.effect === "self-replacement" && "targetPolicy" in value && value.targetPolicy === "alternative-by-kicker") return true;
  return Object.values(value).some(hasAlternativeKickerTargets);
}

const LASTING = new Set<Effect["effect"]>(["pump", "set-pt", "gain-abilities", "player-gain-abilities", "lose-abilities", "gain-control", "become", "modify-characteristics", "cant", "combat-rule"]);

/** Index of this sentence's final period (or end of input); -1 when a
 * quotation is left open. */
function sentenceEnd(c: Cursor): number {
  let end = c.pos;
  let quoted = false;
  while (end < c.tokens.length) {
    const token = c.tokens[end];
    if (token.kind === "punct" && token.value === '"') quoted = !quoted;
    if (!quoted && token.kind === "punct" && token.value === ".") break;
    end++;
  }
  return quoted ? -1 : end;
}

const isInstead = (token: Token | undefined): boolean => token?.kind === "word" && !token.possessive && token.value === "instead";

/** Abbreviated replacements restate only what changes. The unstated parts
 * are those of the replaced instruction itself, never a new selection:
 * Burst Lightning / Akoum Hellkite omit the damage recipient, and "N of
 * those tokens" (Rite of Replication) changes only the number created. */
function parseAbbreviated(tokens: Token[], original: Effect): Effect | null {
  const c = new Cursor(tokens);
  if (original.effect === "damage") {
    const source = parseObjectRef(c);
    if (!source || c.word("deal", "deals") === null) return null;
    const amount = parseAmount(c);
    if (!amount || c.word("damage") === null) return null;
    // An explicitly repeated recipient must be the original announced one.
    if (!c.done() && !(original.to.to === "any-target" && c.words("to", "that", "permanent", "or", "player") && c.done())) return null;
    return { effect: "damage", source, amount, to: structuredClone(original.to) };
  }
  if (original.effect === "create-token") {
    if (c.word("create") === null) return null;
    const count = parseAmount(c);
    if (!count || !c.words("of", "those", "tokens") || !c.done()) return null;
    return { ...structuredClone(original), count };
  }
  return null;
}

/** One adjacent, unconditional instruction has unambiguous replacement
 * scope. A multi-instruction original, original condition, or otherwise
 * branch needs an explicit association before it can be accepted.
 * "X instead if C" states the same replacement with its condition last. */
export function parseSelfReplacement(c: Cursor, previous: Sentence, parseBody: BodyParser): Effect | null {
  return c.attempt(c => {
    if (previous.steps.length !== 1 || previous.condition || previous.otherwise || previous.bindings) return null;
    const original = previous.steps[0];
    if (original.optional || original.forEach || ["self-replacement", "simultaneous", "optional-group", "modal", "create-delayed-trigger", "create-replacement"].includes(original.effect)) return null;
    let condition: Condition | null;
    let bodyTokens: Token[];
    let end: number;
    if (c.word("if") !== null) {
      condition = parseCondition(c);
      if (!condition || !c.punct(",")) return null;
      const leadingInstead = c.word("instead") !== null;
      end = sentenceEnd(c);
      if (end < 0) return null;
      const trailingInstead = isInstead(c.tokens[end - 1]);
      if (leadingInstead === trailingInstead) return null;
      bodyTokens = c.tokens.slice(c.pos, trailingInstead ? end - 1 : end);
    } else {
      end = sentenceEnd(c);
      if (end < 0) return null;
      let quoted = false, at = -1;
      for (let i = c.pos; i < end; i++) {
        const token = c.tokens[i];
        if (token.kind === "punct" && token.value === '"') quoted = !quoted;
        if (!isInstead(token)) continue;
        if (quoted || at >= 0) return null;
        at = i;
      }
      const next = c.tokens[at + 1];
      if (at <= c.pos || next?.kind !== "word" || next.possessive || next.value !== "if") return null;
      const tail = new Cursor(c.tokens.slice(at + 2, end));
      condition = parseCondition(tail);
      if (!condition || !tail.done()) return null;
      bodyTokens = c.tokens.slice(c.pos, at);
    }
    const body = new Cursor(bodyTokens);
    const parsed = parseBody(body);
    let instead: Effect[] | undefined;
    let explicitReplacementTargets = false;
    if (parsed?.length === 1 && body.done() && !parsed[0].condition && !parsed[0].otherwise && !parsed[0].bindings) {
      instead = parsed[0].steps;
      // "That creature gets -4/-4 instead" restates only the modifier; a
      // replacement of the same pump keeps its duration (CR 611.2a). Any
      // other lasting replacement must print its own duration.
      if (original.duration && instead.some(step => !step.duration && LASTING.has(step.effect))) {
        const only = instead[0];
        if (instead.length !== 1 || only.effect !== "pump" || original.effect !== "pump") return null;
        instead = [{ ...only, duration: structuredClone(original.duration) }];
      }
      explicitReplacementTargets = containsExplicitTargets(instead);
    }
    else {
      const abbreviated = parseAbbreviated(bodyTokens, original);
      if (!abbreviated) return null;
      instead = [abbreviated];
      explicitReplacementTargets = containsExplicitTargets(abbreviated.effect === "damage"
        ? { source: abbreviated.source, amount: abbreviated.amount } : abbreviated.effect === "create-token" ? abbreviated.count : abbreviated);
    }
    if (!instead?.length) return null;
    // CR 601.2c and 702.33g permit alternative targets due to a spell's kicker
    // payment. A generic resolution-time condition cannot silently change the
    // set of announced targets. Other associations remain unsupported here.
    const targetPolicy = explicitReplacementTargets ? "alternative-by-kicker" : "retain-original";
    if (explicitReplacementTargets && (condition.condition !== "kicked" ||
        condition.what.ref !== "this" || condition.what.noun !== "spell")) return null;

    let replacementChoice: Extract<Effect, { effect: "self-replacement" }>["replacementChoice"];
    const chooser = new Cursor(bodyTokens);
    const who = parsePlayerRef(chooser);
    const printedMay = who && chooser.word("may") !== null;
    const first = instead[0];
    const optional = first.optional || first.effect === "optional-group";
    if (printedMay || optional || instead.some(step => step.optional || step.effect === "optional-group")) {
      // "If ..., you may X instead" makes replacement application optional.
      // The printed chooser is distinct from X's affected player/recipient.
      if (!printedMay || !who || instead.length !== 1 || !optional) return null;
      if (first.effect === "optional-group") {
        if (JSON.stringify(first.who) !== JSON.stringify(who)) return null;
        instead = first.steps;
      } else {
        const { optional: _optional, ...mandatory } = first;
        instead = [mandatory];
      }
      replacementChoice = { who, declined: "original" };
    }
    // A resolution-time choice cannot retroactively announce the discarded
    // target branch. No supported wording establishes that choice at casting.
    if (replacementChoice && targetPolicy === "alternative-by-kicker") return null;
    c.pos = end < c.tokens.length ? end + 1 : end;
    return { effect: "self-replacement", original, condition, instead, targetPolicy, ...(replacementChoice ? { replacementChoice } : {}) };
  });
}
