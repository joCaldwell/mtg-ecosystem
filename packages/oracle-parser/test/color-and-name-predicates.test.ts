import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { buildCardNameCatalog, parseOracleText } from '../src/index.ts';

// CR 105.1–105.2 (colors, colorless), 201.2 (names), 111.3 (token characteristics).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const cardNames = buildCardNameCatalog([{ name: 'Detention Sphere', layout: 'normal' }, { name: 'Squadron Hawk', layout: 'normal' }]);
function one(text: string, name?: string) {
  const result = parseOracleText(text, name, { cardNames });
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name, { cardNames }).ok, false, text);
const n = (value: number) => ({ amount: 'fixed', value });
const allColors = ['white', 'blue', 'black', 'red', 'green'];

describe('"one or more colors" and "all colors" predicates', () => {
  it('selects every object that is not colorless (Reach of Shadows, All Is Dust)', () => {
    assert.deepEqual(one('Destroy target creature that\'s one or more colors.'), { kind: 'spell', effects: [{ steps: [{ effect: 'destroy',
      what: { ref: 'target', count: n(1), filter: { types: ['creature'], colorless: false } } }] }] });
    assert.deepEqual(one('Each player sacrifices all permanents they control that are one or more colors.'), { kind: 'spell', effects: [{ steps: [{
      effect: 'sacrifice', who: { player: 'each-player' },
      what: { ref: 'all', filter: { cls: 'permanent', controlledBy: { player: 'they' }, colorless: false } } }] }] });
  });
  it('differs from colorless and from a listed color', () => {
    assert.deepEqual(one('Destroy target colorless creature.').effects[0].steps[0].what.filter, { colorless: true, types: ['creature'] });
    assert.deepEqual(one('Destroy target creature that\'s red.').effects[0].steps[0].what.filter, { types: ['creature'], colors: ['red'] });
  });
  it('requires all five colors for "all colors" (Jared Carthalion)', () => {
    assert.deepEqual(one('Destroy target creature. If that creature was all colors, draw a card.').effects[1], {
      steps: [{ effect: 'draw', who: { player: 'you' }, amount: n(1) }], conditionKind: 'if',
      condition: { condition: 'is-filter', what: { ref: 'that', filter: { types: ['creature'] } }, filter: { colors: allColors }, time: 'previous' } });
  });
  it('does not state a negated conjunction of colors', () => {
    fails('Destroy each creature that isn\'t all colors.');
    fails('Destroy target creature that\'s one or more.');
    fails('Destroy target creature that\'s two or more colors.');
  });
});

describe('tokens that are all colors', () => {
  it('gives the token all five colors (The Wandering Minstrel)', () => {
    assert.deepEqual(one('Create a 2/2 Elemental creature token that\'s all colors.'), { kind: 'spell', effects: [{ steps: [{ effect: 'create-token',
      count: n(1), token: { colors: allColors, supertypes: [], types: ['creature'], subtypes: ['Elemental'], keywords: [], power: n(2), toughness: n(2) } }] }] });
    assert.deepEqual(one('Create a 3/3 Kavu creature token with trample that\'s all colors.').effects[0].steps[0].token, {
      colors: allColors, supertypes: [], types: ['creature'], subtypes: ['Kavu'], keywords: [{ keyword: 'trample' }], power: n(3), toughness: n(3) });
  });
  it('rejects a contradiction with stated colors', () => {
    fails('Create a 2/2 red Elemental creature token that\'s all colors.');
    fails('Create a 2/2 colorless Elemental creature token that\'s all colors.');
  });
});

describe('"not named" filters', () => {
  it('excludes the literal name (Detention Sphere)', () => {
    assert.deepEqual(one('Exile target nonland permanent not named Detention Sphere.', 'Detention Sphere'), { kind: 'spell', effects: [{ steps: [{
      effect: 'exile', what: { ref: 'target', count: n(1), filter: { nonTypes: ['land'], cls: 'permanent',
        notName: { name: 'literal', value: 'Detention Sphere' } } } }] }] });
  });
  it('differs from the positive name test', () => {
    assert.deepEqual(one('Exile target creature named Squadron Hawk.').effects[0].steps[0].what.filter, {
      types: ['creature'], name: { name: 'literal', value: 'Squadron Hawk' } });
    assert.deepEqual(one('Exile target creature not named Squadron Hawk.').effects[0].steps[0].what.filter, {
      types: ['creature'], notName: { name: 'literal', value: 'Squadron Hawk' } });
  });
  it('rejects names absent from the catalog', () => {
    fails('Exile target creature not named Nonexistent Thing.');
  });
});
