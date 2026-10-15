// Predicates about how many colors an object is, after "that's" / "is".
// CR 105.1–105.2: an object is zero or more of the five colors; one that is
// none of them is colorless.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

/** "one or more colors" is every object that isn't colorless; "all colors"
 * is every object that is each of the five colors. */
export function parseColorSetPredicate(c: Cursor): ObjectFilter | null {
  if (c.words("one", "or", "more", "colors")) return { colorless: false };
  if (c.words("all", "colors")) return { colors: ["white", "blue", "black", "red", "green"] };
  return null;
}
