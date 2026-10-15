// Counts of what players did earlier in the turn, as the unit after "for
// each": "card you've drawn this turn", "Food you've sacrificed this turn",
// "spell your opponents have cast this turn". They count completed actions
// (CR 608.2i: as each happened), never the current game state, and mirror
// the player-action-history / cast-count conditions.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, ObjectFilter, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseFilter, parsePlayerRef } from "./refs.ts";

type Actor = PlayerRef | "all-players-combined";

/** "you've" / "<players> have|has"; no actor is the passive "by anyone". */
function parseActor(c: Cursor): Actor {
  return c.attempt((c): Actor | null => {
    if (c.word("you've") !== null) return { player: "you" };
    if (c.word("they've") !== null) return { player: "they" };
    const who: PlayerRef | null = c.words("your", "opponents") ? { player: "each-opponent" } : parsePlayerRef(c);
    if (!who || who.player === "target-player" || who.player === "target-opponent") return null;
    return c.word("have", "has") !== null ? who : null;
  }) ?? "all-players-combined";
}

export function parseHistoryUnit(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    const what = parseFilter(c);
    if (!what || what.zone || what.control || what.controlledBy) return null;
    // CR 111.1, 701.7a: "tokens you created this turn" counts the tokens that
    // player created, wherever they are now.
    const created = c.attempt((c): Amount | null => {
      const who = parsePlayerRef(c);
      if (!who || who.player === "target-player" || who.player === "target-opponent" || !c.words("created", "this", "turn")) return null;
      return what.cls === "token" ? { amount: "player-action-count", who, action: { action: "create-token", what }, during: "this-turn" } : null;
    });
    if (created) return created;
    const who = parseActor(c);
    // CR 702.29a: cycling discards the card as a cost, so every card cycled
    // is a card discarded and "cycled or discarded" is the discarded set.
    const verb = c.words("cycled", "or", "discarded") ? "discarded" : c.word("drawn", "sacrificed", "discarded", "cast");
    if (!verb || !c.words("this", "turn")) return null;
    const during = "this-turn" as const;
    if (verb === "cast") return what.cls === "spell" ? { amount: "cast-count", who, what, during } : null;
    if (verb === "drawn") {
      // Only cards are drawn; a narrower description would be a different count.
      return Object.keys(what).length === 1 && what.cls === "card" ? { amount: "player-action-count", who, action: { action: "draw" }, during } : null;
    }
    if (what.cls === "spell") return null;
    const filter: ObjectFilter = what;
    return { amount: "player-action-count", who, action: { action: verb === "sacrificed" ? "sacrifice" : "discard", what: filter }, during };
  });
}

/** "time you've cast your commander from the command zone this game" (after
 * "each"): the casts themselves, so paid commander tax is not what is counted.
 * CR 903.8 counts these casts for the commander tax; "a commander" is any
 * commander designated card, "your commander" one designated for you. */
export function parseCommanderCastUnit(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    if (!c.words("time", "you've", "cast")) return null;
    const own = c.words("your", "commander");
    if (!own && !c.words("a", "commander")) return null;
    if (!c.words("from", "the", "command", "zone", "this", "game")) return null;
    return { amount: "cast-count", who: { player: "you" }, what: { cls: "spell", commander: true, ...(own ? { own: "you" as const } : {}) },
      during: "this-game", from: { zone: "command" } };
  });
}

/** "[time|times] it's been cast from the command zone this game": the casts of
 * that one commander (CR 903.8), never all of a player's commanders summed.
 * `what` is what "it" denotes where the caller stands. */
export function parseTimesCastUnit(c: Cursor, plural: boolean, what: Extract<Amount, { amount: "times-cast" }>["what"]): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word(plural ? "times" : "time") === null || !c.words("it's", "been", "cast", "from", "the", "command", "zone", "this", "game")) return null;
    return { amount: "times-cast", what, from: { zone: "command" }, during: "this-game" };
  });
}

/** "{E} you've paid or lost this turn" after "for each" (CR 107.14): one unit
 * per energy counter paid or lost, not per payment or per effect. */
export function parseEnergySpentUnit(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    const symbol = c.peek();
    if (symbol?.kind !== "symbol" || symbol.value !== "E") return null;
    c.pos++;
    return c.words("you've", "paid", "or", "lost", "this", "turn") ? { amount: "energy-paid-or-lost", who: { player: "you" }, during: "this-turn" } : null;
  });
}
