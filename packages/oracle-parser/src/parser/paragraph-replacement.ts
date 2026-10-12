// CR 614.15: "The text creating a self-replacement effect is usually part of
// the ability whose effect is being replaced, but the text can be a separate
// ability, particularly when preceded by an ability word." Such a paragraph
// has no instruction of its own: it is parsed against the spell ability it
// modifies and forms one block with it (Brimstone Volley, Mirran Mettle).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, Sentence } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { isKeywordName } from "./keywords.ts";
import { parseSelfReplacement } from "./self-replacement.ts";

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
  if (first.kind !== "spell" || first.effects.length !== 1) return null;
  const split = splitAbilityWord(paragraph);
  if (!split) return null;
  const c = new Cursor(split.rest);
  const replacement = parseSelfReplacement(c, first.effects[0], parseBody);
  if (!replacement || replacement.effect !== "self-replacement" || !c.done()) return null;
  if (split.abilityWord) replacement.abilityWord = split.abilityWord;
  return { ...first, effects: [{ ...first.effects[0], steps: [replacement] }] };
}
