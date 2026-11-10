// Origin zones of an event stated by exclusion. CR 601.2a: casting moves the
// card from the zone it is in to the stack, so "from anywhere other than your
// hand" restricts that origin; it never describes where the spell is now.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ZoneRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

/** "from anywhere other than <zone>": every origin zone except the stated
 * one. The excluded zone must be a whole, owned or public zone. */
export function parseExcludedOrigin(c: Cursor, parseZoneRef: (c: Cursor) => ZoneRef | null): ZoneRef | null {
  return c.attempt(c => {
    if (!c.words("from", "anywhere", "other", "than")) return null;
    const zone = parseZoneRef(c);
    return zone && !zone.position ? zone : null;
  });
}
