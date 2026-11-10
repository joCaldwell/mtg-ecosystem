// Line-level classification and the card-level entry point.
//
// One oracle-text line = one ability. Classification order:
//   1. loyalty prefix ("+1:", "-X:", "0:")
//   2. ability word prefix ("Landfall — …")
//   3. triggered ("When/Whenever/At …")
//   4. activated (a top-level ":" splitting costs from effects)
//   5. keyword line ("Flying, ward {2}")
//   6. static ("Creatures you control get +1/+1.")
//   7. spell text (imperative sentences)
// Anything else fails loudly with the farthest-failure diagnostic.

import type { Ability, CostModification, ParseCardResult, ParsedLine, Sentence, TriggeredAbility, PaymentRestriction, TriggerLimit, VariableConstraint, VariableDefinition } from "../ast.ts";
import { LexError, lex, type Token } from "../lexer.ts";
import { normalizeOracleText } from "../normalize.ts";
import type { CardNameCatalog } from "../card-names.ts";
import { Cursor } from "./cursor.ts";
import { parseActivationCostList, parseCostList } from "./costs.ts";
import { type EffectContext, parseSentences, registerLineParser } from "./effects.ts";
import { isKeywordName, parseKeywordLine } from "./keywords.ts";
import { parseCondition } from "./refs.ts";
import { hasUnboundStaticOther, hasUnsupportedStaticConditional, hasUnsupportedStaticTargets, parseStatic } from "./statics.ts";
import { hasAlternativeKickerTargets } from "./self-replacement.ts";
import { hasCastingCostModes } from "./modal.ts";
import { prepareStackCopyInput, type StackCopyInputProof } from "./stack-copy.ts";
import { finalizeCharacteristicDefinitions } from "./characteristic-origin.ts";
import { attachDiceTable, parseDiceTableRows } from "./dice.ts";
import { parseClassCard, parseLevelCard } from "./level-cards.ts";
import { parseStationCard } from "./station-cards.ts";
import { parseStickerSheet } from "./sticker-sheets.ts";
import { hasBrackets, hasCleaveKeyword, parseCleaveParagraph } from "./cleave.ts";
import { parseCaseAbility, startsCaseKeyword } from "./case-cards.ts";
import { parseTrigger, parseTriggerLimit } from "./triggers.ts";
import { parseDelayedSpellPrefix } from "./delayed.ts";
import { parseActivationCostModifications, parseAdditionalCostModifications } from "./cost-modifiers.ts";
import { looksLikeStaticReplacement } from "./replacement.ts";
import { mergeReplacementParagraph } from "./paragraph-replacement.ts";
import { parseActionLimit, parseActivationRules, type ActivationRules } from "./restrictions.ts";
import { mentionsX, parseVariableConstraints } from "./variable-constraints.ts";
import { hasInstructionBinding, parseVariableDefinitions } from "./variable-definitions.ts";
import { parsePaymentRestriction } from "./payment-restrictions.ts";
import { looksLikeStaticPlayPermission } from "./permissions.ts";
import { parseExertOnAttack } from "./status-actions.ts";
import { parseLinkedReveal } from "./linked-reveal.ts";
import { hasUnresolvedKeywordCost } from "./ability-grants.ts";
import { parseAnchorWords } from "./anchor-words.ts";
import { splitFlavorLabel } from "./flavor-labels.ts";
import { parseRingBearerChoice } from "./ring-choice.ts";
import { withConditionDifference } from "./condition-differences.ts";
import { parseSameIsTrueFor } from "./same-is-true.ts";
import { parseCommanderPregameChoice } from "./pregame-choice.ts";

// ---------------------------------------------------------------------------

class LineSession {
  readonly tokens: Token[];
  readonly cursors: { cursor: Cursor; offset: number }[] = [];
  constructor(tokens: Token[]) { this.tokens = tokens; }
  cursor(tokens: Token[]): Cursor {
    const cursor = new Cursor(tokens);
    const offset = tokens.length ? this.tokens.indexOf(tokens[0]) : this.tokens.length;
    this.cursors.push({ cursor, offset });
    return cursor;
  }
  error(): string {
    const best = this.cursors.reduce((a, b) =>
      b.offset + b.cursor.farthest > a.offset + a.cursor.farthest ? b : a);
    return best.cursor.errorMessage(best.offset);
  }
}

type EffectsBlock = { sentences: Sentence[]; costModifications?: CostModification[]; triggerLimit?: TriggerLimit;
  variableConstraints?: VariableConstraint[]; paymentRestriction?: PaymentRestriction; variableDefinitions?: VariableDefinition[];
  actionLimit?: TriggeredAbility["actionLimit"] } & ActivationRules;
/** CR 603.2h presupposes an action its controller can decline to take. */
function hasOptionalAction(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(hasOptionalAction);
  if (!value || typeof value !== "object") return false;
  if ("kind" in value) return false; // a quoted ability's own choices
  if (("optional" in value && value.optional === true) || ("effect" in value &&
      (value.effect === "optional-group" || value.effect === "grant-play-permission"))) return true;
  return Object.values(value).some(hasOptionalAction);
}
function parseEffectsBlock(c: Cursor, options: Token[][], stackCopyInput?: StackCopyInputProof, triggered = false, definitions = false): EffectsBlock | null {
  return c.attempt((c): EffectsBlock | null => {
    const ctx: EffectContext = { modalOptions: options, ...(stackCopyInput ? { stackCopyInput } : {}) };
    const parsed = parseSentences(c, ctx);
    if (!parsed) return null;
    // Only callers that keep it (activated abilities) ask for a definition.
    const variableDefinitions = definitions && !hasInstructionBinding(parsed) ? parseVariableDefinitions(c) : null;
    const sentences = parsed;
    // Activation rules after the definition are not yet supported.
    if (variableDefinitions) return c.done() ? { sentences, variableDefinitions } : null;
    if (c.done()) return { sentences };
    const actionLimit = triggered && hasOptionalAction(sentences) ? parseActionLimit(c) : null;
    if (actionLimit) return { sentences, actionLimit };
    // CR 107.3: "X can't be 0." limits the chosen X; it precedes any
    // activation rules printed after it.
    const variableConstraints = parseVariableConstraints(c);
    // CR 601.2h / 602.2b: "Spend only black mana on X." limits the payment.
    const paymentRestriction = parsePaymentRestriction(c, "x-only");
    const payment = { ...(variableConstraints ? { variableConstraints } : {}), ...(paymentRestriction ? { paymentRestriction } : {}) };
    if ((variableConstraints || paymentRestriction) && c.done()) return { sentences, ...payment };
    const costModifications = parseActivationCostModifications(c);
    const triggerLimit = parseTriggerLimit(c);
    const rules = parseActivationRules(c);
    if (c.done() && (costModifications || triggerLimit || rules)) return {
      sentences, ...payment,
      ...(costModifications ? { costModifications } : {}), ...(triggerLimit ? { triggerLimit } : {}), ...rules,
    };
    return null;
  });
}

function parseLineTokens(tokens: Token[], options: Token[][], session = new LineSession(tokens)): Ability | null {
  const parsed = parseLineTokensUnchecked(tokens, options, session);
  const ability = parsed && finalizeCharacteristicDefinitions(parsed);
  // Kicker alternative targets are chosen while casting that spell, never
  // when a triggered/activated ability happens to resolve (601.2c/702.33g).
  if (ability && ability.kind !== "spell" && (hasAlternativeKickerTargets(ability) || hasCastingCostModes(ability))) return null;
  // A granted cost keyword must have its cost sentence folded in.
  if (ability && hasUnresolvedKeywordCost(ability)) return null;
  return ability;
}

function parseLineTokensUnchecked(tokens: Token[], options: Token[][], session: LineSession): Ability | null {
  // CR 614.12c: bullets after an anchor-word choice are abilities, not modes.
  if (options.length) {
    const anchors = parseAnchorWords(tokens, options, child => parseLineTokens(child, [], session));
    if (anchors) return anchors;
  }
  const repeated = parseSameIsTrueFor(tokens, child => parseLineTokensUnchecked(child, options, session));
  if (repeated) return repeated;
  // Mechanical Case keywords cannot fall through to inert ability words.
  if (startsCaseKeyword(tokens)) return parseCaseAbility(session.cursor(tokens),
    child => parseLineTokens(child, options, session));
  // Chapter symbols are triggered abilities, never flavor words (CR 714.2).
  // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
  const roman = new Map([['i', 1], ['ii', 2], ['iii', 3], ['iv', 4], ['v', 5], ['vi', 6], ['vii', 7], ['viii', 8], ['ix', 9], ['x', 10]]);
  // A chapter number is followed by "," or "—"; "X target creatures ..." is
  // an ordinary sentence whose X is a number.
  if (tokens[0]?.kind === "word" && roman.has(tokens[0].value) && tokens[1]?.kind === "punct" && [",", "—"].includes(tokens[1].value)) {
    const c = session.cursor(tokens);
    const chapters: number[] = [];
    for (;;) {
      const word = c.anyWord();
      const number = word && roman.get(word.value);
      if (!number) return null;
      chapters.push(number);
      if (!c.punct(",")) break;
    }
    if (!c.punct("—")) return null;
    const bodyStart = c.pos;
    const block = parseEffectsBlock(c, options);
    if (block) return !block.restriction && !block.permission && !block.costModifications && !block.triggerLimit && !block.variableConstraints && !block.paymentRestriction ? { kind: "chapter", chapters, effects: block.sentences } : null;
    // CR 207.2c–d: a chapter ability may carry an ability word or flavor
    // word of its own ("I — Aerospark — Exile target creature …").
    const labeled = splitFlavorLabel(tokens.slice(bodyStart));
    if (!labeled) return null;
    const body = parseEffectsBlock(session.cursor(labeled.rest), options);
    return body && !body.restriction && !body.permission && !body.costModifications && !body.triggerLimit && !body.variableConstraints && !body.paymentRestriction
      ? { kind: "chapter", chapters, abilityWord: labeled.label, effects: body.sentences } : null;
  }
  // CR 702.159a: "Visit — [effect]" is the Attraction's triggered visit
  // ability; its body is resolution text, like a chapter's.
  if (tokens[0]?.kind === "word" && !tokens[0].possessive && tokens[0].value === "visit" && tokens[1]?.kind === "punct" && tokens[1].value === "—") {
    const c = session.cursor(tokens);
    c.pos = 2;
    const block = parseEffectsBlock(c, options);
    return block && !block.restriction && !block.permission && !block.costModifications && !block.triggerLimit && !block.variableConstraints ? { kind: "visit", effects: block.sentences } : null;
  }
  // Keyword parameters after a dash are rules, not cosmetic ability words.
  const keywords = parseKeywordLine(session.cursor(tokens));
  if (keywords) return { kind: "keywords", keywords };
  // 1. Loyalty ability
  const loyalty = tryLoyalty(tokens, options, session);
  if (loyalty) return loyalty;

  // 2. Ability word prefix: leading words followed by "—", where what follows
  //    parses as a full ability. (Modal headers also contain "—" but end with it.)
  const abilityWord = tryAbilityWord(tokens, options, session);
  if (abilityWord) return abilityWord;
  // CR 207.2d: flavor words are free text ("Avalanche!", "Nitro-9", "In You,
  // All Things Are Possible"). Such a label is accepted only on an ability
  // that records it; a keyword line after it could be a parameterized rule.
  const flavor = splitFlavorLabel(tokens);
  if (flavor) {
    const ability = parseLineCore(flavor.rest, options, flavor.label, session);
    if (ability && (ability.kind === "triggered" || ability.kind === "activated" || ability.kind === "static" || ability.kind === "spell")
      && ability.abilityWord === flavor.label) return ability;
  }

  return parseLineCore(tokens, options, undefined, session);
}

function tryLoyalty(tokens: Token[], options: Token[][], session: LineSession): Ability | null {
  let sign: 1 | -1 | 0 = 0;
  let i = 0;
  // CR 107.7, 701.71a: within rules text a loyalty symbol is written in
  // square brackets ("[-3]: ..."); it is the same cost as a printed symbol.
  const bracketed = tokens[0]?.kind === "punct" && tokens[0].value === "[";
  if (bracketed) i = 1;
  const first = tokens[i];
  if (first?.kind === "punct" && (first.value === "+" || first.value === "-")) {
    sign = first.value === "+" ? 1 : -1;
    i++;
  }
  const num = tokens[i];
  if (bracketed) {
    const close = tokens[i + 1];
    if (close?.kind !== "punct" || close.value !== "]") return null;
    i++;
  }
  const colon = tokens[i + 1];
  const isAmount =
    num && ((num.kind === "number") || (num.kind === "word" && num.value === "x"));
  if (!isAmount || colon?.kind !== "punct" || colon.value !== ":") return null;
  if (sign === 0 && !(num.kind === "number" && num.value === 0)) return null;
  const c = session.cursor(tokens.slice(i + 2));
  const block = parseEffectsBlock(c, options);
  if (!block || block.triggerLimit || block.variableConstraints || block.paymentRestriction) return null;
  return {
    kind: "loyalty",
    cost: { sign, amount: num.kind === "number" ? num.value : "x" },
    effects: block.sentences,
    ...(block.costModifications ? { costModifications: block.costModifications } : {}),
    ...(block.restriction ? { restriction: block.restriction } : {}),
    ...(block.permission ? { permission: block.permission } : {}),
  };
}

function tryAbilityWord(tokens: Token[], options: Token[][], session: LineSession): Ability | null {
  // Find an early "—" (within the first 4 tokens), preceded only by words.
  let dash = -1;
  for (let i = 1; i <= 4 && i < tokens.length; i++) {
    const t = tokens[i];
    if (t.kind === "punct" && t.value === "—") {
      dash = i;
      break;
    }
    // CR 207.2c: "descend 4" and "descend 8" are the ability words that
    // contain a number; it is part of the label, not a parameter.
    const previous = tokens[i - 1];
    if (t.kind === "number" && (t.value === 4 || t.value === 8) && i === 1
      && previous.kind === "word" && previous.value === "descend") continue;
    if (t.kind !== "word") return null;
  }
  if (dash < 1 || tokens[0].kind !== "word") return null;
  const word = tokens
    .slice(0, dash)
    .map((t) => (t.kind === "word" ? (t.possessive ? `${t.raw}${t.possessiveSuffix ?? "'s"}` : t.raw) : t.kind === "number" ? t.raw : ""))
    .join(" ");
  const rest = tokens.slice(dash + 1);
  if (rest.length === 0) return null;
  const name = word.toLowerCase();
  // Visit labels define Attraction triggers, not cosmetic flavor words.
  if (name === "visit") return null;
  // CR 702.178: a conditional ability grant, not an ability word.
  if (name === "max speed") {
    const ability = parseLineTokens(rest, options, session);
    return ability ? { kind: "max-speed", ability } : null;
  }
  // CR 702.186b: an ability held only while the permanent is harnessed.
  if (name === "∞") {
    const ability = parseLineTokens(rest, options, session);
    return ability ? { kind: "infinity", ability } : null;
  }
  // CR 702.57 / 702.142 / 702.177 / 702.193: these name activated abilities
  // and impose intrinsic timing/zone/history/payment rules. Forecast also
  // keeps its source revealed beyond paying its printed reveal cost.
  // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
  if (name === "boast" || name === "exhaust" || name === "power-up" || name === "forecast") {
    const ability = parseLineCore(rest, options, undefined, session);
    return ability?.kind === "activated" ? { ...ability, keyword: name } : null;
  }
  if (isKeywordName(name)) return null;
  const ability = parseLineCore(rest, options, word, session);
  // The label must be recorded on the ability, never silently dropped.
  if (ability && !("abilityWord" in ability && ability.abilityWord === word)) return null;
  return ability;
}

function parseLineCore(tokens: Token[], options: Token[][], abilityWord: string | undefined, session: LineSession): Ability | null {
  // "As an additional cost to cast this spell, <cost>."
  {
    const c = session.cursor(tokens);
    if (c.words("as", "an", "additional", "cost", "to", "cast", "this", "spell") && c.punct(",")) {
      const optional = c.words("you", "may");
      const costs = parseCostList(c);
      c.punct(".");
      const variableConstraints = costs && !c.done() && mentionsX(costs) ? parseVariableConstraints(c) : null;
      const costModifications = costs && !c.done() ? parseAdditionalCostModifications(c) : null;
      if (costs && c.done()) return { kind: "additional-cost", ...(abilityWord ? { abilityWord } : {}), costs, ...(optional ? { optional: true as const } : {}),
        ...(variableConstraints ? { variableConstraints } : {}),
        ...(costModifications ? { costModifications } : {}) };
      return null;
    }
  }

  // 3. Triggered
  if (tokens[0]?.kind === "word" && ["when", "whenever", "at"].includes(tokens[0].value)) {
    const c = session.cursor(tokens);
    const trigger = parseTrigger(c);
    if (trigger && !hasMatchTimeTarget(trigger) && c.punct(",")) {
      // Intervening if: "When ~ dies, if <cond>, <effects>"
      const condition = c.attempt((cc) => {
        if (cc.word("if") === null) return null;
        // CR 701.54a: only a temptation event carries a Ring-bearer choice.
        const cond = (trigger.trigger === "ring-tempts" && trigger.who.player === "you" ? parseRingBearerChoice(cc) : null) ?? parseCondition(cc);
        if (!cond || !cc.punct(",")) return null;
        return cond;
      });
      // CR603.3d chooses targets after the trigger has matched; CR603.4
      // tests this condition already at trigger time. A fresh target cannot
      // establish either predicate. Do not backtrack into a resolving `if`.
      if (condition && hasMatchTimeTarget(condition)) return null;
      const block = withConditionDifference(condition ?? undefined, () => parseEffectsBlock(c, options, condition ? undefined : prepareStackCopyInput(c, trigger), true));
      if (block && !block.restriction && !block.permission && !block.costModifications && !block.paymentRestriction &&
          (!block.variableConstraints || mentionsX(block.sentences))) {
        return {
          kind: "triggered",
          abilityWord,
          trigger,
          condition: condition ?? undefined,
          effects: block.sentences,
          ...(block.triggerLimit ? { limit: block.triggerLimit } : {}),
          ...(block.variableConstraints ? { variableConstraints: block.variableConstraints } : {}),
          ...(block.actionLimit ? { actionLimit: block.actionLimit } : {}),
        };
      }
    }
    const delayedCursor = session.cursor(tokens);
    const delayed = parseDelayedSpellPrefix(delayedCursor, { modalOptions: options }, parseSentences);
    if (delayed && delayedCursor.done()) return { kind: "spell", abilityWord, effects: [{ steps: delayed }] };
    return null;
  }

  // 4. Activated: top-level colon outside quotes.
  let depth = 0;
  let colonAt = -1;
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (t.kind === "punct" && t.value === '"') depth ^= 1;
    else if (depth === 0 && t.kind === "punct" && t.value === ":") {
      colonAt = i;
      break;
    } else if (depth === 0 && t.kind === "punct" && t.value === ".") break;
  }
  if (colonAt > 0) {
    const costCursor = session.cursor(tokens.slice(0, colonAt));
    const costs = parseActivationCostList(costCursor);
    if (costs && costCursor.done()) {
      const c = session.cursor(tokens.slice(colonAt + 1));
      const block = parseEffectsBlock(c, options, undefined, false, true);
      if (block && !block.triggerLimit && (!block.variableConstraints || mentionsX(costs) || mentionsX(block.sentences)) &&
          (!block.paymentRestriction || mentionsX(costs)) && (!block.variableDefinitions || mentionsX(costs) || mentionsX(block.sentences))) {
        return {
          kind: "activated",
          abilityWord,
          costs,
          effects: block.sentences,
          ...(block.costModifications ? { costModifications: block.costModifications } : {}),
          ...(block.variableConstraints ? { variableConstraints: block.variableConstraints } : {}),
          ...(block.variableDefinitions ? { variableDefinitions: block.variableDefinitions } : {}),
          ...(block.paymentRestriction ? { paymentRestriction: block.paymentRestriction } : {}),
          restriction: block.restriction,
          ...(block.permission ? { permission: block.permission } : {}),
        };
      }
    }
    return null;
  }

  // 5. Keyword line
  {
    const c = session.cursor(tokens);
    const keywords = parseKeywordLine(c);
    if (keywords) return { kind: "keywords", ...(abilityWord ? { abilityWord } : {}), keywords };
  }

  // 5b. Exert as an attack cost (with its linked "When you do" ability)
  {
    const c = session.cursor(tokens);
    const exert = parseExertOnAttack(c, body => parseSentences(body, { modalOptions: [] }));
    if (exert && c.done()) return exert;
  }

  // 5c. A static ability with its linked triggered abilities (CR 603.11)
  {
    const linked = parseLinkedReveal(tokens, body => {
      const block = parseEffectsBlock(session.cursor(body), options, undefined, true);
      return block && Object.keys(block).length === 1 ? block.sentences : null;
    });
    if (linked) return abilityWord ? null : linked;
  }

  // 6. Static
  {
    const c = session.cursor(tokens);
    const effect = parseCommanderPregameChoice(c, parseStatic);
    if (effect && c.done()) return { kind: "static", abilityWord, effect };
  }
  {
    const c = session.cursor(tokens);
    const effect = parseStatic(c);
    if (effect && c.done()) return { kind: "static", abilityWord, effect };
  }

  // 7. Spell text
  {
    const c = session.cursor(tokens);
    if (looksLikeStaticPlayPermission(c) || hasUnboundStaticOther(c) || hasUnsupportedStaticConditional(c) || hasUnsupportedStaticTargets(c) || looksLikeStaticReplacement(c)) return null;
    const block = parseEffectsBlock(c, options);
    if (block && !block.restriction && !block.permission && !block.costModifications && !block.triggerLimit && !block.paymentRestriction &&
        (!block.variableConstraints || mentionsX(block.sentences))) return { kind: "spell", abilityWord, effects: block.sentences,
      ...(block.variableConstraints ? { variableConstraints: block.variableConstraints } : {}) };
  }

  return null;
}

/** Nested granted abilities own their own target declarations. Delayed
 * triggers use delayed.ts's distinct capture/announcement policy instead.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function hasMatchTimeTarget(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if ("kind" in value) return false;
  if ("ref" in value && (value.ref === "target" || value.ref === "any-target")) return true;
  if ("player" in value && (value.player === "target-player" || value.player === "target-opponent")) return true;
  return Object.values(value).some(hasMatchTimeTarget);
}

function containsModal(ability: Ability): boolean {
  if (ability.kind === "anchor-words") return true; // its bullets are its own options
  if (ability.kind === "max-speed" || ability.kind === "infinity" || ability.kind === "case-solved") return containsModal(ability.ability);
  // CR 603.12, 700.2b: "When you do, choose one —" makes the bullets the
  // modes of the reflexive triggered ability this ability creates.
  const owns = (sentences: Sentence[]): boolean => sentences.some(sentence => sentence.steps.some(step =>
    step.effect === "modal" || (step.effect === "create-reflexive-trigger" && owns(step.effects))));
  return "effects" in ability && owns(ability.effects);
}
registerLineParser((tokens, options) => parseLineTokens(tokens, options));

// ---------------------------------------------------------------------------
// Card-level API
// ---------------------------------------------------------------------------

/** Diagnostic-bearing single-line parse. */
function parseLineWithDiagnostics(text: string, optionTokens: Token[][], sourceId?: string, cardNames?: CardNameCatalog, cleave = false): ParsedLine {
  let tokens: Token[];
  try {
    tokens = lex(text, sourceId, cardNames);
  } catch (e) {
    if (e instanceof LexError) {
      return { text, ok: false, error: `${e.message} at position ${e.position}` };
    }
    throw e;
  }
  const session = new LineSession(tokens);
  const ability = cleave && hasBrackets(tokens) ? (optionTokens.length ? null : parseCleaveParagraph(tokens, t => parseLineTokens(t, [], session)))
    : parseLineTokens(tokens, optionTokens, session);
  if (ability) {
    if (optionTokens.length && !containsModal(ability))
      return { text, ok: false, error: "modal bullets require a consuming modal header" };
    return { text, ok: true, ability };
  }
  // Unpaired, empty or nested cleave brackets fail before any parse attempt.
  return { text, ok: false, error: session.cursors.length ? session.error() : "malformed square brackets in cleave text" };

}

function mergeFollowingReplacement(first: Ability, next: string, sourceId?: string, cardNames?: CardNameCatalog): Ability | null {
  if (!/\binstead\b/i.test(next) || parseLineWithDiagnostics(next, [], sourceId, cardNames).ok) return null;
  let tokens: Token[];
  try { tokens = lex(next, sourceId, cardNames); } catch (error) {
    if (error instanceof LexError) return null;
    throw error;
  }
  return mergeReplacementParagraph(first, tokens, body => parseSentences(body, { modalOptions: [] }));
}

export interface ParseContext {
  layout?: string;
  typeLine?: string;
  /** Preserve printed target-selector identities for the later binding pass. */
  includeSourceProvenance?: boolean;
  /** Built once from the full input snapshot, including face and token names.
   * This lexical lookup never changes card eligibility or acceptance scope. */
  cardNames?: CardNameCatalog;
}

/** A results row starts with numeric bounds and a printed table separator.
 * This recognizes the block boundary only; dice.ts validates every bound,
 * effect and overlap before the complete ability can pass (CR 706.3b). */
function startsDiceRow(line: string): boolean {
  return /^(?:•\s*)?-?\d+(?:\s*(?:[—-]\s*-?\d+|\+|or\s+(?:less|lower|fewer|higher|greater|more|\d+)))*\s*(?:\||—)/i.test(line);
}

function parseAbilityBlocks(lines: readonly string[], startIndex: number, context: ParseContext): ParsedLine[] {
  const results: ParsedLine[] = [];
  const sourceId = (index: number) => context.includeSourceProvenance ? `line:${startIndex + index}` : undefined;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // A dice results table is part of its roll's original ability. It is
    // neither a set of separately granted abilities nor a choice of modes.
    if (i + 1 < lines.length && startsDiceRow(lines[i + 1])) {
      let end = i + 1;
      while (end < lines.length && startsDiceRow(lines[end])) end++;
      const text = lines.slice(i, end).join("\n");
      try {
        const header = parseLineWithDiagnostics(line, [], sourceId(i), context.cardNames);
        const rowTokens = lines.slice(i + 1, end).map((row, index) => lex(row, sourceId(i + index + 1), context.cardNames));
        const table = parseDiceTableRows(rowTokens, c => parseSentences(c, { modalOptions: [], allowRollAgain: true }));
        const ability = header.ok && table ? attachDiceTable(header.ability, table) : null;
        results.push(ability ? { text, ok: true, ability: finalizeCharacteristicDefinitions(ability) }
          : { text, ok: false, error: header.ok ? "unsupported or incomplete dice results table" : header.error });
      } catch (error) {
        if (!(error instanceof LexError)) throw error;
        results.push({ text, ok: false, error: `dice results table: ${error.message} at position ${error.position}` });
      }
      i = end - 1;
      continue;
    }

    // CR 702.172b uses plus icons for Spree. CR 700.2i retains every
    // pawprint in a weighted mode; these symbols are not ordinary bullets.
    const bullets: string[] = [];
    let j = i + 1;
    const paid = line.toLowerCase() === "spree";
    while (j < lines.length && (lines[j].startsWith("•") || lines[j].startsWith("{P}") || (paid && lines[j].startsWith("+")))) {
      bullets.push(lines[j]);
      j++;
    }
    if (bullets.length > 0) {
      const blockText = [line, ...bullets].join("\n");
      try {
        const optionTokens = bullets.map((b, index) => {
          const tokens = lex(b, sourceId(i + index + 1), context.cardNames);
          return b.startsWith("{P}") ? tokens : tokens.slice(1);
        });
        const parsed = parseLineWithDiagnostics(line, optionTokens, sourceId(i), context.cardNames);
        results.push({ ...parsed, text: blockText });
      } catch (error) {
        if (!(error instanceof LexError)) throw error;
        results.push({ text: blockText, ok: false, error: `modal option: ${error.message} at position ${error.position}` });
      }
      i = j - 1;
      continue;
    }

    const parsed = parseLineWithDiagnostics(line, [], sourceId(i), context.cardNames, hasCleaveKeyword(lines));
    // CR 614.15: a following paragraph may only replace this spell ability's
    // instruction. It then belongs to this block instead of failing alone.
    const merged = parsed.ok && i + 1 < lines.length && !lines[i + 2]?.startsWith("•")
      ? mergeFollowingReplacement(parsed.ability, lines[i + 1], sourceId(i + 1), context.cardNames) : null;
    if (merged) {
      results.push({ text: `${line}\n${lines[i + 1]}`, ok: true, ability: merged });
      i++;
      continue;
    }
    results.push(parsed);
  }

  return results;
}

export function parseOracleText(text: string, cardName?: string, context: ParseContext = {}): ParseCardResult {
  const lines = normalizeOracleText(text, cardName, context.cardNames, context.typeLine);
  // Dungeon room names and their graph are rules-bearing (CR 309, 701.46).
  // The text alone lacks graph edges, so ordinary flavor-word parsing cannot
  // certify these cards. They remain failures in the unchanged corpus scope.
  // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
  if (context.layout === "dungeon" || /^Dungeon\b/.test(context.typeLine ?? "")) {
    return { name: cardName ?? "", ok: false, lines: (lines.length ? lines : [""]).map(text => ({ text, ok: false, error: "unsupported dungeon room structure" })) };
  }
  const results = parseStickerSheet(lines, (section, startIndex) => parseAbilityBlocks(section, startIndex, context), context.typeLine)
    ?? parseLevelCard(lines, (section, startIndex) => parseAbilityBlocks(section, startIndex, context), context.typeLine)
    ?? parseClassCard(lines, (section, startIndex) => parseAbilityBlocks(section, startIndex, context),
      (line, index) => lex(line, context.includeSourceProvenance ? `line:${index}` : undefined, context.cardNames))
    ?? parseStationCard(lines, (section, startIndex) => parseAbilityBlocks(section, startIndex, context), text)
    ?? parseAbilityBlocks(lines, 0, context);

  // CR 113.3a: resolving spell instructions belong to instants/sorceries.
  // A permanent's unmatched continuous text must not become a one-shot
  // instruction merely because the imperative grammar accepts its words.
  // Use the face's printed card types, not subtypes or the other face's type.
  const cardTypes = context.typeLine?.split(/—|\s-\s/u, 1)[0] ?? "";
  if (/\b(?:Artifact|Battle|Creature|Enchantment|Land|Planeswalker)\b/.test(cardTypes) &&
      !/\b(?:Instant|Sorcery)\b/.test(cardTypes)) {
    for (let i = 0; i < results.length; i++) {
      const result = results[i];
      if (result.ok && result.ability.kind === "spell")
        results[i] = { text: result.text, ok: false, error: "unsupported permanent ability; resolving spell instructions require an instant or sorcery face" };
    }
  }

  const name = cardName ?? "";
  const abilities: Ability[] = [];
  for (const result of results) {
    if (!result.ok) return { name, ok: false, lines: results };
    abilities.push(result.ability);
  }
  return { name, ok: true, lines: results, abilities };
}
