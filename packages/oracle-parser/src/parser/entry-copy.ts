// Copy replacements happen before entry. Copiable exceptions and entry
// state have different lifetimes (CR614.12,707.2,707.5,707.9a–g).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Color, CopyCharacteristicChange, EntryCopy, GainedAbility, ObjectRef, StaticEffect, SubtypeFamily, TypeChange } from '../ast.ts';
import { CARD_TYPES, COLORS, SUPERTYPES, canonicalSubtypeName } from '../vocab.ts';
import type { Cursor } from './cursor.ts';
import { parseCount, parseCounterSpec, parseDuration, parseFilter, parseObjectRef } from './refs.ts';
import { characteristicSubtypeFamily } from './characteristics.ts';
import { hasFreshProtectionChoice } from './protection.ts';

type Abilities = (c: Cursor) => GainedAbility[] | null;
type Exception = { changes?: CopyCharacteristicChange[]; entryExceptions?: EntryCopy['entryExceptions'] };

function self(ref: ObjectRef | null): boolean { return ref?.ref === 'self' || ref?.ref === 'this'; }

function parseCharacteristics(c: Cursor): CopyCharacteristicChange[] | null {
  return c.attempt(c => {
    if (!c.words("it's") && !c.words('it', 'is') && c.word('is') === null) return null;
    c.word('a', 'an');
    const changes: CopyCharacteristicChange[] = [];
    const stats = c.attempt(c => {
      const power = c.number();
      if (power === null || !c.punct('/')) return null;
      const toughness = c.number();
      return toughness === null ? null : { change: 'power-toughness' as const,
        power: { amount: 'fixed' as const, value: power }, toughness: { amount: 'fixed' as const, value: toughness } };
    });
    if (stats) changes.push(stats);
    const cardTypes: string[] = [], supertypes: string[] = [], colors: Color[] = [];
    const subtypes = new Map<SubtypeFamily, string[]>();
    let colorless = false;
    for (;;) {
      const t = c.peek();
      if (t?.kind !== 'word' || t.possessive) break;
      if (COLORS.has(t.value)) {
        if (colorless) return null;
        c.next(); colors.push(t.value as Color);
      } else if (t.value === 'colorless') {
        if (colorless || colors.length) return null;
        c.next(); colorless = true;
      } else if (CARD_TYPES.has(t.value)) { c.next(); cardTypes.push(t.value); }
      else if (SUPERTYPES.has(t.value)) { c.next(); supertypes.push(t.value); }
      else {
        const name = c.words('time', 'lord') ? 'Time Lord' : canonicalSubtypeName(t.raw);
        if (!name) break;
        if (name !== 'Time Lord') c.next();
        const family = characteristicSubtypeFamily(name);
        subtypes.set(family, [...(subtypes.get(family) ?? []), name]);
      }
      // Only a characteristic continuation belongs inside this phrase.
      const separator = c.pos;
      if (c.word('and') !== null) {
        const next = c.peek();
        if (next?.kind !== 'word' || next.possessive || !(COLORS.has(next.value)
          || SUPERTYPES.has(next.value) || CARD_TYPES.has(next.value) || canonicalSubtypeName(next.raw))) c.pos = separator;
      }
    }
    let addTypes = false, addColors = false;
    if (c.words('in', 'addition', 'to', 'its', 'other')) {
      if (c.word('colors') !== null) {
        addColors = true;
        if (c.word('and') !== null) {
          if (c.word('types') === null) return null;
          addTypes = true;
        }
      } else {
        c.word('creature', 'land');
        if (c.word('types') === null) return null;
        addTypes = true;
      }
      if (addTypes && !cardTypes.length && !supertypes.length && !subtypes.size || addColors && !colors.length) return null;
    }
    const types: TypeChange = {};
    // CR205.1b's artifact-creature exception retains previous card types;
    // a stated "in addition" also retains corresponding subtype families.
    const artifactCreature = cardTypes.includes('artifact') && cardTypes.includes('creature');
    if (cardTypes.length) types.cardTypes = { operation: addTypes || artifactCreature ? 'add' : 'set', types: cardTypes };
    if (supertypes.length) types.supertypes = { operation: 'add', types: supertypes };
    if (subtypes.size) types.subtypes = [...subtypes].map(([family, names]) => ({ family,
      operation: addTypes || artifactCreature && family !== 'creature' ? 'add' : 'set', types: names }));
    if (Object.keys(types).length) changes.push({ change: 'types', types });
    if (colors.length || colorless) changes.push({ change: 'colors', operation: addColors ? 'add' : 'set', colors });
    return changes.length ? changes : null;
  });
}

function parseException(c: Cursor, abilities: Abilities): Exception | null {
  return c.attempt(c => {
    if (c.words('it', "isn't", 'legendary') || c.words("it's", 'not', 'legendary'))
      return { changes: [{ change: 'types', types: { supertypes: { operation: 'remove', types: ['legendary'] } } }] };
    const gained = c.attempt(c => {
      if (!c.words('it', 'has') && c.word('has') === null) return null;
      const parsed = abilities(c);
      return parsed && !hasFreshProtectionChoice(parsed) ? parsed : null;
    });
    if (gained) return { changes: [{ change: 'abilities', abilities: gained }] };
    const counters = c.attempt(c => {
      if (!c.words('it', 'enters', 'with')) return null;
      const count = parseCount(c);
      if (!count) return null;
      const additional = c.word('additional') !== null;
      const counter = parseCounterSpec(c);
      if (!counter || c.word('counter', 'counters') === null || !c.words('on', 'it')) return null;
      return { exception: 'counters' as const, count, counter, ...(additional ? { additional: true as const } : {}) };
    });
    if (counters) return { entryExceptions: [counters] };
    const changes = parseCharacteristics(c);
    return changes ? { changes } : null;
  });
}

/** A complete entry-copy ability only. Unsupported exception/rider tails
 * fail as a unit instead of escaping into ordinary resolving instructions. */
export function parseEntryCopy(c: Cursor, abilities: Abilities): StaticEffect | null {
  return c.attempt(c => {
    let what: ObjectRef | null, optional = false, asEnters = false;
    if (c.word('as') !== null) {
      what = parseObjectRef(c);
      if (!self(what) || c.word('enters') === null || !c.punct(',') || !c.words('you', 'may', 'have', 'it', 'become')) return null;
      optional = true; asEnters = true;
    } else if (c.words('you', 'may', 'have')) {
      what = parseObjectRef(c);
      if (!self(what) || c.word('enter') === null) return null;
      optional = true;
    } else {
      what = parseObjectRef(c);
      if (!what || !['self', 'this', 'each', 'all'].includes(what.ref) || c.word('enter', 'enters') === null) return null;
    }
    if (!what) return null;
    const tapped = !asEnters && c.word('tapped') !== null;
    if (!asEnters && c.word('as') === null || !c.words('a', 'copy', 'of')) return null;
    const model = c.attempt((c): EntryCopy['model'] | null => {
      if (c.word('any', 'a', 'an') === null) return null;
      const filter = parseFilter(c);
      return filter ? { model: 'choose', who: { player: 'you' }, filter, count: 1, when: 'before-entry' } : null;
    }) ?? c.attempt((c): EntryCopy['model'] | null => {
      const ref = parseObjectRef(c);
      return ref && ['self', 'this', 'enchanted', 'equipped'].includes(ref.ref) ? { model: 'reference', what: ref } : null;
    });
    if (!model) return null;
    // The only explicit copy lifetime in this bounded entry production.
    const duration = parseDuration(c);
    if (duration && duration.duration !== 'end-of-turn') return null;
    const result: Extract<StaticEffect, { static: 'entry-copy' }> = {
      static: 'entry-copy', what, model, ...(optional ? { optional: { who: { player: 'you' as const } } } : {}),
      ...(tapped ? { entry: { tapped: true } } : {}), ...(duration ? { duration } : {}),
    };
    if (c.punct(',')) {
      if (c.word('except') === null) return null;
      for (;;) {
        const exception = parseException(c, abilities);
        if (!exception) return null;
        if (exception.changes) (result.changes ??= []).push(...exception.changes);
        if (exception.entryExceptions) (result.entryExceptions ??= []).push(...exception.entryExceptions);
        if (c.punct(',')) { c.word('and'); continue; }
        if (c.word('and') !== null) continue;
        break;
      }
    }
    c.punct('.');
    return c.done() ? result : null;
  });
}
