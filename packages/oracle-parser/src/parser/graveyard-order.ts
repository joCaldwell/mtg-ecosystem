// Cards identified by their position in a graveyard. CR 404.1–404.2: cards
// are put on top of their owner's graveyard and the pile's order is kept, so
// "the top creature card of your graveyard" is determined, never chosen.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, ObjectFilter, ObjectRef, ZoneRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";

interface Grammar {
  parseAmount(c: Cursor): Amount | null;
  parseFilter(c: Cursor): ObjectFilter | null;
  parseZoneRef(c: Cursor): ZoneRef | null;
}

/** "the top [N] [creature] card(s) of <one graveyard>". Words between the
 * count and "of" must be one complete card description with nothing else
 * (no zone, pool or relation of its own). */
export function parseGraveyardTopCards(c: Cursor, g: Grammar): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (!c.words("the", "top")) return null;
    let end = c.pos;
    for (;; end++) {
      const token = c.tokens[end];
      if (token?.kind !== "word" || token.possessive) return null;
      if (token.value === "of") break;
    }
    const head = new Cursor(c.tokens.slice(c.pos, end));
    const count = head.attempt(g.parseAmount) ?? { amount: "fixed" as const, value: 1 };
    const noun = head.tokens.at(-1);
    const plural = noun?.kind === "word" && noun.value === "cards";
    if (plural !== !(count.amount === "fixed" && count.value === 1)) return null;
    const described = g.parseFilter(head);
    if (!described || !head.done() || described.cls !== "card") return null;
    const { cls: _cls, ...matching } = described;
    if (Object.keys(matching).some(key => !["types", "nonTypes", "subtypes", "supertypes", "colors", "permanent"].includes(key))) return null;
    c.pos = end + 1;
    const of = g.parseZoneRef(c);
    // One identified graveyard: pooled or distributed graveyards have no single order.
    if (!of || of.zone !== "graveyard" || !of.owner || of.position ||
        (typeof of.owner === "string" && !["your", "their", "its-owner", "that-player", "an-opponent"].includes(of.owner))) return null;
    return { ref: "graveyard-cards", position: "top", count, ...(Object.keys(matching).length ? { matching } : {}), of };
  });
}
