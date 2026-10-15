// CR 701.38c–d: "voting" refers only to an actual vote; a player with
// several votes casts them all at the time they would otherwise vote.
// "While voting, you may vote an additional time" (optional) and "While
// voting, you get an additional vote" are rules of the game for that player.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { StaticEffect } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parsePlayerRef } from "./refs.ts";

export function parseVoteRule(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("while", "voting") || !c.punct(",")) return null;
    const who = parsePlayerRef(c);
    if (!who || who.player !== "you") return null;
    let optional: boolean;
    if (c.words("may", "vote", "an", "additional", "time")) optional = true;
    else if (c.words("get", "an", "additional", "vote")) optional = false;
    else return null;
    if (!c.punct(".") || !c.done()) return null;
    return { static: "player-rule", rule: { who, rule: "additional-votes", additional: 1, optional } };
  });
}
