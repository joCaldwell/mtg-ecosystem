// Library placements whose library is stated by the surrounding instruction
// instead of the destination phrase itself. The library is recovered from the
// printed antecedent (the shuffle, the look, or the preceding destination of
// the same put instruction); it is never guessed from an unrelated clause.
// CR 401.4 (ordering cards put into one library position), 701.20e (looked-at
// cards stay in their library), 701.24b/g (found cards are placed after the
// shuffle and are not randomized by it).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, PlayerRef, ZoneRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseAmount, parseObjectRef, parseZoneRef } from "./refs.ts";

type ZoneMove = Extract<Effect, { effect: "move-zone" }>;

const isWordAt = (c: Cursor, index: number, ...values: string[]): boolean => {
  const token = c.tokens[index];
  return token?.kind === "word" && !token.possessive && values.includes(token.value);
};
const isPunctAt = (c: Cursor, index: number, value: string): boolean => {
  const token = c.tokens[index];
  return token?.kind === "punct" && token.value === value;
};

/** Only a reference to already identified cards can be put "back" or on an
 * unnamed library; a fresh selection would need its own source zone. */
function parsePlacedCards(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    const what = parseObjectRef(c);
    return what && (what.ref === "it" || what.ref === "them" || (what.ref === "that" && what.filter.cls === "card" && Object.keys(what.filter).length === 1))
      ? what : null;
  });
}

function parseOrder(c: Cursor): "any" | "random" | null {
  return c.attempt(c => {
    if (c.word("in") === null) return null;
    if (c.words("any", "order")) return "any" as const;
    if (c.words("a", "random", "order")) return "random" as const;
    return null;
  });
}

/** The library named by "look(s) at the top N cards of <library>" when that
 * instruction ends immediately before ", then put(s)" at verbAt. */
function lookedLibraryBefore(c: Cursor, verbAt: number): ZoneRef | null {
  if (!isWordAt(c, verbAt - 1, "then") || !isPunctAt(c, verbAt - 2, ",")) return null;
  for (let start = verbAt - 3; start >= 1; start--) {
    if (isPunctAt(c, start, ".")) return null;
    if (!isWordAt(c, start, "at") || !isWordAt(c, start - 1, "look", "looks")) continue;
    const look = new Cursor(c.tokens.slice(start + 1, verbAt - 2));
    if (!look.words("the", "top")) return null;
    const count = parseAmount(look);
    if (!count || look.word("cards") === null || look.word("of") === null) return null;
    const library = parseZoneRef(look);
    return library && look.done() && library.zone === "library" && library.owner && !library.position ? library : null;
  }
  return null;
}

/** Called after the put verb (at verbAt) and its actor have been consumed. */
export function parseContextualLibraryPlacement(c: Cursor, who: PlayerRef, verbAt: number): ZoneMove[] | null {
  return c.attempt((c): ZoneMove[] | null => {
    const what = parsePlacedCards(c);
    if (!what) return null;
    // "…, then put them back in any order": the looked-at cards never left
    // the top of that library; only their order there is chosen (701.20e).
    if (c.word("back") !== null) {
      const looked = what.ref === "them" ? lookedLibraryBefore(c, verbAt) : null;
      if (!looked || !c.words("in", "any", "order")) return null;
      return [{ effect: "move-zone", what, to: { ...looked, position: "top" }, order: "any" }];
    }
    // "…shuffle(s) and put(s) that card on top": the shuffled library is the
    // acting player's own, exactly as the bare shuffle states it (701.24b).
    if (!isWordAt(c, verbAt - 1, "and") || !isWordAt(c, verbAt - 2, "shuffle", "shuffles")) return null;
    if (!c.words("on", "top") || c.isWord("of")) return null;
    const order = parseOrder(c);
    if (order === "random") return null;
    return [{ effect: "move-zone", what, to: { zone: "library", owner: who, position: "top" }, ...(order ? { order } : {}) }];
  });
}

/** "… on top of your library and the rest on the bottom in any order": a
 * later destination of the same put instruction names only the position. */
export function parseSameLibraryPlacement(c: Cursor, previous: ZoneRef): { to: ZoneRef; order?: "any" | "random" } | null {
  return c.attempt(c => {
    if (previous.zone !== "library" || !previous.position || !previous.owner) return null;
    const position = c.words("on", "top") ? "top" as const : c.words("on", "the", "bottom") ? "bottom" as const : null;
    if (!position || position === previous.position || c.isWord("of")) return null;
    const order = parseOrder(c);
    return { to: { ...previous, position }, ...(order ? { order } : {}) };
  });
}

const ORDINALS = ["second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth"];

/** "into its owner's library third from the top" (CR 401.7): called right
 * after a whole-library destination; the depth is part of that destination. */
export function parseLibraryDepth(c: Cursor, library: ZoneRef): ZoneRef | null {
  return c.attempt(c => {
    if (library.zone !== "library" || library.position || !library.owner) return null;
    const ordinal = c.word(...ORDINALS);
    if (!ordinal || !c.words("from", "the", "top")) return null;
    return { ...library, position: "top", depth: ORDINALS.indexOf(ordinal) + 2 };
  });
}

/** "put one <there> and the other <elsewhere>": elliptical "one" selects one
 * of the cards just found or looked at, exactly like "one of them"; "the
 * other" is whatever remains of that pool and exists only after such a pick. */
export function parseSplitPairMember(c: Cursor, earlier: ZoneMove[]): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (!earlier.length) {
      if (c.word("one") === null || !c.isWord("onto", "into", "on")) return null;
      return { ref: "from-pool", pool: { ref: "them" }, count: { amount: "fixed", value: 1 } };
    }
    const first = earlier[0].what;
    if (earlier.length !== 1 || first.ref !== "from-pool" || first.upTo || first.count.amount !== "fixed" || first.count.value !== 1) return null;
    return c.words("the", "other") && c.isWord("onto", "into", "on") ? { ref: "rest" } : null;
  });
}
