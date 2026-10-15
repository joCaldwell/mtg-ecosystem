// CR 119.5: an effect that sets a life total makes the player gain or lose
// the necessary amount of life; it is not a continuous effect.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Effect, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

export interface LifeTotalGrammar {
  parsePossessivePlayer(c: Cursor): PlayerRef | null;
  parseAmount(c: Cursor): Amount | null;
}

/** "the highest/lowest life total among all players": one number shared by
 * every affected player, read before any life total changes. */
function parseLifeExtremum(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word("the") === null) return null;
    const adjective = c.word("highest", "lowest");
    if (!adjective || !c.words("life", "total", "among")) return null;
    const among: PlayerRef | null = c.words("all", "players") ? { player: "each-player" } : null;
    return among && { amount: "life-total-extremum", extremum: adjective === "highest" ? "maximum" : "minimum", among };
  });
}

/** "[player]'s life total becomes N" and the causative "you may have your
 * life total become N" (the caller has consumed "you may have"). */
export function parseSetLifeTotal(c: Cursor, g: LifeTotalGrammar): Extract<Effect, { effect: "set-life-total" }> | null {
  return c.attempt(c => {
    const previous = c.tokens[c.pos - 1];
    const causative = previous?.kind === "word" && !previous.possessive && previous.value === "have";
    const who = g.parsePossessivePlayer(c);
    if (!who || !c.words("life", "total") || c.word(causative ? "become" : "becomes") === null) return null;
    c.words("equal", "to");
    const amount = parseLifeExtremum(c) ?? g.parseAmount(c);
    return amount && { effect: "set-life-total", who, amount };
  });
}
