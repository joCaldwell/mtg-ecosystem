// Text-changing effects (CR 612, layer 3 of 613.1c): "Change the text of
// … by replacing all instances of one color word with another."
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, TextReplacement } from "../ast.ts";
import { canonicalSubtypeName } from "../vocab.ts";
import { Cursor } from "./cursor.ts";
import { parseObjectRef } from "./refs.ts";

type ChangeText = Extract<Effect, { effect: "change-text" }>;

function parseSubtypeWord(c: Cursor): string | null {
  return c.attempt(c => {
    const word = c.anyWord();
    return word && (canonicalSubtypeName(word.raw) ?? null);
  });
}

function parseReplacement(c: Cursor): TextReplacement | null {
  return c.attempt((c): TextReplacement | null => {
    if (c.word("one") === null) return null;
    const category: TextReplacement["category"] | null = c.words("color", "word") ? "color-word"
      : c.words("basic", "land", "type") ? "basic-land-type" : c.words("creature", "type") ? "creature-type" : null;
    if (!category || c.word("with") === null) return c.fail("text category replaced with another word");
    if (c.word("another") !== null) return { category };
    // A printed new word (New Blood: "with Vampire").
    const to = category === "creature-type" ? parseSubtypeWord(c) : null;
    return to ? { category, to } : null;
  });
}

/** Alternatives joined by "or" offer one replacement, not both (CR 608.2d).
 * "The new creature type can't be Wall." restricts the chosen new word of
 * the same instruction (608.2c); it is consumed here and the sentence's
 * final period is left to the caller. A trailing duration is a rider. */
export function parseChangeText(c: Cursor): ChangeText | null {
  return c.attempt((c): ChangeText | null => {
    if (!c.words("change", "the", "text", "of")) return null;
    const what = parseObjectRef(c);
    if (!what || !c.words("by", "replacing", "all", "instances", "of")) return null;
    const first = parseReplacement(c);
    if (!first) return null;
    const replacements = [first];
    for (;;) {
      const next = c.attempt(c => c.word("or") !== null ? parseReplacement(c) : null);
      if (!next) break;
      replacements.push(next);
    }
    if (c.isWord("or")) return c.fail("complete alternative text replacement");
    const chosenType = replacements.length === 1 && first.category === "creature-type" && !first.to;
    const excluded = chosenType ? c.attempt(c => {
      if (!c.punct(".") || !c.words("the", "new", "creature", "type", "can't", "be")) return null;
      return parseSubtypeWord(c);
    }) : null;
    if (excluded) first.excluding = [excluded];
    return { effect: "change-text", what, who: { player: "you" }, replacements };
  });
}
