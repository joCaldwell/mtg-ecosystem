// CR 614.15: "The text creating a self-replacement effect is usually part of
// the ability whose effect is being replaced, but the text can be a separate
// ability, particularly when preceded by an ability word." Such a paragraph
// has no instruction of its own: it is parsed against the spell ability it
// modifies and forms one block with it (Brimstone Volley, Mirran Mettle).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, Effect, Sentence } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { isKeywordName } from "./keywords.ts";
import { parseCount } from "./refs.ts";
import { parseSelfReplacement, withSelfReplacement } from "./self-replacement.ts";

type BodyParser = (c: Cursor) => Sentence[] | null;

/** Mechanical labels are never cosmetic ability words (see tryAbilityWord). */
const MECHANICAL_LABELS = new Set(["visit", "max speed", "boast", "exhaust", "power-up", "forecast"]);

function splitAbilityWord(tokens: Token[]): { abilityWord?: string; rest: Token[] } | null {
  for (let i = 0; i <= 4 && i < tokens.length; i++) {
    const token = tokens[i];
    if (token.kind === "punct" && token.value === "—") {
      if (i === 0) return null;
      const abilityWord = tokens.slice(0, i).map(t => t.kind === "word" ? t.raw : "").join(" ");
      const name = abilityWord.toLowerCase();
      return MECHANICAL_LABELS.has(name) || isKeywordName(name) ? null : { abilityWord, rest: tokens.slice(i + 1) };
    }
    if (token.kind !== "word") break;
  }
  return { rest: tokens };
}

/** The replaced ability must be one unconditional spell instruction; a
 * paragraph of several sentences has no unambiguous replaced part. The
 * replacement paragraph must be exactly one self-replacement sentence. */
export function mergeReplacementParagraph(first: Ability, paragraph: Token[], parseBody: BodyParser): Ability | null {
  if (first.kind !== "spell" || !first.effects.length) return null;
  const split = splitAbilityWord(paragraph);
  if (!split) return null;
  const search = replaceSearchQuantity(first, split.rest, parseBody, split.abilityWord);
  if (search) return search;
  if (first.effects.length !== 1) return null;
  const c = new Cursor(split.rest);
  const replacement = parseSelfReplacement(c, first.effects[0], parseBody);
  if (!replacement || replacement.effect !== "self-replacement" || !c.done()) return null;
  if (split.abilityWord) {
    replacement.abilityWord = split.abilityWord;
    c.foldTokens(replacement, paragraph.slice(0, paragraph.length - split.rest.length));
  }
  return { ...first, effects: [withSelfReplacement(first.effects[0], replacement)] };
}


/** CR 614.15: replace the quantity of the preceding search only. Both
 * branches still reveal and place the actual found cards, then shuffle.
 * https://magic.wizards.com/en/news/feature/magic-origins-release-notes-2015-07-08 */
function replaceSearchQuantity(spell: Extract<Ability, { kind: "spell" }>, tokens: Token[], parseBody: BodyParser,
  abilityWord?: string): Ability | null {
  const split = tokens.findIndex(t => t.kind === "word" && !t.possessive && t.value === "instead");
  if (split < 0) return null;
  const tail = new Cursor(tokens.slice(split + 1));
  if (tail.word("of") === null) return null;
  const originalAmount = parseCount(tail);
  tail.punct(".");
  if (originalAmount?.amount !== "fixed" || !tail.done()) return null;
  const original = spell.effects[0];
  const [search] = original.steps;
  if (original.condition || original.otherwise || original.bindings || search?.effect !== "search"
      || search.optional || search.forEach || search.for.ref !== "filter" || !search.for.upTo
      || search.for.count.amount !== "fixed" || search.for.count.value !== originalAmount.value
      || original.steps.slice(1).some(step => step.effect === "search" || step.effect === "self-replacement")) return null;
  const head = new Cursor(tokens.slice(0, split));
  const branch = parseBody(head);
  if (!branch || !head.done() || branch.length !== 1) return null;
  const { condition, conditionKind, steps, otherwise, bindings } = branch[0];
  const [newSearch] = steps;
  if (!condition || conditionKind !== "if" || otherwise || bindings || steps.length !== 1
      || newSearch.effect !== "search" || newSearch.for.ref !== "filter" || newSearch.for.count.amount !== "fixed") return null;
  const normalized = { ...newSearch, for: { ...newSearch.for, count: originalAmount } };
  if (JSON.stringify(normalized) !== JSON.stringify(search)) return null;
  const replacement: Effect = { effect: "self-replacement", original: search,
    condition, instead: [newSearch], targetPolicy: "retain-original", ...(abilityWord ? { abilityWord } : {}) };
  const effects = [{ ...original, steps: [replacement, ...original.steps.slice(1)] }, ...spell.effects.slice(1)];
  return { ...spell, effects };
}
