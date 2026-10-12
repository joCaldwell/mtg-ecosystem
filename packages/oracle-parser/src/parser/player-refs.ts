// Player references whose identity depends on an earlier antecedent.
// CR 109.5, 608.2c: the words are read against what was established earlier
// in the same spell or ability; nothing here selects a new player.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectRef, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

type PossessiveObject = (c: Cursor) => ObjectRef | null;

const singularThat = (of: ObjectRef | null): of is Extract<ObjectRef, { ref: "that" }> =>
  of?.ref === "that" && !of.plural;

/** Called after "that player". "…or that planeswalker's controller" names the
 * one player implied by a single player-or-object antecedent (Blightning);
 * it is not a second alternative participant. */
export function parseOrThatObjectsController(c: Cursor, possessiveObject: PossessiveObject): PlayerRef | null {
  return c.attempt(c => {
    if (c.word("or") === null || !c.isWord("that")) return null;
    const of = possessiveObject(c);
    return singularThat(of) && c.word("controller") !== null ? { player: "that-player-or-controller", of } : null;
  });
}

/** Called after "that permanent's controller": the reversed printing
 * "…or that player" (Acorn Catapult) has the same meaning. */
export function parseOrThatPlayer(c: Cursor, of: ObjectRef): PlayerRef | null {
  return c.attempt(c => singularThat(of) && c.words("or", "that", "player")
    ? { player: "that-player-or-controller", of } : null);
}
