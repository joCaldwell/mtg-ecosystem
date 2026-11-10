// "All cards from target player's hand and graveyard": under "all", a second
// personal zone joined by "and" extends the set to the cards of both zones.
// The shared possessive or quantifier covers the bare second zone (CR 400.1).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter, ZoneRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

const SINGULAR = ["hand", "graveyard", "library"] as const;
const PLURAL = ["hands", "graveyards", "libraries"] as const;

/** Called right after the filter of an "all …" reference. Returns the filter
 * with both zones as alternatives of membership (the shape "from your hand
 * or graveyard" already has), or null when no second zone follows. */
export function parseAllZoneUnion(c: Cursor, filter: ObjectFilter): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    const { zone, ...rest } = filter;
    if (!zone || zone.position || !zone.owner || rest.anyOf || !(SINGULAR as readonly string[]).includes(zone.zone)) return null;
    // The zone phrase must close the description, so "and" continues it.
    const previous = c.tokens[c.pos - 1];
    const pooled = zone.owner === "all-players" || zone.owner === "all-opponents";
    const forms = pooled ? PLURAL : SINGULAR;
    if (previous?.kind !== "word" || previous.possessive || previous.value !== forms[SINGULAR.indexOf(zone.zone as typeof SINGULAR[number])]) return null;
    if (c.word("and") === null) return null;
    const word = c.word(...forms);
    if (!word || c.isWord("and", "or") || c.isPunct("/") || c.isPunct(",")) return null;
    const second: ZoneRef = { zone: SINGULAR[(forms as readonly string[]).indexOf(word)], owner: zone.owner };
    if (second.zone === zone.zone) return null;
    return { ...rest, anyOf: [{ zone }, { zone: second }] };
  });
}
