// Destination-first battlefield moves: "Put onto the battlefield under your
// control all creature cards in all graveyards …", "Return to the battlefield
// tapped all artifact … cards in your graveyard …". The destination and its
// entry state precede a long object phrase; the instruction is the same
// zone move as "put/return [objects] onto/to the battlefield …" (CR 608.2c:
// the text is read as ordinary English, the word order carries no meaning).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseBattlefieldEntryDetails } from "./entry.ts";

type ZoneMove = Extract<Effect, { effect: "move-zone" }>;

/** Called right after the verb. Only "tapped" and a controller may stand
 * between the destination and the objects; the object phrase must close the
 * clause, so nothing after it can be mistaken for part of the destination. */
export function parseInvertedBattlefieldMove(c: Cursor, verb: "put" | "return",
    parseObjects: (c: Cursor) => ObjectRef[] | null): ZoneMove[] | null {
  return c.attempt(c => {
    if (c.word(verb === "put" ? "onto" : "to") === null || !c.words("the", "battlefield")) return null;
    const details = parseBattlefieldEntryDetails(c);
    if (!details || details.from || details.entry) return null;
    const refs = parseObjects(c);
    if (!refs) return null;
    if (!c.isPunct(".") && !c.done()) return c.fail("end of a destination-first move");
    return refs.map(what => ({ effect: "move-zone", what, to: { zone: "battlefield" }, ...details }));
  });
}
