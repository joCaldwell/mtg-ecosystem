import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 107.2: "the creature with the greatest power or tied for the greatest
// power" is any creature whose power equals the greatest power among all
// creatures.
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const fixed = (value: number) => ({ amount: 'fixed', value });
const controlsTop = (attribute: string, extremum = 'maximum') => ({ condition: 'control', who: { player: 'you' }, what: { ref: 'filter', count: fixed(1), filter: {
  types: ['creature'], properties: [{ property: attribute, comparison: { op: 'eq', value: { amount: 'attribute-extremum', extremum, attribute, among: { types: ['creature'] } } } }] } } });

describe('"the creature with the greatest X or tied for the greatest X"', () => {
  it('reads the control condition as controlling a creature with the extreme value (Triumph of Ferocity)', () => {
    assert.deepEqual(one('At the beginning of your upkeep, draw a card if you control the creature with the greatest power or tied for the greatest power.'), {
      kind: 'triggered', trigger: { trigger: 'phase', phase: 'upkeep', whose: 'your' },
      effects: [{ steps: [{ effect: 'draw', who: { player: 'you' }, amount: fixed(1) }], condition: controlsTop('power'), conditionKind: 'if' }] });
  });
  it('reads toughness and least the same way', () => {
    assert.deepEqual(one('Draw a card if you control the creature with the greatest toughness or tied for the greatest toughness.').effects[0].condition, controlsTop('toughness'));
    assert.deepEqual(one('Draw a card if you control the creature with the least power or tied for the least power.').effects[0].condition, controlsTop('power', 'minimum'));
  });
  it('rejects a definite description without the tie clause and mismatched tie clauses', () => {
    fails('Draw a card if you control the creature with the greatest power.');
    fails('Draw a card if you control the creature with the greatest power or tied for the greatest toughness.');
    fails('Draw a card if you control the creature with the greatest power or tied for the least power.');
    fails('Draw a card if you control the artifact creature with the greatest power or tied for the greatest power.');
  });
});
