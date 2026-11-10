// "<color> is the most common color among <objects> or is tied for most
// common": a comparison of how many of the stated objects are each color.
// An object counts toward every color it is (CR 105.2); colorless objects
// count toward none. No tie policy is assumed when the text states none.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Color, Condition, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { COLORS } from "../vocab.ts";

type MostCommonColor = Extract<Condition, { condition: "most-common-color" }>;

export function parseMostCommonColorCondition(c: Cursor, g: { parseObjectRef(c: Cursor): ObjectRef | null }): MostCommonColor | null {
  return c.attempt(c => {
    const color = c.word(...COLORS);
    if (!color || !c.words("is", "the", "most", "common", "color", "among")) return null;
    const among = g.parseObjectRef(c);
    // Only a whole described group is counted, never a single referent.
    if (!among || (among.ref !== "all" && among.ref !== "each")) return null;
    const ties = c.words("or", "is", "tied", "for", "most", "common") ? "included"
      : c.words("but", "isn't", "tied", "for", "most", "common") ? "excluded" : null;
    return ties && { condition: "most-common-color", color: color as Color, among, ties };
  });
}
