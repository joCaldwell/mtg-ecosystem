// CR 601.2b: a static ability can add an optional additional cost to spells
// as they are cast (Defiler of Vigor: "As an additional cost to cast green
// permanent spells, you may pay 2 life."), and CR 601.2f lets a companion
// sentence reduce the total cost if that cost was paid. CR 118.7b: a reduction
// by colored mana turns into generic mana when the cost lacks that color,
// unless the text restricts it to that color.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Color, CostModification, StaticEffect } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseCostList } from "./costs.ts";
import { parseCastScope } from "./cost-modifiers.ts";

const COLOR_OF_SYMBOL: Record<string, Color> = { W: "white", U: "blue", B: "black", R: "red", G: "green" };

export function parseSpellAdditionalCostGrant(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("as", "an", "additional", "cost", "to", "cast")) return null;
    const start = c.pos;
    while (!c.done() && !c.isPunct(",")) c.pos++;
    if (c.done()) return null;
    const subject = c.tokens.slice(start, c.pos);
    const parsed = parseCastScope(subject);
    // The caster pays: "you may pay" names the caster, so the caster is the player.
    if (!parsed || parsed.caster || parsed.spells.ref === "this" || parsed.spells.ref === "target" || parsed.occurrence) return null;
    const scope = { ...parsed, caster: { player: "you" as const } };
    if (!c.punct(",") || !c.words("you", "may")) return null;
    const costs = parseCostList(c);
    // Only a life payment is named by "paid life this way".
    if (!costs || costs.length !== 1 || costs[0].cost !== "pay-life" || !c.punct(".")) return null;
    const grant: StaticEffect = { static: "spell-additional-cost", scope, costs, optional: true };
    if (c.done()) return grant;
    if (!c.words("those", "spells", "cost")) return null;
    const symbol = c.peek();
    if (symbol?.kind !== "symbol" || !(symbol.value in COLOR_OF_SYMBOL)) return null;
    c.pos++;
    if (!c.words("less", "to", "cast", "if", "you", "paid", "life", "this", "way") || !c.punct(".")) return null;
    const modification: CostModification = { change: { direction: "reduce", symbols: [symbol.value] } };
    if (!c.words("this", "effect", "reduces", "only", "the", "amount", "of")) return null;
    const color = c.word("white", "blue", "black", "red", "green");
    if (!color || color !== COLOR_OF_SYMBOL[symbol.value] || !c.words("mana", "you", "pay")) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "group", effects: [grant, { static: "conditional", condition: { condition: "additional-cost-paid", cost: costs },
      effect: { static: "cost-modification", scope, ...modification, onlyColor: color as Color } }] };
  });
}
