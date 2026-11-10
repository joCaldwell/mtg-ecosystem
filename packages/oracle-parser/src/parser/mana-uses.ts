// Purposes other than casting a spell or activating an ability that produced
// mana can be limited to (CR 106.6): special-action costs (116.2), costs of a
// named keyword, and costs described by a symbol they contain.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ManaSpendingUse } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseFilter } from "./refs.ts";

function parseContainedSymbol(c: Cursor): "X" | "C" | null {
  const symbol = c.peek();
  if (symbol?.kind !== "symbol" || (symbol.value !== "X" && symbol.value !== "C")) return c.fail("{X} or {C}");
  c.pos++;
  return symbol.value;
}

/** `after` is the word that introduced the purpose: "to [pay ...]" or the
 * bare "on [costs that contain {X}]". */
export function parseSpecialSpendingUse(c: Cursor, after: "to" | "on"): ManaSpendingUse | null {
  return c.attempt((c): ManaSpendingUse | null => {
    if (after === "on") {
      if (!c.words("costs", "that", "contain")) return null;
      const symbol = parseContainedSymbol(c);
      return symbol && { use: "pay-cost-containing", symbol };
    }
    if (c.word("pay") !== null) {
      if (c.words("generic", "mana", "costs")) return { use: "pay-generic-costs" };
      if (c.words("a", "cost", "that", "contains")) {
        const symbol = parseContainedSymbol(c);
        return symbol && { use: "pay-cost-containing", symbol };
      }
      c.word("a");
      const keyword = c.words("cumulative", "upkeep") ? "cumulative upkeep" : c.word("disturb", "morph");
      if (!keyword || c.word("cost", "costs") === null) return null;
      return { use: "pay-keyword-cost", keyword: keyword as "cumulative upkeep" | "disturb" | "morph" };
    }
    if (c.word("turn") !== null) {
      c.word("a", "an");
      const what = parseFilter(c);
      return what && c.words("face", "up") ? { use: "turn-face-up", what } : null;
    }
    if (c.word("foretell") !== null)
      return c.words("a", "card", "from", "your", "hand") || c.word("cards") !== null ? { use: "foretell" } : null;
    if (c.word("unlock") !== null) return c.words("a", "door") || c.word("doors") !== null ? { use: "unlock-door" } : null;
    if (c.word("activate") !== null) {
      c.word("an");
      const abilityKeyword = c.word("equip", "power-up") as "equip" | "power-up" | null;
      return abilityKeyword && c.word("ability", "abilities") !== null && !c.isWord("of") ? { use: "activate", abilityKeyword } : null;
    }
    return null;
  });
}
