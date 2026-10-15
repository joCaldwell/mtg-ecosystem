// "For each player, choose friend or foe. Each friend …. Each foe …."
// (Battlebond). The controller labels every player; the later sentences
// address the players by their label. No player is targeted (CR 115.1), and
// the labels are chosen as the instruction resolves (CR 608.2d).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, PlayerRef, Sentence } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

export function parseChooseFriendOrFoe(c: Cursor): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    if (!c.words("for", "each", "player") || !c.punct(",")) return null;
    if (!c.words("choose", "friend", "or", "foe")) return null;
    return [{ effect: "choose-friend-or-foe", who: { player: "you" }, players: { player: "each-player" } }];
  });
}

/** "each friend" / "each foe". */
export function parseLabeledPlayers(c: Cursor): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    if (c.word("each") === null) return null;
    const label = c.word("friend", "foe");
    return label ? { player: "filtered", base: { player: "each-player" }, filter: { friendOrFoe: label as "friend" | "foe" } } : null;
  });
}

/** A label is only meaningful after this ability's own choice of labels. */
export function labelsChosenFirst(sentences: Sentence[]): boolean {
  let chosen = false;
  for (const sentence of sentences) {
    const text = JSON.stringify(sentence);
    if (text.includes('"friendOrFoe"') && !chosen) return false;
    if (text.includes('"choose-friend-or-foe"')) chosen = true;
  }
  return true;
}
