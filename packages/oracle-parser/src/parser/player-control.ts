// Controlling another player (CR 723): "You control target player during
// that player's next turn." Only control of the player changes (723.3); the
// controller makes that player's choices and decisions for the turn (723.5).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parsePlayerRef, parsePossessivePlayer } from "./refs.ts";

export interface PlayerControlGrammar {
  actor(c: Cursor, imperative: string, conjugated: string): PlayerRef | null;
}

/** The turn is the controlled player's own next turn, printed with an
 * anaphor back to that player ("that player's", "their"); any other turn
 * owner is not the CR 723.1 effect. */
export function parsePlayerControl(c: Cursor, g: PlayerControlGrammar): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const who = g.actor(c, "control", "controls");
    if (!who) return null;
    const controlled = parsePlayerRef(c);
    if (!controlled || controlled.player === "you" || c.word("during") === null) return null;
    const whose = parsePossessivePlayer(c);
    if (!whose || (whose.player !== "that-player" && whose.player !== "they") || !c.words("next", "turn")) return null;
    return [{ effect: "control-player", who, controlled, during: { turn: "next", whose } }];
  });
}
