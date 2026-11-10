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
  if ("effect" in value && value.effect === "self-replacement" && "targetPolicy" in value && (value.targetPolicy === "alternative-by-kicker" || value.targetPolicy === "alternative-by-cost")) return true;
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

function oneSentence(tokens: Token[], parseBody: BodyParser): boolean {
  const body = new Cursor(tokens);
  const parsed = parseBody(body);
  return parsed?.length === 1 && body.done();
}

const isInstead = (token: Token | undefined): boolean => token?.kind === "word" && !token.possessive && token.value === "instead";

/** Abbreviated replacements restate only what changes. The unstated parts
 * are those of the replaced instruction itself, never a new selection:
 * Burst Lightning / Akoum Hellkite omit the damage recipient, and "N of
 * those tokens" (Rite of Replication) changes only the number created. */
function parseAbbreviated(tokens: Token[], original: Effect): { effect: Effect; bindings?: Sentence["bindings"] } | null {
  const c = new Cursor(tokens);
  const effect = parseAbbreviatedEffect(c, original);
  if (!effect) return null;
  if (c.done()) return { effect };
  // "instead create X of those tokens, where X is …" (The Final Days): the
  // replacement defines its own X, so it stands as a whole sentence.
  if (effect.effect !== "create-token" || !c.punct(",") || !c.words("where", "x", "is")) return null;
  const value = parseAmount(c);
  return value && c.done() ? { effect, bindings: [{ variable: "x", value }] } : null;
}

function parseAbbreviatedEffect(c: Cursor, original: Effect): Effect | null {
  if (original.effect === "damage") {
    const source = parseObjectRef(c);
    if (!source || c.word("deal", "deals") === null) return null;
    const amount = parseAmount(c);
    if (!amount || c.word("damage") === null) return null;
    // An explicitly repeated recipient must be the original announced one.
    if (!c.done() && !(original.to.to === "any-target" && c.words("to", "that", "permanent", "or", "player") && c.done())) return null;
    return { effect: "damage", source, amount, to: structuredClone(original.to) };
  }
  // "Prevent the next 6 damage this way instead" (Pollen Remedy): the same
  // shield, recipients and division with a different budget.
  if (original.effect === "prevent-damage" && original.prevention.shield.shield === "damage-budget") {
    if (!c.words("prevent", "the", "next")) return null;
    const amount = parseAmount(c);
    // "prevent the next 2 damage instead" (Elvish Healer) omits "this way".
    if (!amount || c.word("damage") === null || (!c.words("this", "way") && !c.done()) || !c.done()) return null;
    const replaced = structuredClone(original);
    if (replaced.prevention.shield.shield !== "damage-budget") return null;
    replaced.prevention.shield.amount = amount;
    return replaced;
  }
  if (original.effect === "create-token") {
    if (c.word("create") === null) return null;
    const count = parseAmount(c);
    if (!count || !c.words("of", "those", "tokens")) return null;
    return { ...structuredClone(original), count };
  }
  return null;
}

const SINGLE_EXCLUDED = ["self-replacement", "simultaneous", "optional-group", "modal", "create-delayed-trigger", "create-replacement"];
const WHOLE_EXCLUDED = ["self-replacement", "optional-group", "modal", "create-delayed-trigger", "create-replacement"];

/** The sentence that results from replacing `previous`. A whole-sentence
 * original carries the condition and definitions of that sentence itself. */
export function withSelfReplacement(previous: Sentence, replacement: Effect): Sentence {
  return replacement.effect === "self-replacement" && "sequence" in replacement.original
    ? { steps: [replacement] } : { ...previous, steps: [replacement] };
}

/** One adjacent, unconditional instruction has unambiguous replacement
 * scope and keeps the single-instruction shape. Otherwise the replaced part
 * is the whole preceding sentence (CR 614.15), which the wording must make
 * unambiguous: "If C, instead X" replaces everything that sentence says,
 * while a trailing "X instead" names one action and so only replaces a
 * sentence with a single instruction. An otherwise branch stays unsupported.
 * "X instead if C" states the same replacement with its condition last. */
export function parseSelfReplacement(c: Cursor, previous: Sentence, parseBody: BodyParser): Effect | null {
  return c.attempt(c => {
    if (previous.otherwise) return null;
    const lone = previous.steps.length === 1 ? previous.steps[0] : undefined;
    // "You may X. If C, instead you may Y." (Court of Bounty): your optional
    // instruction is replaced by your own optional alternative; declining Y
    // leaves you the original choice, which the replacement choice states.
    const optionalLone = lone?.optional && !lone.decidedBy && !lone.forEach && !SINGLE_EXCLUDED.includes(lone.effect)
      && !previous.condition && !previous.bindings ? lone : undefined;
    const single = lone && !previous.condition && !previous.bindings && !lone.optional && !lone.forEach
      && !SINGLE_EXCLUDED.includes(lone.effect) ? lone : optionalLone;
    if (!single && previous.steps.some(step => step.optional || step.forEach || WHOLE_EXCLUDED.includes(step.effect))) return null;
    let condition: Condition | null;
    let bodyTokens: Token[];
    let end: number;
    let leadingInstead = false;
    if (c.word("if") !== null) {
      condition = parseCondition(c);
      if (!condition || !c.punct(",")) return null;
      leadingInstead = c.word("instead") !== null;
      // "instead only you draw a card" (Kumena's Awakening): "only" restates
      // that the replacement is all that happens, which the replaced original
      // already expresses.
      if (leadingInstead) c.word("only");
      end = sentenceEnd(c);
      if (end < 0) return null;
      // "instead destroy that creature. It can't be regenerated." (Toxic
      // Stench): a following rider that the instruction grammar folds into
      // the replacement's own instruction belongs to the replacement.
      if (leadingInstead) {
        for (let next = end, extra = 0; extra < 2 && next < c.tokens.length - 1; extra++) {
          const from = new Cursor(c.tokens);
          from.pos = next + 1;
          next = sentenceEnd(from);
          if (next < 0) break;
          if (oneSentence(c.tokens.slice(c.pos, next), parseBody)) end = next;
        }
      }
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
    if (!single && !leadingInstead && (!lone || lone.effect === "simultaneous")) return null;
    const body = new Cursor(bodyTokens);
    const parsed = parseBody(body);
    const sentence = parsed?.length === 1 && body.done() && !parsed[0].otherwise ? parsed[0] : undefined;
    let instead: Effect[] | undefined;
    let wholeInstead: Sentence | undefined;
    let explicitReplacementTargets = false;
    if (sentence && !sentence.condition && !sentence.bindings) {
      instead = sentence.steps;
      // "That creature gets -4/-4 instead" restates only the modifier; a
      // replacement of the same pump keeps its duration (CR 611.2a). Any
      // other lasting replacement must print its own duration.
      if (single?.duration && instead.some(step => !step.duration && LASTING.has(step.effect))) {
        const only = instead[0];
        if (instead.length !== 1 || only.effect !== "pump" || single.effect !== "pump") return null;
        instead = [{ ...only, duration: structuredClone(single.duration) }];
      }
      explicitReplacementTargets = containsExplicitTargets(instead);
    }
    else if (sentence) {
      // A replacement with its own condition or definition of X.
      instead = sentence.steps;
      wholeInstead = sentence;
      explicitReplacementTargets = containsExplicitTargets(sentence);
    }
    else {
      const parsedAbbreviation = single ? parseAbbreviated(bodyTokens, single) : null;
      if (!parsedAbbreviation) return null;
      const abbreviated = parsedAbbreviation.effect;
      instead = [abbreviated];
      if (parsedAbbreviation.bindings) wholeInstead = { steps: instead, bindings: parsedAbbreviation.bindings };
      explicitReplacementTargets = containsExplicitTargets(abbreviated.effect === "damage"
        ? { source: abbreviated.source, amount: abbreviated.amount } : abbreviated.effect === "create-token" ? [abbreviated.count, parsedAbbreviation.bindings]
        : abbreviated.effect === "prevent-damage" ? abbreviated.prevention.shield : abbreviated);
    }
    if (!instead?.length) return null;
    // "If A, X. If B, Y instead." leaves open whether Y still depends on A.
    // A conditional sentence is replaced as a whole only by a sentence that
    // states its own condition ("destroy it if its mana value is 4 or less
    // instead"); an "unless" alternative always happens in one of its forms.
    if (previous.condition && previous.conditionKind !== "unless" && !wholeInstead?.condition) return null;
    // A whole replaced sentence lends no duration to its replacement.
    if ((!single || wholeInstead) && previous.steps.some(step => step.duration)
      && instead.some(step => !step.duration && LASTING.has(step.effect))) return null;
    // CR 601.2c permits alternative targets only for an alternative or
    // additional cost chosen while casting: kicker (702.33g), bargain
    // (702.166d), gift (702.174m), teamwork (702.194c), or the spell's own
    // optional additional cost. A generic resolution-time condition cannot
    // silently change the set of announced targets.
    const thisSpell = (what: { ref: string; noun?: string } | undefined) => what?.ref === "this" && what.noun === "spell";
    const byKicker = condition.condition === "kicked" && thisSpell(condition.what);
    const byCost = (condition.condition === "keyword-choice" && ["bargain", "gift", "teamwork"].includes(condition.keyword)
      && (!condition.what || thisSpell(condition.what))) || (condition.condition === "additional-cost-paid" && "what" in condition && thisSpell(condition.what));
    const targetPolicy = !explicitReplacementTargets ? "retain-original" : byKicker ? "alternative-by-kicker" : "alternative-by-cost";
    if (explicitReplacementTargets && !byKicker && !byCost) return null;

    let replacementChoice: Extract<Effect, { effect: "self-replacement" }>["replacementChoice"];
    const chooser = new Cursor(bodyTokens);
    const who = parsePlayerRef(chooser);
    const printedMay = who && chooser.word("may") !== null;
    const first = instead[0];
    const optional = first.optional || first.effect === "optional-group";
    if (optionalLone && (!leadingInstead || !printedMay || JSON.stringify(who) !== JSON.stringify({ player: "you" }))) return null;
    if (printedMay || optional || instead.some(step => step.optional || step.effect === "optional-group")) {
      // "If ..., you may X instead" makes replacement application optional.
      // The printed chooser is distinct from X's affected player/recipient.
      if (!printedMay || !who || instead.length !== 1 || !optional || wholeInstead) return null;
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
    if (replacementChoice && targetPolicy !== "retain-original") return null;
    c.pos = end < c.tokens.length ? end + 1 : end;
    return { effect: "self-replacement", original: single ?? { sequence: [{ ...previous }] }, condition,
      instead: wholeInstead ? { sequence: [wholeInstead] } : instead, targetPolicy, ...(replacementChoice ? { replacementChoice } : {}) };
  });
}
