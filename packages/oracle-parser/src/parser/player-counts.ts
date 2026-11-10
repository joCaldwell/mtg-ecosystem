// Counts of players rather than objects. CR 102.2–102.3: a player's
// opponents are the other players still in the game who aren't teammates.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, PlayerFilter, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

export interface PlayerCountGrammar {
  parsePlayerRef(c: Cursor): PlayerRef | null;
  isPluralPlayerRef(ref: PlayerRef): boolean;
  /** The "who ..." / "with ..." eligibility clauses of a player reference. */
  parsePlayerQualifiers?(c: Cursor): PlayerFilter | null;
}

/** "<player> has/have": one player's own opponents; a quantified player
 * would need a per-player or summed reading that nothing prints. */
function parseHolder(c: Cursor, g: PlayerCountGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    const who = g.parsePlayerRef(c);
    if (!who || g.isPluralPlayerRef(who) || c.word("have", "has") === null) return null;
    return { amount: "opponent-count", who };
  });
}

/** "the number of opponents / players who ...": the players satisfying the
 * printed eligibility are counted. A bare plural has no qualifier to count
 * by here ("opponents you have" is parseHolder's own form). */
function parseQualifiedPlayers(c: Cursor, g: PlayerCountGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    // "your opponents" is the same set as "opponents" here (CR 102.2).
    const yours = c.word("your") !== null;
    const noun = c.word("opponents", "players");
    if (yours && noun !== "opponents") return null;
    const filter = noun && g.parsePlayerQualifiers?.(c);
    if (!noun || !filter) return null;
    return { amount: "player-count", of: { player: "filtered", base: { player: noun === "opponents" ? "each-opponent" : "each-player" }, filter } };
  });
}

/** Amount position: "the number of opponents you have". */
export function parseOpponentCountAmount(c: Cursor, g: PlayerCountGrammar): Amount | null {
  return c.attempt(c => c.words("the", "number", "of", "opponents") ? parseHolder(c, g) : null)
    ?? c.attempt(c => c.words("the", "number", "of") ? parseQualifiedPlayers(c, g) : null)
    // Every player still in the game (CR 102.1, 104.3a); none who has left.
    ?? c.attempt((c): Amount | null => c.words("the", "number", "of", "players", "in", "the", "game")
      ? { amount: "player-count", of: { player: "each-player" } } : null);
}

/** Unit after "for each": "opponent you have". It multiplies one
 * instruction's quantity; the instruction is not repeated per opponent. */
export function parseOpponentUnit(c: Cursor, g: PlayerCountGrammar): Amount | null {
  return c.attempt(c => c.word("opponent") !== null ? parseHolder(c, g) : null) ?? c.attempt((c): Amount | null => {
    // "You gain 1 life for each player.": every player in the game counts once.
    if (c.word("player") === null || !c.isPunct(".")) return null;
    return { amount: "player-count", of: { player: "each-player" } };
  }) ?? c.attempt((c): Amount | null => {
    // "for each opponent who lost life this turn": "each opponent who ..." is
    // the ordinary quantified player reference, read from its own "each".
    const each = c.tokens[c.pos - 1];
    if (each?.kind !== "word" || each.value !== "each" || each.possessive) return null;
    c.pos--;
    const of = g.parsePlayerRef(c);
    return of?.player === "filtered" && g.isPluralPlayerRef(of) && (of.base.player === "each-opponent" || of.base.player === "each-player")
      ? { amount: "player-count", of } : null;
  });
}

/** "players being attacked", "opponents you attacked this combat": the
 * number of players (not creatures) in that attacked state. Singular nouns
 * are the unit after "for each"; plural nouns follow "the number of". */
function parseAttackedPlayers(c: Cursor, plural: boolean): Amount | null {
  return c.attempt((c): Amount | null => {
    const noun = c.word(plural ? "players" : "player", plural ? "opponents" : "opponent");
    if (!noun) return null;
    const players = noun.startsWith("opponent") ? "opponents" as const : "players" as const;
    if (c.words("being", "attacked")) return { amount: "attacked-player-count", players, which: { status: "being-attacked" } };
    // "opponents you're attacking": being attacked by that player right now.
    if (c.words("you're", "attacking")) return { amount: "attacked-player-count", players, which: { status: "being-attacked", by: { player: "you" } } };
    if (!c.words("you", "attacked")) return null;
    const during = c.words("this", "turn") ? "this-turn" as const : c.words("this", "combat") ? "this-combat" as const : null;
    return during && { amount: "attacked-player-count", players, which: { by: { player: "you" }, during } };
  });
}

export function parseAttackedPlayerCountAmount(c: Cursor): Amount | null {
  return c.attempt(c => c.words("the", "number", "of") ? parseAttackedPlayers(c, true) : null);
}

export function parseAttackedPlayerUnit(c: Cursor): Amount | null {
  return parseAttackedPlayers(c, false);
}
