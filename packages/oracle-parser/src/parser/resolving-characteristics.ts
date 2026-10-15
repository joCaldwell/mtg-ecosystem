// Resolving characteristic changes form one effect across layers 4–7.
// Type replacement/retention: CR205.1a/b; color:105.3; values:611.2c/d;
// ordinary granted definitions:604.3a; layers:613.1/4/6.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Color, Effect, GainedAbility, ObjectRef, ResolvingCharacteristicModifier, SubtypeFamily, TypeChange } from '../ast.ts';
import { CARD_TYPES, COLORS, SUPERTYPES, canonicalSubtypeName, singularize } from '../vocab.ts';
import { Cursor } from './cursor.ts';
import { characteristicSubtypeFamily, parseCharacteristicModifiers, parseStaticBasePt } from './characteristics.ts';
import { parseValueChoice } from './choices.ts';
import { parseAmount, parseDuration, parseEqualTo, parseObjectRef, parsePossessiveObject } from './refs.ts';

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

/** "…becomes an X/X Elemental creature until end of turn, where X is ….
 * It's still a land.": the retention sentence follows the binding clause of
 * the instruction it completes. It turns that instruction's type setting
 * into an addition exactly as the adjacent form does (CR 205.1b). */
export function foldRetainedTypeSentence(c: Cursor, previous: { steps: Effect[]; bindings?: unknown }): boolean {
  const effect = previous.steps.at(-1);
  if (previous.steps.length !== 1 || !previous.bindings || effect?.effect !== 'modify-characteristics' || effect.statedAs) return false;
  const types = effect.modifiers.find(modifier => modifier.modifier === 'types');
  if (!types || types.modifier !== 'types') return false;
  const start = c.pos;
  const plural = pluralSubject(effect.what);
  const stated = (plural ? c.word("they're") !== null || c.words('they', 'are') : c.word("it's") !== null || c.words('it', 'is'))
    && c.word('still') !== null;
  c.word('a', 'an');
  const noun = stated ? c.anyWord() : null;
  if (!noun || !(CARD_TYPES.has(singularize(noun.value)) || SUPERTYPES.has(noun.value) || canonicalSubtypeName(noun.raw))
    || !(c.punct('.') || c.done())) { c.pos = start; return false; }
  const change = types.change;
  if (change.cardTypes?.operation === 'set') change.cardTypes = { ...change.cardTypes, operation: 'add' };
  if (change.subtypes) change.subtypes = change.subtypes.map(entry => entry.operation === 'set' ? { ...entry, operation: 'add' as const } : entry);
  if (change.chosenSubtype?.operation === 'set') change.chosenSubtype = { ...change.chosenSubtype, operation: 'add' };
  return true;
}

/** The preceding complete sentence of this same body instructs choosing a
 * subtype, so "that type" has exactly one antecedent (CR 205.3e, 608.2c).
 * A lexical proof only: the choice sentence must itself still parse. */
function precedingTypeChoice(c: Cursor): 'creature' | 'land' | null {
  const punct = (index: number, ...values: string[]) => {
    const token = c.tokens[index];
    return token.kind === 'punct' && values.includes(token.value);
  };
  let end = c.pos - 1;
  while (end >= 0 && !punct(end, '.')) end--;
  if (end < 0) return null;
  let start = end - 1;
  while (start >= 0 && !punct(start, '.', ':', '—', ',')) start--;
  const sentence = new Cursor(c.tokens.slice(start + 1, end));
  if (sentence.word('choose') === null) return null;
  // Includes an excluded value: "choose a creature type other than Wall".
  const choice = parseValueChoice(sentence);
  const kind = choice?.category === 'creature-type' ? 'creature' : choice?.category === 'basic-land-type' ? 'land' : null;
  return kind && sentence.done() ? kind : null;
}

/** Values supplied by a choice rather than printed: a fresh choice made as
 * the effect is created (608.2d) or a symbolic earlier choice (607.2d).
 * Setting a color or subtype replaces the previous ones unless the effect
 * says "in addition" (105.3, 205.1a/b); a set basic land type is 305.7. */
function parseChoiceCharacteristics(c: Cursor): ResolvingCharacteristicModifier[] | null {
  return c.attempt((c): ResolvingCharacteristicModifier[] | null => {
    const you = { player: 'you' } as const;
    const inAddition = (kind: 'types' | 'colors') => c.attempt(c => {
      if (!c.words('in', 'addition', 'to') || c.word('its', 'their') === null || c.word('other') === null) return null;
      if (kind === 'types') c.word('creature', 'land');
      return c.word(kind) !== null ? true : null;
    }) === true;
    if (c.words('all', 'colors'))
      return [{ modifier: 'colors', operation: 'set', colors: ['white', 'blue', 'black', 'red', 'green'] }];
    if (c.words('that', 'type')) {
      const kind = precedingTypeChoice(c);
      if (!kind) return c.fail('preceding subtype choice for "that type"');
      return [{ modifier: 'types', change: { chosenSubtype: { operation: inAddition('types') ? 'add' : 'set', choice: { choice: 'chosen-type', kind } } } }];
    }
    if (c.word('the') === null) return null;
    const last = c.word('last') !== null;
    if (c.word('chosen') !== null) {
      if (c.word('color') !== null)
        return [{ modifier: 'colors', operation: inAddition('colors') ? 'add' : 'set',
          colors: { choice: 'chosen-value', category: 'color', which: last ? 'last-chosen' : 'chosen' } }];
      if (last) return null;
      const kind = c.word('creature', 'land');
      if (c.word('type') === null) return null;
      return [{ modifier: 'types', change: { chosenSubtype: { operation: inAddition('types') ? 'add' : 'set',
        choice: { choice: 'chosen-type', ...(kind === 'creature' || kind === 'land' ? { kind } : {}) } } } }];
    }
    if (last) return null;
    if (c.word('color') !== null) {
      const quantity = c.words('or', 'colors') ? 'one-or-more' : 'one';
      if (!c.words('of', 'your', 'choice')) return null;
      return [{ modifier: 'colors', operation: inAddition('colors') ? 'add' : 'set',
        colors: { choice: 'fresh-choice', category: 'color', who: you, when: 'effect-created', quantity } }];
    }
    const category = c.words('basic', 'land', 'type') ? 'basic-land-type' : c.words('creature', 'type') ? 'creature-type' : null;
    if (!category || !c.words('of', 'your', 'choice')) return null;
    return [{ modifier: 'types', change: { subtypes: [{ family: category === 'creature-type' ? 'creature' : 'land',
      operation: inAddition('types') ? 'add' : 'set',
      types: { choice: 'fresh-choice', category, who: you, when: 'effect-created' } }] } }];
  });
}

function pluralSubject(what: ObjectRef): boolean {
  if (what.ref === 'them' || what.ref === 'each' || what.ref === 'all' || what.ref === 'zone-cards') return true;
  if (what.ref === 'that') return !!what.plural;
  if ('number' in what) return what.number === 'plural';
  if ('count' in what && what.count) return what.count.amount !== 'fixed' || what.count.value !== 1;
  return false;
}

type BasePt = Extract<ResolvingCharacteristicModifier, { modifier: 'base-pt' }>;
type Modify = Extract<Effect, { effect: 'modify-characteristics' }>;

/** CR 613.4b: an effect that sets only base power or only base toughness
 * leaves the other value alone. Values are fixed when the effect begins
 * (611.2d). "[object]'s base ... becomes" changes it; "[object] has base
 * ..." states it (611.2e). The fixed "has base power and toughness N/N"
 * form keeps its existing set-pt node and is not accepted here. */
function parseBasePtChange(c: Cursor, previous: ObjectRef | undefined): Modify | null {
  const stats = (c: Cursor): 'both' | 'power' | 'toughness' | null => c.attempt(c => {
    if (c.word('base') === null) return null;
    const first = c.word('power', 'toughness');
    if (first === 'toughness') return 'toughness';
    if (first !== 'power') return null;
    return c.attempt(c => {
      if (c.word('and') === null) return null;
      c.word('base');
      return c.word('toughness') !== null ? 'both' as const : null;
    }) ?? 'power';
  });
  const scoped = (value: Omit<BasePt, 'modifier' | 'valueScope'>): BasePt => ({ modifier: 'base-pt', ...value,
    ...([value.power, value.toughness].some(amount => amount && amount.amount !== 'fixed') ? { valueScope: 'each-affected-object' as const } : {}) } as BasePt);
  const single = (c: Cursor, stat: 'power' | 'toughness'): BasePt | null => {
    const value = parseEqualTo(c) ?? parseAmount(c);
    return value && scoped(stat === 'power' ? { power: value } : { toughness: value });
  };
  const finish = (what: ObjectRef, modifier: BasePt, statedAs?: 'has'): Modify => {
    const duration = parseDuration(c) ?? undefined;
    return { effect: 'modify-characteristics', what, modifiers: [modifier], snapshot: 'effect-created',
      ...(statedAs ? { statedAs } : {}), ...(duration ? { duration } : {}) };
  };
  return c.attempt((c): Modify | null => {
    const changed = c.attempt((c): Modify | null => {
      // "the base power and toughness of <object> become": the same change
      // as "<object>'s base power and toughness become".
      const ofForm = c.attempt((c): { what: ObjectRef; stat: 'both' | 'power' | 'toughness' } | null => {
        if (c.word('the') === null) return null;
        const stat = stats(c);
        const what = stat && c.word('of') !== null ? parseObjectRef(c) : null;
        return stat && what ? { what, stat } : null;
      });
      const what = ofForm?.what ?? parsePossessiveObject(c);
      const stat = ofForm?.stat ?? (what && stats(c));
      if (!what || !stat || c.word('becomes', 'become') === null) return null;
      if (stat !== 'both') {
        const modifier = single(c, stat);
        return modifier && finish(what, modifier);
      }
      const pair = c.attempt((c): BasePt | null => {
        // "equal to that creature's power and toughness": each from its own.
        if (!c.words('equal', 'to')) return null;
        const of = parsePossessiveObject(c);
        if (!of || !c.words('power', 'and', 'toughness')) return null;
        return scoped({ power: { amount: 'attribute', of, attribute: 'power' },
          toughness: { amount: 'attribute', of: structuredClone(of), attribute: 'toughness' } });
      }) ?? c.attempt((c): BasePt | null => {
        const power = parseAmount(c);
        if (!power || !c.punct('/')) return null;
        const toughness = parseAmount(c);
        return toughness && scoped({ power, toughness });
      }) ?? c.attempt((c): BasePt | null => {
        // "become equal to ~'s power": one value for both.
        const value = parseEqualTo(c);
        return value && scoped({ power: value, toughness: structuredClone(value) });
      });
      return pair && finish(what, pair);
    });
    if (changed) return changed;
    const what = parseObjectRef(c) ?? previous;
    if (!what || c.word('has', 'have') === null) return null;
    const start = c.pos;
    const stat = stats(c);
    if (!stat) return null;
    if (stat !== 'both') {
      const modifier = single(c, stat);
      return modifier && finish(what, modifier, 'has');
    }
    c.pos = start;
    const pt = parseStaticBasePt(c);
    if (!pt || pt.power.amount === 'fixed' && pt.toughness.amount === 'fixed') return null;
    return finish(what, scoped(pt), 'has');
  });
}

/** CR 611.2e: resolving text stating that an object "is/isn't
 * [characteristic]" creates the same kind of continuous effect as
 * "becomes", but applies simultaneously with that object's entry when this
 * resolution also puts it onto the battlefield. The subject is explicit;
 * a pronoun may carry the verb ("it's", "they're"). */
function parseStatedCharacteristics(c: Cursor, parseAbilities: AbilityParser, previous?: ObjectRef): Extract<Effect, { effect: 'modify-characteristics' }> | null {
  return c.attempt(c => {
    // After "return it ... at the beginning of the next end step." the stated
    // characteristic belongs to that delayed return (Grave Betrayal), not to
    // this resolution. That scope is not represented; keep it failing.
    for (let i = c.pos - 2; i >= 2; i--) {
      const token = c.tokens[i];
      if (token.kind === 'punct' && token.value === '.') break;
      const word = (index: number, value: string) => { const t = c.tokens[index]; return t.kind === 'word' && t.value === value; };
      // A line-initial "At the beginning of your upkeep," is this triggered
      // ability's own trigger, not a delayed instruction.
      const ownTrigger = i === 2 && !(word(i + 1, 'of') && word(i + 2, 'the') && word(i + 3, 'next'));
      if (word(i, 'beginning') && word(i - 1, 'the') && word(i - 2, 'at') && !ownTrigger) return c.fail('stated characteristic outside a delayed instruction');
    }
    // "It has flying and is an Angel …": a coordinated verb shares the
    // subject of the preceding step.
    const before = c.tokens[c.pos - 1];
    const coordinated = previous && before?.kind === 'word' && before.value === 'and' && c.isWord('is', 'are') ? previous : null;
    const what: ObjectRef | null = coordinated ?? (c.isWord("it's") ? { ref: 'it' } : c.isWord("they're") ? { ref: 'them' } : parseObjectRef(c));
    if (!what) return null;
    const stated = parseCharacteristicModifiers(c, what, parseAbilities);
    if (!stated) return null;
    // "is black and is a Nightmare in addition to its other creature types":
    // each predicate keeps its own set/add operation (CR 105.3, 205.1b).
    for (;;) {
      const next = c.attempt(c => c.word('and') !== null && c.isWord('is', 'are', "isn't", "aren't")
        ? parseCharacteristicModifiers(c, what, parseAbilities) : null);
      if (!next) break;
      stated.push(...next);
    }
    const modifiers: ResolvingCharacteristicModifier[] = [];
    for (const modifier of stated) {
      if (modifier.condition || modifier.forEach || modifier.multiplier) return null;
      if (modifier.modifier === 'types') modifiers.push({ modifier: 'types', change: modifier.change });
      else if (modifier.modifier === 'colors') modifiers.push({ modifier: 'colors', operation: modifier.operation, colors: modifier.colors });
      else if (modifier.modifier === 'base-pt' && modifier.power && modifier.toughness) modifiers.push({ modifier: 'base-pt',
        power: modifier.power, toughness: modifier.toughness, ...(modifier.valueScope ? { valueScope: modifier.valueScope } : {}) });
      else if (modifier.modifier === 'abilities') modifiers.push({ modifier: 'abilities', abilities: modifier.abilities });
      else return null;
    }
    // Oracle states a temporary change with "becomes ... until end of turn".
    // A stated characteristic lasts indefinitely or "for as long as" (611.2b);
    // any other trailing lifetime keeps this production from applying.
    const duration = parseDuration(c) ?? undefined;
    if (duration && duration.duration !== 'as-long-as') return null;
    return { effect: 'modify-characteristics', what, modifiers, snapshot: 'effect-created', statedAs: 'is', ...(duration ? { duration } : {}) };
  });
}

/** New descriptions only. Already supported simple BecomeSpec productions
 * keep their existing AST; no grammar text is rewritten to obtain a parse. */
export function parseResolvingCharacteristics(c: Cursor, previous: ObjectRef | undefined,
  parseAbilities: AbilityParser): Extract<Effect, { effect: 'modify-characteristics' }> | null {
  const stated = parseStatedCharacteristics(c, parseAbilities, previous) ?? parseBasePtChange(c, previous);
  if (stated) return stated;
  return c.attempt(c => {
    const what = parseObjectRef(c) ?? previous;
    if (!what) return null;
    const distributive = c.word('each') !== null;
    const verb = c.word('becomes', 'become');
    if (!verb) return null;
    const chosen = parseChoiceCharacteristics(c);
    if (chosen) {
      const duration = parseDuration(c) ?? undefined;
      return { effect: 'modify-characteristics', what, modifiers: chosen, snapshot: 'effect-created', ...(duration ? { duration } : {}) };
    }
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
        // The older BecomeSpec production never accepted a named family.
        if (c.word('creature', 'land') !== null) advanced = true;
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
