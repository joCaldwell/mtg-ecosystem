// CR 602.2b: an activated ability's activation cost is its analog of a
// spell's mana cost, so an effect can offer an alternative to it, chosen as
// the ability is activated (601.2b, 601.2f, 118.9). Additional costs,
// increases and reductions still apply to the alternative.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { CostModificationScope, StaticEffect } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseCostList } from "./costs.ts";
import { parseActivationScope } from "./cost-modifiers.ts";

type ActivationScope = Extract<CostModificationScope, { action: "activate" }>;
const KEYWORDS = ["equip", "cycling", "power-up"] as const;
type Keyword = typeof KEYWORDS[number];
const ORDINALS: Record<string, number> = { first: 1, second: 2, third: 3 };

/** CR 702.29c: to cycle a card is to activate its cycling ability, so "the
 * first card you cycle each turn" is the first such activation. */
function parseCycledCard(c: Cursor): ActivationScope | null {
  return c.attempt((c): ActivationScope | null => {
    if (c.word("the") === null) return null;
    const marker = c.anyWord();
    const ordinal = marker ? ORDINALS[marker.value] : undefined;
    if (!ordinal || !c.words("card", "you", "cycle", "each", "turn")) return null;
    return { action: "activate", activator: { player: "you" }, occurrence: { ordinal, period: "turn" }, keyword: "cycling",
      abilities: { ref: "each", filter: { cls: "ability" } } };
  });
}

/** "You may pay {0} rather than pay the equip cost of the first equip ability
 * you activate each turn" / "… rather than pay cycling costs". */
export function parseAlternativeActivationCost(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("you", "may")) return null;
    const costs = parseCostList(c);
    if (!costs || !c.words("rather", "than", "pay")) return null;
    let scope: ActivationScope | null = null;
    // "… you activate during each of your turns": the ordinal counts within
    // each turn, and only that player's own turns qualify.
    const turn: { yourTurn?: true } = {};
    const general = c.word(...KEYWORDS) as Keyword | null;
    if (general) {
      // Every such activation cost the player pays.
      if (c.word("costs") === null) return null;
      scope = { action: "activate", activator: { player: "you" }, keyword: general, abilities: { ref: "each", filter: { cls: "ability" } } };
    } else {
      if (c.word("the") === null) return null;
      const keyword = c.word(...KEYWORDS) as Keyword | null;
      if (!keyword || !c.words("cost", "of")) return null;
      const end = c.tokens.findIndex((token, index) => index >= c.pos && token.kind === "punct" && token.value === ".");
      const tokens = c.tokens.slice(c.pos, end < 0 ? c.tokens.length : end);
      const local = new Cursor(tokens);
      scope = keyword === "cycling" ? parseCycledCard(local) : null;
      if (scope && !local.done()) scope = null;
      scope ??= parseActivationScope(tokens, turn);
      // The named cost must be the cost of the abilities described.
      if (!scope || scope.keyword !== keyword) return null;
      c.pos += tokens.length;
    }
    c.punct(".");
    if (!c.done()) return c.fail("complete alternative activation cost");
    const effect: StaticEffect = { static: "alternative-activation-cost", who: { player: "you" }, scope, costs };
    return turn.yourTurn ? { static: "conditional", condition: { condition: "your-turn" }, effect } : effect;
  });
}
