import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 105.1–105.2: an object is zero or more of the five colors.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const n = (value: number) => ({ amount: 'fixed', value });
const colorsOf = (among: unknown) => ({ amount: 'distinct-count', characteristic: 'color', among });

describe('the number of colors one object is', () => {
  it('multiplies a static modifier per affected object (Civic Saber)', () => {
    assert.deepEqual(one('Equipped creature gets +1/+0 for each of its colors.'), { kind: 'static', effect: { static: 'modify',
      what: { ref: 'equipped', noun: 'creature' }, modifiers: [{ modifier: 'pt', power: { sign: 1, amount: n(1) }, toughness: { sign: 1, amount: n(0) },
        multiplier: colorsOf({ ref: 'it' }), valueScope: 'each-affected-object' }] } });
  });
  it('counts the colors of a possessive antecedent (Ramos, Dragon Engine)', () => {
    assert.deepEqual(one('Whenever you cast a spell, put a +1/+1 counter on Ramos for each of that spell\'s colors.', 'Ramos, Dragon Engine'), {
      kind: 'triggered', trigger: { trigger: 'cast', who: { player: 'you' }, what: { cls: 'spell' } }, effects: [{ steps: [{ effect: 'put-counters',
        on: { ref: 'self' }, counter: { type: '+1/+1' },
        count: { amount: 'product', factors: [n(1), colorsOf({ ref: 'that', filter: { cls: 'spell' } })] } }] }] });
  });
  it('reads "the number of colors that spell is" as the same amount (Mana Cannons)', () => {
    assert.deepEqual(one('Whenever you cast a multicolored spell, this enchantment deals X damage to any target, where X is the number of colors that spell is.'), {
      kind: 'triggered', trigger: { trigger: 'cast', who: { player: 'you' }, what: { multicolored: true, cls: 'spell' } },
      effects: [{ steps: [{ effect: 'damage', source: { ref: 'this', noun: 'enchantment' }, amount: { amount: 'x' }, to: { to: 'any-target' } }],
        bindings: [{ variable: 'x', value: colorsOf({ ref: 'that', filter: { cls: 'spell' } }) }] }] });
  });
  it('stays distinct from colors among a set of objects', () => {
    assert.deepEqual(one('You gain life equal to the number of colors among permanents you control.'), { kind: 'spell', effects: [{ steps: [{
      effect: 'gain-life', who: { player: 'you' }, amount: colorsOf({ ref: 'each', filter: { cls: 'permanent', control: 'you' } }) }] }] });
  });
  it('rejects plural possessors and unfinished wordings', () => {
    fails('You gain 1 life for each of their colors.');
    fails('You gain 1 life for each of its colors among permanents you control.');
    fails('Whenever you cast a spell, you gain X life, where X is the number of colors those spells are.');
    fails('Whenever you cast a spell, you gain X life, where X is the number of colors that spell.');
  });
});

// CR 702.16i: protection from each color of a set of colors.
describe('protection and hexproof from the colors of objects', () => {
  const grant = (keyword: string, what: unknown) => [{ gained: 'keyword', keyword: { keyword, from: [{ scope: 'colors-of', what }] } }];
  it('uses each affected object\'s own colors (Earnest Fellowship, Tam)', () => {
    assert.deepEqual(one('Each creature has protection from each of its colors.'), { kind: 'static', effect: { static: 'modify',
      what: { ref: 'each', filter: { types: ['creature'] } }, modifiers: [{ modifier: 'abilities', abilities: grant('protection', { ref: 'it' }) }] } });
    assert.deepEqual(one('Each other creature you control has hexproof from each of its colors.'), { kind: 'static', effect: { static: 'modify',
      what: { ref: 'each', filter: { other: true, types: ['creature'], control: 'you' } },
      modifiers: [{ modifier: 'abilities', abilities: grant('hexproof', { ref: 'it' }) }] } });
  });
  it('uses the colors among a set of objects (Empty-Shrine Kannushi)', () => {
    assert.deepEqual(one('This creature has protection from each color among permanents you control.'), { kind: 'static', effect: { static: 'modify',
      what: { ref: 'this', noun: 'creature' }, modifiers: [{ modifier: 'abilities',
        abilities: grant('protection', { ref: 'each', filter: { cls: 'permanent', control: 'you' } }) }] } });
  });
  it('uses a referenced object\'s colors in a resolving grant (Samite Elder)', () => {
    assert.deepEqual(one('{T}: Choose target permanent you control. Creatures you control gain protection from each of that permanent\'s colors until end of turn.').effects[1], {
      steps: [{ effect: 'gain-abilities', what: { ref: 'each', filter: { types: ['creature'], control: 'you' } },
        abilities: grant('protection', { ref: 'that', filter: { cls: 'permanent' } }), duration: { duration: 'end-of-turn' } }] });
  });
  it('keeps "each color" as all five colors and rejects other sets', () => {
    assert.deepEqual(one('This creature has protection from each color.').effect.modifiers, [{ modifier: 'abilities',
      abilities: [{ gained: 'keyword', keyword: { keyword: 'protection', from: [{ scope: 'all-colors' }] } }] }]);
    fails('This creature has protection from each color among a permanent you control.');
    fails('This creature has protection from each of their colors.');
    fails('This creature has protection from each of its card types.');
  });
});
