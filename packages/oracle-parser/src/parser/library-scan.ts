// Sequential top-of-library reveal/exile instructions. Revealing does not
// move a card; a subsequent iteration advances past the revealed prefix.
// CR 609.3, 701.13, 701.20a/b; no hidden-zone search or arbitrary choice.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Effect, ObjectFilter, PlayerRef, ZoneRef } from '../ast.ts';
import { Cursor } from './cursor.ts';

interface Grammar {
  actor(c: Cursor, imperative: string, conjugated: string): PlayerRef | null;
  count(c: Cursor): Amount | null;
  filter(c: Cursor): ObjectFilter | null;
  player(c: Cursor): PlayerRef | null;
  zone(c: Cursor): ZoneRef | null;
}

function sameActor(actor: PlayerRef, repeated: PlayerRef): boolean {
  if (JSON.stringify(actor) === JSON.stringify(repeated)) return true;
  // These pronouns name the stated actor of the same instruction, including
  // the current actor of an each-player scan, not one pooled multiplayer scan.
  return actor.player !== 'you' && repeated.player === 'they';
}

function describesAnotherPool(filter: ObjectFilter): boolean {
  return !!(filter.zone || filter.fromAmong || filter.relation ||
    filter.allOf?.some(describesAnotherPool) || filter.anyOf?.some(describesAnotherPool));
}

export function parseLibraryScan(c: Cursor, g: Grammar): Effect[] | null {
  return c.attempt(c => {
    let operation: 'reveal' | 'exile' = 'reveal';
    let who = g.actor(c, 'reveal', 'reveals');
    if (!who) { operation = 'exile'; who = g.actor(c, 'exile', 'exiles'); }
    if (!who || !c.words('cards', 'from', 'the', 'top', 'of')) return null;
    const library = g.zone(c);
    if (!library || library.zone !== 'library' || !library.owner || library.position || c.word('until') === null) return null;
    const repeated = g.player(c);
    if (!repeated || !sameActor(who, repeated) || c.word(operation, `${operation}s`) === null) return null;
    const count = g.count(c);
    if (!count || (count.amount !== 'x' && (count.amount !== 'fixed' || count.value < 0))) return null;
    const matching = g.filter(c);
    if (!matching || matching.cls !== 'card' || describesAnotherPool(matching)) return null;
    return [{ effect: 'scan-library', who, operation, library, until: { count, matching } }];
  });
}
