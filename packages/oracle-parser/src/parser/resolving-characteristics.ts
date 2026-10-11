// Resolving characteristic changes form one effect across layers 4–7.
// Type replacement/retention: CR205.1a/b; color:105.3; values:611.2c/d;
// ordinary granted definitions:604.3a; layers:613.1/4/6.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Color, Effect, GainedAbility, ObjectRef, ResolvingCharacteristicModifier, SubtypeFamily, TypeChange } from '../ast.ts';
import { CARD_TYPES, COLORS, SUPERTYPES, canonicalSubtypeName, singularize } from '../vocab.ts';
import { Cursor } from './cursor.ts';
import { characteristicSubtypeFamily, parseStaticBasePt } from './characteristics.ts';
import { parseAmount, parseDuration, parseObjectRef, parsePossessiveObject } from './refs.ts';

type AbilityParser = (c: Cursor) => GainedAbility[] | null;
type Stats = { power: Amount; toughness: Amount };

function pairedStats(c: Cursor): Stats | null {
  return c.attempt(c => {
    c.word('base');
    if (!c.words('power', 'and', 'toughness', 'equal', 'to')) return null;
    const of = parsePossessiveObject(c);
    if (!of || !c.words('power', 'and', 'toughness')) return null;
    return { power: { amount: 'attribute', of, attribute: 'power' },
      toughness: { amount: 'attribute', of: structuredClone(of), attribute: 'toughness' } };
  });
}

/** Retaining any prior type retains all prior types and subtypes (205.1b).
 * The retained name is still validated; arbitrary trailing text is not a
 * permission to reinterpret a replacement as an addition. */
function retainedTypes(c: Cursor, plural: boolean): boolean {
  return !!c.attempt(c => {
    if (!(plural ? c.words('that', 'are') : c.word("that's") !== null || c.words('that', 'is'))) {
      const previous = c.tokens[c.pos - 1], beforeQuote = c.tokens[c.pos - 2];
      const quotedPeriod = previous?.kind === 'punct' && previous.value === '"'
        && beforeQuote?.kind === 'punct' && beforeQuote.value === '.';
      if (!c.punct('.') && !quotedPeriod) return null;
      if (!(plural ? c.word("they're") !== null || c.words('they', 'are')
        : c.word("it's", "he's", "she's") !== null || c.words('it', 'is') || c.words('he', 'is') || c.words('she', 'is'))) return null;
    }
    if (c.word('still') === null) return null;
    c.word('a', 'an');
    const noun = c.anyWord();
    return noun && (CARD_TYPES.has(singularize(noun.value)) || SUPERTYPES.has(noun.value)
      || canonicalSubtypeName(noun.raw)) ? true : null;
  });
}

function pluralSubject(what: ObjectRef): boolean {
  if (what.ref === 'them' || what.ref === 'each' || what.ref === 'all' || what.ref === 'zone-cards') return true;
  if (what.ref === 'that') return !!what.plural;
  if ('number' in what) return what.number === 'plural';
  if ('count' in what && what.count) return what.count.amount !== 'fixed' || what.count.value !== 1;
  return false;
}

/** New descriptions only. Already supported simple BecomeSpec productions
 * keep their existing AST; no grammar text is rewritten to obtain a parse. */
export function parseResolvingCharacteristics(c: Cursor, previous: ObjectRef | undefined,
  parseAbilities: AbilityParser): Extract<Effect, { effect: 'modify-characteristics' }> | null {
  return c.attempt(c => {
    const what = parseObjectRef(c) ?? previous;
    if (!what) return null;
    const distributive = c.word('each') !== null;
    const verb = c.word('becomes', 'become');
    if (!verb) return null;
    c.word('a', 'an');
    const cardTypes: string[] = [], supertypes: string[] = [];
    const subtypes = new Map<SubtypeFamily, string[]>();
    let colors: Color[] | undefined, stats: Stats | undefined, abilities: GainedAbility[] | undefined;
    let allCreatureTypes = false, advanced = distributive;
    // A printed supertype can precede the P/T pair (Genju of the Realm).
    for (;;) {
      const token = c.peek();
      if (token?.kind !== 'word' || token.possessive || !SUPERTYPES.has(token.value)) break;
      supertypes.push(token.value); c.next(); advanced = true;
    }
    const initialStats = c.attempt(c => {
      const power = parseAmount(c);
      if (!power || !c.punct('/')) return null;
      const toughness = parseAmount(c);
      return toughness ? { power, toughness } : null;
    });
    if (initialStats) {
      stats = initialStats;
      if (stats.power.amount !== 'fixed' || stats.toughness.amount !== 'fixed') advanced = true;
    }
    for (;;) {
      const token = c.peek();
      if (token?.kind !== 'word' || token.possessive) break;
      if (COLORS.has(token.value)) {
        if (colors?.length === 0) return null;
        (colors ??= []).push(token.value as Color); c.next();
      } else if (token.value === 'colorless') {
        if (colors) return null;
        colors = []; advanced = true; c.next();
      } else if (CARD_TYPES.has(singularize(token.value))) {
        cardTypes.push(singularize(token.value)); c.next();
      } else if (SUPERTYPES.has(token.value)) {
        supertypes.push(token.value); advanced = true; c.next();
      } else {
        const name = c.words('time', 'lord') ? 'Time Lord' : canonicalSubtypeName(token.raw);
        if (!name) break;
        if (name !== 'Time Lord') c.next();
        const family = characteristicSubtypeFamily(name);
        subtypes.set(family, [...(subtypes.get(family) ?? []), name]);
      }
      const separator = c.pos;
      if (c.word('and') !== null) {
        const next = c.peek();
        if (next?.kind !== 'word' || next.possessive || !(COLORS.has(next.value) || SUPERTYPES.has(next.value)
          || CARD_TYPES.has(singularize(next.value)) || canonicalSubtypeName(next.raw))) c.pos = separator;
      }
    }
    if (!stats && !colors && !cardTypes.length && !supertypes.length && !subtypes.size) return null;
    if (c.word('with') !== null) {
      for (;;) {
        const part = c.attempt(c => {
          const hadBase = c.isWord('base');
          const pt = pairedStats(c) ?? parseStaticBasePt(c);
          if (pt) {
            if (stats) return null;
            stats = pt;
            if (!hadBase || pt.power.amount !== 'fixed' || pt.toughness.amount !== 'fixed') advanced = true;
            return true;
          }
          if (c.words('all', 'creature', 'types')) {
            if (allCreatureTypes) return null;
            allCreatureTypes = true; advanced = true; return true;
          }
          const gained = parseAbilities(c);
          if (!gained) return null;
          (abilities ??= []).push(...gained); advanced = true; return true;
        });
        if (!part) return null;
        const connector = c.pos;
        if (c.word('and') === null) break;
        // Only a second characteristic belongs to this description. The
        // enclosing sentence parser retains independent coordinated verbs.
        if (!c.isWord('base', 'power', 'all')) { c.pos = connector; break; }
      }
    }
    let addTypes = false, addColors = false;
    if (c.words('in', 'addition', 'to')) {
      if (c.word('its', 'their') === null || c.word('other') === null) return null;
      if (c.word('colors') !== null) {
        addColors = true;
        if (c.word('and') !== null) {
          if (c.word('types') === null) return null;
          addTypes = true; advanced = true;
        }
      } else {
        c.word('creature', 'land');
        if (c.word('types') === null) return null;
        addTypes = true;
      }
      if (addTypes && !cardTypes.length && !supertypes.length && !subtypes.size && !allCreatureTypes
        || addColors && !colors?.length) return null;
    }
    const plural = pluralSubject(what);
    if (retainedTypes(c, plural)) { addTypes = true; advanced = true; }
    const duration = parseDuration(c) ?? undefined;
    if (retainedTypes(c, plural)) { addTypes = true; advanced = true; }
    if (!advanced) return null;
    const types: TypeChange = {};
    const artifactCreature = cardTypes.includes('artifact') && cardTypes.includes('creature');
    if (cardTypes.length) types.cardTypes = { operation: addTypes || artifactCreature ? 'add' : 'set', types: cardTypes };
    if (supertypes.length) types.supertypes = { operation: 'add', types: supertypes };
    if (subtypes.size) types.subtypes = [...subtypes].map(([family, types]) => ({ family,
      operation: addTypes || artifactCreature && family !== 'creature' ? 'add' : 'set', types }));
    if (allCreatureTypes) {
      if (subtypes.has('creature')) return null;
      (types.subtypes ??= []).push({ family: 'creature', operation: addTypes ? 'add' : 'set', types: { all: true } });
    }
    const modifiers: ResolvingCharacteristicModifier[] = [];
    if (Object.keys(types).length) modifiers.push({ modifier: 'types', change: types });
    if (colors) modifiers.push({ modifier: 'colors', operation: addColors ? 'add' : 'set', colors });
    if (stats) modifiers.push({ modifier: 'base-pt', ...stats,
      ...(stats.power.amount !== 'fixed' || stats.toughness.amount !== 'fixed' ? { valueScope: 'each-affected-object' as const } : {}) });
    if (abilities) modifiers.push({ modifier: 'abilities', abilities });
    return { effect: 'modify-characteristics', what, modifiers, snapshot: 'effect-created', ...(duration ? { duration } : {}) };
  });
}
