import type { PlayerRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";

/** Explicit plural actors belong to an instruction, not an eligibility
 * query or new collection of target slots. Keep occurrences even when two
 * roles refer to the same player (Angel of Destiny's redirected-damage
 * ruling). One group is one action, with rule-defined simultaneity/order.
 * CR101.4,121.2c/d,608.2e/f:
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
 * https://magic.wizards.com/en/news/feature/zendikar-rising-release-notes-2020-09-10 */
export function parsePlayerGroupActor(c: Cursor, player: (c: Cursor) => PlayerRef | null): PlayerRef | null {
  return c.attempt(c => {
    if (!c.words("you", "and")) return null;
    const second = player(c);
    // Other groups introduce independent choices, quantifiers, reciprocal
    // roles or optional per-player decisions; those need distinct scopes.
    if (!second || !["target-opponent", "that-player"].includes(second.player) || c.word("each") === null) return null;
    return { player: "group", members: [{ player: "you" }, second], multiplicity: "per-member-occurrence" };
  });
}
