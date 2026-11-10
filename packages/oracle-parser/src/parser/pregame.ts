// Abilities that function before the first turn: opening-hand actions
// (CR 103.6) and deck-construction rules (CR 100.2a, 113.6n). They are
// neither resolving instructions nor battlefield statics, so each complete
// sentence is matched as a whole and anything else stays unparsed.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectRef, Sentence, StaticEffect } from "../ast.ts";
import { wordNumber } from "../vocab.ts";
import type { Cursor } from "./cursor.ts";
import { parseObjectRef } from "./refs.ts";

type SourceRef = Extract<ObjectRef, { ref: "self" | "this" }>;

function parseSource(c: Cursor): SourceRef | null {
  return c.attempt(c => {
    const what = parseObjectRef(c);
    return what && (what.ref === "self" || (what.ref === "this" && what.noun === "card")) ? what : null;
  });
}

/** CR 103.6a: "If this card is in your opening hand, you may begin the game
 * with it on the battlefield." The pronoun or repeated name is this card. */
function parseBeginOnBattlefield(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (c.word("if") === null) return null;
    const what = parseSource(c);
    if (!what || !c.words("is", "in", "your", "opening", "hand") || !c.punct(",")) return null;
    if (!c.words("you", "may", "begin", "the", "game", "with")) return null;
    if (c.word("it") === null && !c.selfref()) return null;
    if (!c.words("on", "the", "battlefield")) return null;
    c.punct(".");
    return c.done() ? { static: "opening-hand-action", what, who: { player: "you" }, action: "begin-on-battlefield" } : null;
  });
}

/** CR 103.6b: "You may reveal this card from your opening hand. If you do,
 * <instructions>." The instructions run only when the card was revealed. */
function parseRevealFromOpeningHand(c: Cursor, parseBody: (c: Cursor) => Sentence[] | null): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("you", "may", "reveal")) return null;
    const what = parseSource(c);
    if (!what || !c.words("from", "your", "opening", "hand") || !c.punct(".")) return null;
    if (!c.words("if", "you", "do") || !c.punct(",")) return null;
    c.pregame = true;
    let effects: Sentence[] | null;
    try { effects = parseBody(c); } finally { c.pregame = false; }
    return effects && c.done() ? { static: "opening-hand-action", what, who: { player: "you" }, action: "reveal", effects } : null;
  });
}

/** CR 100.2a / 113.6n: a named exception to the four-copy limit. */
function parseDeckCopyLimit(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("a", "deck", "can", "have")) return null;
    let maximum: number | undefined;
    if (!c.words("any", "number", "of")) {
      if (!c.words("up", "to")) return null;
      const token = c.next();
      const value = token?.kind === "number" ? token.value : token?.kind === "word" && !token.possessive ? wordNumber(token.value) : undefined;
      if (value === undefined || value < 1) return null;
      maximum = value;
    }
    if (!c.words("cards", "named")) return null;
    const name = c.next();
    if (name?.kind !== "card-name") return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "deck-copy-limit", name: { name: "literal", value: name.value }, ...(maximum === undefined ? {} : { maximum }), during: "deck-construction" };
  });
}

export function parsePregameRule(c: Cursor, parseBody: (c: Cursor) => Sentence[] | null): StaticEffect | null {
  return parseBeginOnBattlefield(c) ?? parseRevealFromOpeningHand(c, parseBody) ?? parseDeckCopyLimit(c);
}
