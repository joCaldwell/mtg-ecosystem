// CR 702.167: "Craft with [materials] [cost]" is an activated ability whose
// cost also exiles this permanent and the described materials. The materials
// stay a typed description; their zones are the keyword's own rule.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Cost, CraftMaterial, KeywordInstance } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCount, parseFilter } from "./refs.ts";

function parseMaterial(c: Cursor): CraftMaterial | null {
  return c.attempt((c): CraftMaterial | null => {
    const stated = parseCount(c);
    if (stated && stated.amount !== "fixed") return null;
    let count: Amount = stated ?? { amount: "fixed", value: 1 };
    if (stated?.amount === "fixed" && c.words("or", "more")) count = { amount: "chosen", minimum: stated.value };
    // "Craft with one or more {5}": any permanents and/or graveyard cards.
    if (stated && c.peek()?.kind === "symbol") return { count, filter: {} };
    if (stated && c.words("that", "share", "a", "card", "type")) {
      if (stated.amount !== "fixed" || stated.value < 2 || count.amount !== "fixed") return null;
      return { count, filter: {}, groupConstraints: [{ constraint: "shared-value", characteristic: "card-type" }] };
    }
    const filter = parseFilter(c);
    if (!filter) return null;
    // A bare description is one object ("Craft with artifact"); a plural one
    // always states its number.
    const noun = c.peek(-1);
    if (!stated && (noun?.kind !== "word" || noun.value.endsWith("s"))) return c.fail("number of craft materials");
    return { count, filter };
  });
}

export function parseCraft(c: Cursor, parseCost: (c: Cursor) => Cost[] | null): KeywordInstance | null {
  return c.attempt((c): KeywordInstance | null => {
    if (!c.words("craft", "with")) return null;
    const materials: CraftMaterial[] = [];
    for (;;) {
      const material = parseMaterial(c);
      if (!material) return null;
      materials.push(material);
      if (!c.punct(",")) break;
      c.word("and");
    }
    // A list names one object per description (Throne of the Grim Captain).
    if (materials.length > 1 && materials.some(material => material.count.amount !== "fixed" || material.count.value !== 1))
      return c.fail("one object per listed craft material");
    const cost = parseCost(c);
    return cost ? { keyword: "craft", materials, cost } : null;
  });
}
