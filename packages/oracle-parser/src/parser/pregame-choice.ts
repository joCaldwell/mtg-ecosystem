/**
 * "If ~ is your commander, choose a color before the game begins. ~ is the
 * chosen color." (The Prismatic Piper, Faceless One). CR 903.4b: the choice
 * is made before the game begins; CR 607.2p links it to the
 * characteristic-defining ability printed in the same paragraph.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
 */
import type { StaticEffect } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseValueChoice } from "./choices.ts";

export function parseCommanderPregameChoice(c: Cursor, parseStatic: (c: Cursor) => StaticEffect | null): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (c.word("if") === null || !c.selfref() || !c.words("is", "your", "commander") || !c.punct(",")) return null;
    if (c.word("choose") === null) return null;
    const choice = parseValueChoice(c);
    if (!choice || choice.category !== "color" || choice.count || choice.excluding || choice.options) return null;
    if (!c.words("before", "the", "game", "begins") || !c.punct(".")) return null;
    const linked = parseStatic(c);
    // Only the CDA that states this object's color as the chosen one is
    // linked to the pregame choice (CR 604.3, 607.2p).
    if (!linked || linked.static !== "cda-colors" || linked.what.ref !== "self"
      || JSON.stringify(linked.colors) !== JSON.stringify({ choice: "chosen-value", category: "color", which: "chosen" })) return null;
    return { static: "commander-pregame-choice", what: { ref: "self" }, who: { player: "you" }, choice, linked };
  });
}
