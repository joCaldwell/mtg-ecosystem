// Bare plural nouns joined by "and" share one head ("Treefolk and Forests
// you control"): the trailing qualifier restricts both (CR 608.2c). Some
// subtypes print the same in the singular and the plural, so their form
// alone does not show plurality. Such a noun is read as plural only next to a
// visibly plural partner and when no singular determiner precedes the list.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Token } from "../lexer.ts";

const INVARIANT_PLURALS = new Set([
  "treefolk", "merfolk", "kithkin", "moonfolk", "kor", "vedalken", "surrakar", "sheep", "fish", "jellyfish",
  "bison", "elk", "aurochs", "astartes", "plains",
]);

const SINGULAR_DETERMINERS = new Set(["a", "an", "another", "one", "each", "target", "that", "this", "the"]);

/** Both nouns of "<noun> and <noun>" are bare plurals. `singular` is each
 * noun's singular spelling; `before` is the token ahead of the noun phrase. */
export function barePluralPair(first: Token, firstSingular: string, second: Token, secondSingular: string,
  before: Token | undefined): boolean {
  if (first.kind !== "word" || second.kind !== "word") return false;
  const visible = [first.value !== firstSingular.toLowerCase(), second.value !== secondSingular.toLowerCase()];
  if (visible.every(Boolean)) return true;
  if (!visible.some(Boolean)) return false;
  const invariant = visible[0] ? second.value : first.value;
  if (!INVARIANT_PLURALS.has(invariant)) return false;
  return !(before?.kind === "word" && SINGULAR_DETERMINERS.has(before.value));
}
