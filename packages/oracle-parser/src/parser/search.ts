// Searching multiple zones is one instruction. The number and qualities of
// found cards apply across those zones, while hidden/public search rules are
// applied to each source zone (CR 701.23a-d, 701.23h).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { SearchZones, ZoneRef } from '../ast.ts';
import { Cursor } from './cursor.ts';
import { parseZoneRef } from './refs.ts';

function validZone(zone: ZoneRef): boolean {
  // A positioned library is not a whole zone, and search limits such as
  // 'the top four cards' need an explicit region rather than discarded words.
  return !zone.position && (!['library', 'graveyard', 'hand'].includes(zone.zone) || zone.owner !== undefined);
}

export function parseSearchZones(c: Cursor): SearchZones | null {
  return c.attempt(c => {
    const first = parseZoneRef(c);
    if (!first || !validZone(first)) return null;
    const of = [first];
    let selection: SearchZones['selection'] | undefined;
    let commaWithoutJoin = false;
    for (;;) {
      const next = c.attempt(c => {
        const comma = c.punct(',');
        let join: SearchZones['selection'] | undefined;
        if (c.word('and') !== null) {
          if (c.punct('/')) {
            if (c.word('or') === null) return null;
            join = 'one-or-more';
          } else join = 'all';
        }
        else if (c.word('or') !== null) join = 'one';
        if (!comma && !join) return null;
        const parsed = parseZoneRef(c);
        if (!parsed) return null;
        // A coordinated bare zone inherits the nearest stated owner, which
        // may change midway through a list of different players' zones.
        const zone = parsed.owner === undefined ? { ...parsed, owner: of.at(-1)!.owner } : parsed;
        return validZone(zone) ? { zone, join } : null;
      });
      if (!next) break;
      if (selection && next.join && selection !== next.join) return null;
      if (of.some(zone => JSON.stringify(zone) === JSON.stringify(next.zone))) return null;
      of.push(next.zone);
      if (next.join) selection = next.join;
      commaWithoutJoin = next.join === undefined;
    }
    if (of.length > 1 && (!selection || commaWithoutJoin)) return c.fail('complete search-zone list');
    return { selection: selection ?? 'all', of };
  });
}
