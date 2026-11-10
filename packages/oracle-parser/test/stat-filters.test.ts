import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 208 (power/toughness), 201.2b (different names).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };

describe('"power or toughness N" filters', () => {
  it('accepts either value under the one comparison (Repel Calamity)', () => {
    const comparison = { op: 'ge', value: n(4) };
    assert.deepEqual(one('Destroy target creature with power or toughness 4 or greater.'), { kind: 'spell', effects: [{ steps: [{ effect: 'destroy',
      what: { ref: 'target', count: n(1), filter: { types: ['creature'], anyOf: [
        { properties: [{ property: 'power', comparison }] }, { properties: [{ property: 'toughness', comparison }] }] } } }] }] });
  });
  it('differs from a single-stat constraint', () => {
    assert.deepEqual(one('Destroy target creature with power 4 or greater.'), { kind: 'spell', effects: [{ steps: [{ effect: 'destroy',
      what: { ref: 'target', count: n(1), filter: { types: ['creature'], properties: [{ property: 'power', comparison: { op: 'ge', value: n(4) } }] } } }] }] });
  });
  it('rejects incomplete or mixed attribute alternatives', () => {
    fails('Destroy target creature with power or toughness.');
    fails('Destroy target creature with power or mana value 4 or greater.');
    // Base power/toughness is represented (CR 208.4b), so this is a
    // positive parse distinct from the current-value alternative above.
    assert.deepEqual(one('Destroy target creature with base power or toughness 1.'), { kind: 'spell', effects: [{ steps: [{ effect: 'destroy',
      what: { ref: 'target', count: n(1), filter: { types: ['creature'], anyOf: [
        { properties: [{ property: 'base-power', comparison: { op: 'eq', value: n(1) } }] },
        { properties: [{ property: 'base-toughness', comparison: { op: 'eq', value: n(1) } }] }] } } }] }] });
  });
});

describe('distinct names and powers', () => {
  it('counts differently named objects, not objects (Christine Chapel)', () => {
    const lands = { types: ['land'], control: 'you' };
    assert.deepEqual(one('You gain life equal to the number of differently named lands you control.'), { kind: 'spell', effects: [{ steps: [{
      effect: 'gain-life', who: you, amount: { amount: 'distinct-count', characteristic: 'name', among: { ref: 'each', filter: lands } } }] }] });
    assert.deepEqual(one('You gain life equal to the number of lands you control.'), { kind: 'spell', effects: [{ steps: [{
      effect: 'gain-life', who: you, amount: { amount: 'count', of: lands } }] }] });
  });
  it('scales one event by the number of different powers (Golden Ratio)', () => {
    assert.deepEqual(one('Draw a card for each different power among creatures you control.'), { kind: 'spell', effects: [{ steps: [{
      effect: 'draw', who: you, amount: { amount: 'product', factors: [n(1),
        { amount: 'distinct-count', characteristic: 'power', among: { ref: 'each', filter: { types: ['creature'], control: 'you' } } }] } }] }] });
  });
  it('rejects a differently-named phrase without a counted set', () => {
    fails('You gain life equal to the number of differently named.');
    fails('You gain life equal to the number of differently named target lands.');
  });
});
