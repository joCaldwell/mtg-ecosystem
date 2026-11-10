// Continuous characteristic changes apply in layers 4–7, with the entire
// effect retaining its affected objects across those layers (CR 613.6).
// Replacement/addition, corresponding subtype families, and basic-land
// setting have distinct semantics (CR 105.3, 205.1a-b, 305.7).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, ChosenValueRef, Color, GainedAbility, ObjectRef, StaticModifier, SubtypeFamily, TypeChange } from '../ast.ts';
import { CARD_TYPES, COLORS, SUPERTYPES, canonicalSubtypeName, singularize } from '../vocab.ts';
import { Cursor } from './cursor.ts';
import { parseAmount, parseAmountAdjustment, parseFilter } from './refs.ts';

type AbilityParser = (c: Cursor) => GainedAbility[] | null;
const FAMILY_NAMES: Partial<Record<SubtypeFamily, string>> = {
  artifact: 'Attraction|Blood|Bobblehead|Book|Clue|Contraption|Equipment|Food|Fortification|Gold|Heartwood|Incubator|Infinity|Junk|Lander|Map|Mutagen|Powerstone|Spacecraft|Stone|Treasure|Vehicle|Vibranium',
  enchantment: 'Aura|Background|Cartouche|Case|Class|Curse|Plan|Role|Room|Rune|Saga|Shard|Shrine',
  land: "Cave|Desert|Forest|Gate|Island|Lair|Locus|Mine|Mountain|Plains|Planet|Power-Plant|Sphere|Swamp|Tower|Town|Urza's",
  planeswalker: 'Ajani|Aminatou|Angrath|Arlinn|Ashiok|Bahamut|Basri|Bolas|Calix|Chandra|Comet|Dack|Dakkon|Daretti|Davriel|Dellian|Dihada|Domri|Dovin|Ellywick|Elminster|Elspeth|Estrid|Freyalise|Garruk|Gideon|Grist|Guff|Huatli|Jace|Jared|Jaya|Jeska|Kaito|Karn|Kasmina|Kaya|Kiora|Koth|Liliana|Lolth|Lukka|Minsc|Mordenkainen|Nahiri|Narset|Niko|Nissa|Nixilis|Oko|Quintorius|Ral|Rowan|Saheeli|Samut|Sarkhan|Serra|Sivitri|Sorin|Szat|Tamiyo|Tasha|Teferi|Teyo|Tezzeret|Tibalt|Tyvar|Ugin|Urza|Venser|Vivien|Vraska|Vronos|Will|Windgrace|Wrenn|Xenagos|Yanggu|Yanling|Zariel',
  spell: 'Adventure|Arcane|Lesson|Omen|Trap', battle: 'Siege', dungeon: 'Undercity',
};
const FAMILIES = new Map(Object.entries(FAMILY_NAMES).flatMap(([family, names]) =>
  names.split('|').map(name => [name, family as SubtypeFamily] as const)));

/** Call only after canonicalSubtypeName has validated the subtype. */
export function characteristicSubtypeFamily(name: string): SubtypeFamily {
  return FAMILIES.get(name) ?? 'creature';
}

function subtype(c: Cursor): { name: string; family: SubtypeFamily } | null {
  if (c.words('time', 'lord')) return { name: 'Time Lord', family: 'creature' };
  const word = c.peek();
  if (word?.kind !== 'word' || word.possessive) return null;
  const name = canonicalSubtypeName(word.raw);
  if (!name) return null;
  c.pos++;
  return { name, family: characteristicSubtypeFamily(name) };
}

/** Separate dynamic values retain a per-affected-object interpretation when
 * they mention "its": March of the Machines does not use the source's MV. */
export function parseStaticBasePt(c: Cursor): { power: Amount; toughness: Amount } | null {
  return c.attempt(c => {
    c.word('base');
    if (!c.words('power', 'and')) return null;
    c.word('base');
    if (c.word('toughness') === null) return null;
    const dynamic = c.attempt(c => {
      c.word('each');
      if (!c.words('equal', 'to')) return null;
      return parseAmount(c);
    });
    if (dynamic) return { power: dynamic, toughness: structuredClone(dynamic) };
    const power = parseAmount(c);
    if (!power || !c.punct('/')) return null;
    const toughness = parseAmount(c);
    return toughness ? { power, toughness } : null;
  });
}

/** "[~'s power is equal to N] and its toughness is equal to that number
 * plus 1": one characteristic-defining ability with two values (CR 604.3,
 * 613.4a). "That number" is the power value stated in the same sentence, so
 * the toughness repeats that expression instead of a contextual reference. */
export function parseCoordinatedCdaToughness(c: Cursor, power: Amount): Amount | null {
  return c.attempt(c => {
    if (!c.words("and", "its", "toughness", "is", "equal", "to")) return null;
    if (c.words("that", "number")) return parseAmountAdjustment(c, structuredClone(power));
    return parseAmount(c);
  });
}

function retainedTypes(c: Cursor, plural: boolean): boolean {
  return !!c.attempt(c => {
    const sameSentence = plural ? c.words('that', 'are') : c.word("that's") !== null || c.words('that', 'is');
    if (!sameSentence) {
      if (!c.punct('.')) return null;
      const pronoun = plural ? c.word("they're") !== null || c.words('they', 'are') : c.word("it's") !== null || c.words('it', 'is');
      if (!pronoun) return null;
    }
    if (c.word('still') === null) return null;
    c.word('a', 'an');
    const noun = c.anyWord();
    if (!noun || !CARD_TYPES.has(singularize(noun.value))) return null;
    return true;
  });
}

/** "The same is true for <objects> and <objects>." extends one continuous
 * type/color change to the stated spells and cards in their own zones
 * (Conspiracy; CR 611.3a). Each added set is an explicit, zone-bearing
 * filter; any other "the same is true" rider remains unsupported here. */
export function parseSameIsTrueSubjects(c: Cursor, what: ObjectRef, modifiers: StaticModifier[]): ObjectRef[] | null {
  return c.attempt(c => {
    if (what.ref !== 'each' || !modifiers.every(modifier => (modifier.modifier === 'types' || modifier.modifier === 'colors')
      && !modifier.condition && !modifier.forEach && !modifier.multiplier)) return null;
    if (!c.words('the', 'same', 'is', 'true', 'for')) return null;
    const subjects: ObjectRef[] = [];
    for (;;) {
      const filter = parseFilter(c);
      // Only spells and cards outside the battlefield extend a battlefield rule.
      if (!filter || !(filter.cls === 'spell' || filter.cls === 'card' && filter.outsideZone)) return null;
      subjects.push({ ref: 'each', filter });
      if (c.word('and') === null) break;
    }
    return subjects;
  });
}

export function parseCharacteristicModifiers(c: Cursor, _what: ObjectRef, parseAbilities: AbilityParser): StaticModifier[] | null {
  return c.attempt(c => {
    // A pronoun subject can carry its verb: "it's a creature in addition ...".
    const verb = c.word("isn't", "aren't", 'is', 'are') ?? (_what.ref === 'it' ? c.word("it's") && 'is'
      : _what.ref === 'them' ? c.word("they're") && 'are' : null);
    if (!verb) return null;
    const negative = verb === "isn't" || verb === "aren't";
    const noLonger = c.words('no', 'longer');
    const also = c.word('also') !== null;
    c.word('a', 'an');
    const cardTypes: string[] = [], supertypes: string[] = [];
    const subtypes = new Map<SubtypeFamily, string[]>();
    let colors: Color[] | ChosenValueRef<'color'> | undefined;
    let allSubtypes: { family: 'land' | 'creature'; basic?: boolean } | undefined;
    let chosenSubtype: NonNullable<TypeChange['chosenSubtype']>['choice'] | undefined;
    let stats: { power: Amount; toughness: Amount } | undefined;
    let abilities: GainedAbility[] | undefined;
    const earlyStats = c.attempt(c => {
      const power = c.number();
      if (power === null || !c.punct('/')) return null;
      const toughness = c.number();
      return toughness === null ? null : { power: { amount: 'fixed' as const, value: power }, toughness: { amount: 'fixed' as const, value: toughness } };
    });
    if (earlyStats) stats = earlyStats;
    if (c.words('all', 'colors')) colors = ['white', 'blue', 'black', 'red', 'green'];
    else if (c.words('the', 'chosen', 'color')) colors = { choice: 'chosen-value', category: 'color', which: 'chosen' };
    else if (c.words('the', 'chosen', 'type')) chosenSubtype = { choice: 'chosen-type' };
    else if (c.words('the', 'chosen', 'creature', 'type')) chosenSubtype = { choice: 'chosen-type', kind: 'creature' };
    else if (c.words('the', 'chosen', 'land', 'type')) chosenSubtype = { choice: 'chosen-type', kind: 'land' };
    else if (c.word('all', 'every') !== null) {
      const basic = c.word('basic', 'nonbasic');
      const family = c.word('land', 'creature');
      if (family !== 'land' && family !== 'creature' || !c.word('type', 'types') || basic && family !== 'land') return null;
      allSubtypes = { family, ...(basic ? { basic: basic === 'basic' } : {}) };
    } else {
      let consumed = false;
      for (;;) {
        const part = c.attempt(c => {
          const word = c.peek();
          if (word?.kind !== 'word' || word.possessive) return null;
          if (COLORS.has(word.value)) {
            if (colors && (!Array.isArray(colors) || colors.length === 0)) return null;
            c.pos++;
            if (!colors) colors = [];
            if (!Array.isArray(colors)) return null;
            colors.push(word.value as Color); return true;
          }
          if (word.value === 'colorless') {
            if (colors) return null;
            c.pos++; colors = []; return true;
          }
          if (CARD_TYPES.has(singularize(word.value))) { c.pos++; cardTypes.push(singularize(word.value)); return true; }
          if (SUPERTYPES.has(word.value)) { c.pos++; supertypes.push(word.value); return true; }
          const type = subtype(c);
          if (!type) return null;
          subtypes.set(type.family, [...(subtypes.get(type.family) ?? []), type.name]);
          return true;
        });
        if (!part) break;
        consumed = true;
        const separator = c.pos;
        const comma = c.punct(',');
        const and = c.word('and') !== null;
        if (comma || and) {
          const next = c.peek();
          const characteristic = next?.kind === 'word' && !next.possessive && (COLORS.has(next.value)
            || SUPERTYPES.has(next.value) || CARD_TYPES.has(singularize(next.value)) || !!canonicalSubtypeName(next.raw));
          if (!characteristic) { c.pos = separator; break; }
        }
      }
      if (!consumed) return null;
    }
    if (c.word('with') !== null) {
      const pt = parseStaticBasePt(c);
      if (pt) stats = pt;
      else {
        abilities = parseAbilities(c) ?? undefined;
        if (!abilities) return null;
      }
    }
    let addTypes = also, addColors = false;
    if (c.words('in', 'addition', 'to')) {
      if (c.word('its', 'their') === null || c.word('other') === null) return null;
      if (c.word('colors') !== null) {
        addColors = true;
        if (c.word('and') !== null) {
          if (c.word('types') === null) return null;
          addTypes = true;
        }
      } else {
        c.word('land', 'creature');
        if (c.word('types') === null) return null;
        addTypes = true;
      }
      if (addTypes && !cardTypes.length && !subtypes.size && !allSubtypes && !chosenSubtype && !supertypes.length || addColors && !colors) return null;
    }
    // CR 205.1a: "and [it] loses all other card types" restates that the
    // stated card type replaces the others, which is what setting means; it
    // contradicts a retained type (205.1b). "…and abilities" additionally
    // removes every ability this effect doesn't itself grant (613.1f).
    const othersLost = c.attempt(c => {
      c.punct(',');
      if (c.word('and') === null) return null;
      c.word('it', 'they');
      if (c.word('loses', 'lose') === null || !c.words('all', 'other', 'card', 'types')) return null;
      return { abilities: c.words('and', 'abilities') };
    });
    if (retainedTypes(c, verb === 'are' || verb === "aren't")) addTypes = true;
    if (othersLost && (!cardTypes.length || addTypes || also || negative || noLonger
      || cardTypes.includes('artifact') && cardTypes.includes('creature') || othersLost.abilities && !abilities)) return null;
    // "Also" is currently represented only for types. It must not silently
    // turn an additional color into a color replacement.
    if (also && colors) return null;
    if ((negative || noLonger) && (colors || stats || abilities || allSubtypes || chosenSubtype || also || addTypes || addColors)) return null;
    const removing = negative || noLonger;
    // Scryfall ruling (Tezzeret, Agent of Bolas): an artifact that "becomes an
    // artifact creature" retains its other types, subtypes and supertypes.
    const artifactCreature = cardTypes.includes('artifact') && cardTypes.includes('creature');
    const change: TypeChange = {};
    if (cardTypes.length) change.cardTypes = { operation: removing ? 'remove' : addTypes || artifactCreature ? 'add' : 'set', types: cardTypes };
    if (supertypes.length) change.supertypes = { operation: removing ? 'remove' : 'add', types: supertypes };
    if (subtypes.size) change.subtypes = [...subtypes].map(([family, types]) => ({ family,
      operation: removing ? 'remove' : addTypes || artifactCreature && family !== 'creature' ? 'add' : 'set', types }));
    if (allSubtypes) change.subtypes = [{ family: allSubtypes.family, operation: addTypes ? 'add' : 'set',
      types: { all: true, ...(allSubtypes.basic === undefined ? {} : { basic: allSubtypes.basic }) } }];
    // CR 205.1a/b, 305.7: without "in addition" the chosen subtype replaces
    // the subtypes of its own family (the linked choice supplies that family).
    // A source replacing its own subtypes with a linked choice is left
    // unsupported (pinned by static-characteristics.test.ts).
    if (chosenSubtype && !addTypes && (_what.ref === 'self' || _what.ref === 'this')) return null;
    if (chosenSubtype) change.chosenSubtype = { operation: addTypes ? 'add' : 'set', choice: chosenSubtype };
    const modifiers: StaticModifier[] = [];
    if (Object.keys(change).length) modifiers.push({ modifier: 'types', change });
    if (colors) modifiers.push({ modifier: 'colors', operation: addColors ? 'add' : 'set', colors });
    if (stats) modifiers.push({ modifier: 'base-pt', ...stats,
      ...(stats.power.amount !== 'fixed' || stats.toughness.amount !== 'fixed' ? { valueScope: 'each-affected-object' as const } : {}) });
    if (abilities) modifiers.push({ modifier: 'abilities', abilities });
    if (othersLost?.abilities) modifiers.push({ modifier: 'lose-abilities', removal: { removal: 'all-other', than: 'abilities-granted-by-this-effect' } });
    return modifiers.length ? modifiers : null;
  });
}
