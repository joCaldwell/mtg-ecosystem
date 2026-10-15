// Players described by a superlative: "a player with the most life",
// "each player who controls the most lands", "defending player controls the
// most creatures or is tied for the most". The candidate's own measurement
// equals the greatest such measurement among the compared players, so every
// player tied for it qualifies, as with "the creature with the greatest
// power" (attribute-extremum); the printed tie clause adds nothing (CR 107.1,
// 119.1).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Comparison, ObjectFilter, PlayerFilter, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

export interface PlayerMostGrammar {
  parseFilter(c: Cursor): ObjectFilter | null;
}

type Among = Extract<PlayerRef, { player: "each-player" | "each-opponent" }>;

/** The compared players: printed "among your opponents" / "among all
 * players", else (only when `implicit` is known) every player. */
function parseAmong(c: Cursor, implicit: Among | null): Among | null {
  if (c.words("among", "your", "opponents")) return { player: "each-opponent" };
  if (c.words("among", "all", "players")) return { player: "each-player" };
  return implicit;
}

function hasScope(filter: ObjectFilter): boolean {
  return !!(filter.control || filter.controlledBy || filter.own || filter.zone || filter.relation ||
    filter.allOf?.some(hasScope) || filter.anyOf?.some(hasScope));
}

/** "the most life [among …] [or tied for most life]" after "with"/"has".
 * A bare "with the most life" over every player ("each player with the most
 * life") is left unsupported: only the printed tie clause or an indefinite
 * choice among stated players ("an opponent with the most life among your
 * opponents") is accepted. */
function parseMostLife(c: Cursor, implicit: Among | null): Comparison | null {
  return c.attempt((c): Comparison | null => {
    if (!c.words("the", "most", "life")) return null;
    const explicit = parseAmong(c, null);
    const among = explicit ?? implicit;
    if (!among) return null;
    // Tie clause: "or tied for most life" (qualifier), "or is tied for the most life" (predicate).
    const tied = c.attempt(c => {
      if (c.word("or") === null) return null;
      c.word("is", "are");
      if (!c.words("tied", "for")) return null;
      c.word("the");
      return c.words("most", "life") || null;
    });
    if (!tied && !explicit) return null;
    return { op: "eq", value: { amount: "life-total-extremum", extremum: "maximum", among } };
  });
}

/** "the most <objects> [among …] [or is tied for the most]" after "controls". */
function parseMostControlled(c: Cursor, g: PlayerMostGrammar, implicit: Among | null): PlayerFilter["controls"] | null {
  return c.attempt((c): PlayerFilter["controls"] | null => {
    if (!c.words("the", "most")) return null;
    const filter = g.parseFilter(c);
    // Each candidate's own objects are counted; a printed scope would
    // contradict that.
    if (!filter || hasScope(filter)) return null;
    const among = parseAmong(c, implicit);
    if (!among) return null;
    c.attempt(c => c.words("or", "is", "tied", "for", "the", "most") || c.words("or", "are", "tied", "for", "the", "most") || null);
    return { filter, comparison: { op: "eq", value: { amount: "per-player-extremum", extremum: "maximum", among,
      count: { count: "controlled", of: structuredClone(filter) } } } };
  });
}

/** Players compared without a printed "among" range over every player only
 * when the described player is drawn from all players. */
function implicitAmong(base: PlayerRef | undefined): Among | null {
  return base?.player === "a-player" || base?.player === "each-player" ? { player: "each-player" } : null;
}

/** Qualifier after a player noun: "with the most life …", "who controls the
 * most lands …". */
export function parseMostPlayerQualifier(c: Cursor, g: PlayerMostGrammar, base: PlayerRef | undefined): PlayerFilter | null {
  return c.attempt((c): PlayerFilter | null => {
    const implicit = implicitAmong(base);
    if (c.word("with") !== null) {
      // An indefinite player is chosen among the stated players.
      if (base?.player !== "a-player" && base?.player !== "an-opponent") return null;
      const lifeTotal = parseMostLife(c, implicit);
      return lifeTotal ? { lifeTotal } : null;
    }
    if (c.word("who") === null || c.word("controls", "control") === null) return null;
    const controls = parseMostControlled(c, g, implicit);
    return controls ? { controls } : null;
  });
}

/** Predicate of a condition whose subject is one identified player:
 * "<player> controls the most creatures or is tied for the most", compared
 * with every player unless "among" says otherwise. */
export function parseMostPredicate(c: Cursor, g: PlayerMostGrammar):
  { measure: "controls"; filter: ObjectFilter; comparison: Comparison } | { measure: "life"; comparison: Comparison } | null {
  return c.attempt(c => {
    const verb = c.word("controls", "control", "has", "have");
    if (!verb) return null;
    if (verb.startsWith("control")) {
      const controls = parseMostControlled(c, g, { player: "each-player" });
      return controls ? { measure: "controls" as const, ...controls } : null;
    }
    const comparison = parseMostLife(c, { player: "each-player" });
    return comparison ? { measure: "life" as const, comparison } : null;
  });
}

/** "the player with the most life", "the player who controls the most
 * creatures": a definite description over every player, denoting the one
 * player whose measurement exceeds every other player's. A tie clause would
 * admit several players and is not this description. */
export function parseDefiniteMostPlayer(c: Cursor, g: PlayerMostGrammar): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    if (!c.words("the", "player")) return null;
    const start = c.pos;
    const each: Among = { player: "each-player" };
    let filter: PlayerFilter | null = null;
    if (c.words("with", "the", "most", "life")) {
      filter = { lifeTotal: { op: "eq", value: { amount: "life-total-extremum", extremum: "maximum", among: each } } };
    } else if (c.words("who", "controls")) {
      const controls = parseMostControlled(c, g, each);
      filter = controls ? { controls } : null;
    }
    if (!filter || c.isWord("or") || c.tokens.slice(start, c.pos).some(t => t.kind === "word" && t.value === "tied")) return null;
    return { player: "definite", filter };
  });
}
