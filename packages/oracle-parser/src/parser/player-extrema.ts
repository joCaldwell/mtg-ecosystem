// Extremes of a per-player quantity: "the greatest number of cards a player
// discarded this way", "the highest life total among your opponents". The
// result is one number; ties are irrelevant and quantities of different
// players are never summed (CR 107.1, 119.1, 608.2h).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, ObjectFilter, PerPlayerCount, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

export interface PlayerExtremumGrammar {
  parseFilter(c: Cursor): ObjectFilter | null;
}

type Among = Extract<PlayerRef, { player: "each-player" | "each-opponent" }>;

/** The indefinite player of the measured clause ranges over all candidates. */
function candidates(ref: PlayerRef | undefined): Among | null {
  return ref?.player === "a-player" ? { player: "each-player" } : ref?.player === "an-opponent" ? { player: "each-opponent" } : null;
}

/** "the highest/lowest life total among all players / players / your
 * opponents" and "the life total of an opponent with the most life". */
export function parseLifeTotalExtremum(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word("the") === null) return null;
    if (c.words("life", "total", "of", "an", "opponent", "with", "the", "most", "life"))
      return { amount: "life-total-extremum", extremum: "maximum", among: { player: "each-opponent" } };
    const adjective = c.word("highest", "lowest");
    if (!adjective || !c.words("life", "total", "among")) return null;
    const among: Among | null = c.words("all", "players") || c.word("players") !== null ? { player: "each-player" }
      : c.words("your", "opponents") ? { player: "each-opponent" } : null;
    return among && { amount: "life-total-extremum", extremum: adjective === "highest" ? "maximum" : "minimum", among };
  });
}

/** "the greatest number of <objects> a player controls / discarded this way"
 * and "... of cards an opponent has drawn this turn". The filter grammar
 * reads the indefinite player as the objects' controller or actor; here that
 * player is each candidate in turn, so it moves to `among`. */
function parseGreatestNumber(c: Cursor, g: PlayerExtremumGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    if (!c.words("the", "greatest", "number", "of")) return null;
    const filter = g.parseFilter(c);
    if (!filter) return null;
    let among: Among | null = null;
    let count: PerPlayerCount | null = null;
    const { controlledBy, relation, ...of } = filter;
    if (controlledBy && !relation) {
      among = candidates(controlledBy);
      count = { count: "controlled", of };
    } else if (relation && !controlledBy) {
      if (relation.relation !== "event-result" || relation.event !== "discard" || relation.scope !== "this-way" || relation.period) return null;
      among = candidates(relation.who);
      count = { count: "event-result", event: "discard", scope: "this-way", of };
    } else if (!controlledBy && !relation && Object.keys(of).length === 1 && of.cls === "card") {
      // CR 121.1: cards the candidate has drawn, wherever they are now.
      among = c.words("a", "player") ? { player: "each-player" } : c.words("an", "opponent") ? { player: "each-opponent" } : null;
      if (!c.words("has", "drawn", "this", "turn")) return null;
      count = { count: "cards-drawn", during: "this-turn" };
    }
    return among && count ? { amount: "per-player-extremum", extremum: "maximum", among, count } : null;
  });
}

export function parsePlayerExtremum(c: Cursor, g: PlayerExtremumGrammar): Amount | null {
  return parseLifeTotalExtremum(c) ?? parseGreatestNumber(c, g);
}
