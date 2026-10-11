import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';
import { lex } from '../src/lexer.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { parseStatic } from '../src/parser/statics.ts';

// Complete named clauses verified in the cached Scryfall corpus. Relevant
// distinctions: CR 105.3, 205.1a-b, 305.7, 604.3a, 611.3, 613.4, and 613.6.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value });
const signed = (value: number) => ({ sign: value < 0 ? -1 : 1, amount: n(Math.abs(value)) });
const pt = (power: number, toughness: number) => ({ modifier: 'pt', power: signed(power), toughness: signed(toughness) });
const base = (power: number, toughness: number) => ({ modifier: 'base-pt', power: n(power), toughness: n(toughness) });
const enchanted = { ref: 'enchanted', noun: 'creature' };
const equipped = { ref: 'equipped', noun: 'creature' };
const source = { ref: 'this', noun: 'creature' };
const abilities = (...keywords: string[]) => ({ modifier: 'abilities', abilities: keywords.map(keyword => ({ gained: 'keyword', keyword: { keyword } })) });
const modify = (what: object, modifiers: object[]) => ({ kind: 'static', effect: { static: 'modify', what, modifiers } });
const types = (change: object) => ({ modifier: 'types', change });
const subtypes = (family: string, operation: string, types: unknown) => ({ subtypes: [{ family, operation, types }] });
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}

describe('continuous type, color, and P/T changes', () => {
  it('distinguishes Blood Moon basic-type setting from Urborg/Prismatic Omen additions', () => {
    assert.deepEqual(one('Nonbasic lands are Mountains.'), modify({ ref: 'each', filter: { nonSupertypes: ['basic'], types: ['land'] } }, [
      types(subtypes('land', 'set', ['Mountain'])),
    ]));
    assert.deepEqual(one('Each land is a Swamp in addition to its other land types.'), modify({ ref: 'each', filter: { types: ['land'] } }, [
      types(subtypes('land', 'add', ['Swamp'])),
    ]));
    assert.deepEqual(one('Lands you control are every basic land type in addition to their other types.'), modify({ ref: 'each', filter: { types: ['land'], control: 'you' } }, [
      types(subtypes('land', 'add', { all: true, basic: true })),
    ]));
  });
  it('keeps Song of the Dryads card types, land subtype, and colorlessness distinct', () => {
    assert.deepEqual(one('Enchanted permanent is a colorless Forest land.'), modify({ ref: 'enchanted', noun: 'permanent' }, [
      types({ cardTypes: { operation: 'set', types: ['land'] }, ...subtypes('land', 'set', ['Forest']) }),
      { modifier: 'colors', operation: 'set', colors: [] },
    ]));
  });
  it('keeps every Frogify layer part together with its affected object', () => {
    assert.deepEqual(one('Enchanted creature loses all abilities and is a blue Frog creature with base power and toughness 1/1.'), modify(enchanted, [
      { modifier: 'lose-abilities', removal: { removal: 'all' } },
      types({ cardTypes: { operation: 'set', types: ['creature'] }, ...subtypes('creature', 'set', ['Frog']) }),
      { modifier: 'colors', operation: 'set', colors: ['blue'] }, base(1, 1),
    ]));
  });
  it('retains Wind Zendikon existing types and corresponding subtypes under still-a-land', () => {
    assert.deepEqual(one("Enchanted land is a 2/2 blue Elemental creature with flying. It's still a land."), modify({ ref: 'enchanted', noun: 'land' }, [
      types({ cardTypes: { operation: 'add', types: ['creature'] }, ...subtypes('creature', 'add', ['Elemental']) }),
      { modifier: 'colors', operation: 'set', colors: ['blue'] }, base(2, 2), abilities('flying'),
    ]));
    assert.deepEqual(one('All lands are 2/2 creatures that are still lands.'), modify({ ref: 'all', filter: { types: ['land'] } }, [
      types({ cardTypes: { operation: 'add', types: ['creature'] } }), base(2, 2),
    ]));
  });
  it('applies the artifact-creature exception without retaining existing creature types', () => {
    assert.deepEqual(one('Enchanted artifact is a Golem artifact creature with base power and toughness 5/4.'), modify({ ref: 'enchanted', noun: 'artifact' }, [
      types({ cardTypes: { operation: 'add', types: ['artifact', 'creature'] }, ...subtypes('creature', 'set', ['Golem']) }), base(5, 4),
    ]));
    assert.deepEqual(one('Enchanted artifact is a Golem creature with base power and toughness 5/4 in addition to its other types.'), modify({ ref: 'enchanted', noun: 'artifact' }, [
      types({ cardTypes: { operation: 'add', types: ['creature'] }, ...subtypes('creature', 'add', ['Golem']) }), base(5, 4),
    ]));
  });
  it('retains all coordinated Equipment bonuses and additive color/type changes', () => {
    assert.deepEqual(one('Equipped creature gets +2/+0, has vigilance, and is a Knight in addition to its other types.'), modify(equipped, [
      pt(2, 0), abilities('vigilance'), types(subtypes('creature', 'add', ['Knight'])),
    ]));
    assert.deepEqual(one('Enchanted creature gets -1/-1 and is a black Zombie in addition to its other colors and types.'), modify(enchanted, [
      pt(-1, -1), types(subtypes('creature', 'add', ['Zombie'])), { modifier: 'colors', operation: 'add', colors: ['black'] },
    ]));
  });
  it('keeps Opalescence dynamic base P/T scoped to each affected enchantment', () => {
    const manaValue = { amount: 'attribute', of: { ref: 'it' }, attribute: 'mana-value' };
    assert.deepEqual(one('Each other non-Aura enchantment is a creature in addition to its other types and has base power and base toughness each equal to its mana value.'), modify({ ref: 'each', filter: { other: true, nonSubtypes: ['Aura'], types: ['enchantment'] } }, [
      types({ cardTypes: { operation: 'add', types: ['creature'] } }),
      { modifier: 'base-pt', power: manaValue, toughness: manaValue, valueScope: 'each-affected-object' },
    ]));
    assert.deepEqual(one('Each noncreature artifact is an artifact creature with power and toughness each equal to its mana value.'), modify({ ref: 'each', filter: { nonTypes: ['creature'], types: ['artifact'] } }, [
      types({ cardTypes: { operation: 'add', types: ['artifact', 'creature'] } }),
      { modifier: 'base-pt', power: manaValue, toughness: manaValue, valueScope: 'each-affected-object' },
    ]));
  });
  it('keeps supertype and explicit card-type removal independent of creature subtypes', () => {
    assert.deepEqual(one('Enchanted permanent is legendary.'), modify({ ref: 'enchanted', noun: 'permanent' }, [
      types({ supertypes: { operation: 'add', types: ['legendary'] } }),
    ]));
    assert.deepEqual(one("Equipped permanent isn't a planeswalker and is a creature in addition to its other types."), modify({ ref: 'equipped', noun: 'permanent' }, [
      types({ cardTypes: { operation: 'remove', types: ['planeswalker'] } }),
      types({ cardTypes: { operation: 'add', types: ['creature'] } }),
    ]));
  });
  it('distinguishes intrinsic Mistform subtype definition from an ordinary quoted grant', () => {
    const change = subtypes('creature', 'set', { all: true });
    assert.deepEqual(one('Mistform Ultimus is every creature type.', 'Mistform Ultimus'), {
      kind: 'static', effect: { static: 'cda-types', what: { ref: 'self' }, change },
    });
    assert.deepEqual(one('This creature gains "This creature is every creature type." until end of turn.'), {
      kind: 'spell', effects: [{ steps: [{ effect: 'gain-abilities', what: source,
        abilities: [{ gained: 'quoted', ability: modify(source, [types(change)]) }], duration: { duration: 'end-of-turn' },
      }] }],
    });
  });
  it('preserves chosen-color references and ordinary color grants without creating a new choice', () => {
    const choice = { choice: 'chosen-value', category: 'color', which: 'chosen' };
    assert.deepEqual(one('All nonland permanents are the chosen color.'), modify({ ref: 'all', filter: { nonTypes: ['land'], cls: 'permanent' } }, [
      { modifier: 'colors', operation: 'set', colors: choice },
    ]));
    assert.deepEqual(one('This creature gains "This creature is all colors." until end of turn.'), {
      kind: 'spell', effects: [{ steps: [{ effect: 'gain-abilities', what: source,
        abilities: [{ gained: 'quoted', ability: modify(source, [{ modifier: 'colors', operation: 'set', colors: ['white', 'blue', 'black', 'red', 'green'] }]) }],
        duration: { duration: 'end-of-turn' },
      }] }],
    });
  });
  it('rejects unrepresented names, copy definitions, exceptions, and duration/classifier shortcuts', () => {
    for (const text of [
      'Enchanted creature is a green Citizen creature with base power and toughness 1/1 named Legitimate Businessperson.',
      'Enchanted creature is an Insect artifact creature and loses all other abilities, card types, and creature types.',
      'Enchanted creature is a copy of the chosen creature.',
      'This creature is the chosen type.',
      'All creatures are the color of your choice.',
      'Enchanted creature is blue and.',
      'Enchanted creature is a Dinosaur in addition to its other colors.',
      'Target creature is a green Frog until end of turn.',
      'Enchanted creature is a Frog until end of turn.',
      'Enchanted land is a 2/2 creature. They are still lands.',
      "Lands you control are 2/2 creatures. It's still a land.",
      'Enchanted creature is also blue.',
    ]) {
      const cursor = new Cursor(lex(text));
      assert.equal(parseStatic(cursor), null, text);
      assert.equal(cursor.pos, 0, text);
      assert.equal(parseOracleText(text).ok, false, text);
    }
  });
});

describe('dynamic static scalar multipliers', () => {
  it('counts counters on the affected object rather than on the source', () => {
    for (const [text, what, counter] of [
      ['Equipped creature gets +1/+1 for each counter on it.', equipped, 'any'],
      ['Each Fungus creature gets +1/+1 for each spore counter on it.', { ref: 'each', filter: { subtypes: ['Fungus'], types: ['creature'] } }, { type: 'spore' }],
    ] as const) assert.deepEqual(one(text), modify(what, [{ ...pt(1, 1), multiplier: {
      amount: 'counter-count', counter, on: { ref: 'it' },
    }, valueScope: 'each-affected-object' }]));
    assert.deepEqual(one('Creatures you control get +1/+0 for each experience counter you have.'), modify({ ref: 'each', filter: { types: ['creature'], control: 'you' } }, [{
      ...pt(1, 0), multiplier: { amount: 'counter-count', counter: { type: 'experience' }, on: { player: 'you' } }, valueScope: 'each-affected-object',
    }]));
  });
  it('counts distinct basic land types for Domain and keeps other modifiers unscaled', () => {
    assert.deepEqual(one('Enchanted creature gets +1/+1 for each basic land type among lands you control and has trample.'), modify(enchanted, [{
      ...pt(1, 1), multiplier: { amount: 'distinct-count', characteristic: 'basic-land-type', among: { ref: 'each', filter: { types: ['land'], control: 'you' } } },
      valueScope: 'each-affected-object',
    }, abilities('trample')]));
    for (const text of [
      'This creature gets +1/+1 for each poison counter your opponents have.',
      'This creature gets +1/+1 for each poison counter on each opponent.',
      'This creature gets +1/+1 for each poison counter on each player with more life than you.',
      'Enchanted creature gets +1/+1 for each +1/+1 counter on other creatures you control.',
      'Creatures you control have flying for each color among permanents you control.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});
