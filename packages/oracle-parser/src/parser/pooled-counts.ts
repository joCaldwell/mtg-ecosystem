// "For each creature you control and each creature card in your graveyard",
// "the greatest power among creatures you control and creature cards in your
// graveyard": two counted pools joined by "and". Objects in different zones
// are different objects (CR 400.7), so when the pools cannot overlap the
// count over both is the count over their union. Pools that can share an
// object (two filters over the same zone) are not merged: whether such an
// object counts once or twice is not stated by this construct.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

/** Where every member of a pool is: the battlefield (a permanent
 * description with no zone phrase, CR 110.1) or one named zone. */
function poolZone(filter: ObjectFilter): string | null {
  if (filter.anyOf || filter.allOf || filter.zoneChangeHistory) return null;
  if (!filter.zone) {
    // Cards, spells, and abilities need a stated place; only permanent
    // descriptions default to the battlefield.
    return filter.cls === undefined || filter.cls === "permanent" ? "battlefield" : null;
  }
  if (filter.zone.position || filter.zone.zone === "battlefield") return null;
  return filter.zone.zone;
}

/** Can no object satisfy both filters? Only provable zone disjointness counts. */
export function disjointPools(a: ObjectFilter, b: ObjectFilter): boolean {
  const zoneA = poolZone(a);
  const zoneB = poolZone(b);
  return !!zoneA && !!zoneB && zoneA !== zoneB;
}

/** Called right after the first pool. `each` is true when every pool repeats
 * its determiner ("for each A and each B"). */
export function parsePooledFilter(c: Cursor, first: ObjectFilter, parsePool: (c: Cursor) => ObjectFilter | null, each: boolean): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (c.word("and") === null) return null;
    if (each && c.word("each") === null) return null;
    const second = parsePool(c);
    if (!second || !disjointPools(first, second)) return null;
    return { anyOf: [first, second] };
  });
}
