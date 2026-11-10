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

/** "the owner of target creature", "the controller of that spell": the same
 * player as "target creature's owner" (CR 108.3, 108.4). Only one object can
 * be meant; "the owner of each card ..." distributes over several objects and
 * names a different player for each, which this node cannot say. */
export function parseRelationalPlayer(c: Cursor, parseObject: (c: Cursor) => ObjectRef | null): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    if (c.word("the") === null) return null;
    const relation = c.word("owner", "controller") as "owner" | "controller" | null;
    if (!relation || c.word("of") === null) return null;
    const of = parseObject(c);
    if (!of || !singleObject(of)) return null;
    return { player: relation, of };
  });
}

function singleObject(of: ObjectRef): boolean {
  switch (of.ref) {
    case "it": case "self": case "this": case "equipped": case "enchanted": return true;
    case "that": return !of.plural;
    case "related": case "choice-result": return of.number === "singular";
    case "target": return of.count.amount === "fixed" && of.count.value === 1;
    default: return false;
  }
}

/** "each other player", "each other opponent": the quantified players with
 * the unresolved contrast of PlayerFilter.other. */
export function parseEachOtherPlayer(c: Cursor): { base: PlayerRef; filter: { other: true } } | null {
  return c.attempt(c => {
    if (!c.words("each", "other")) return null;
    const noun = c.word("player", "opponent");
    if (!noun) return null;
    return { base: { player: noun === "player" ? "each-player" : "each-opponent" }, filter: { other: true as const } };
  });
}
