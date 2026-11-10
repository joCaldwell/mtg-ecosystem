import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 208.1 (power/toughness), 202.3 (mana value): comparative descriptions.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const n = (value: number) => ({ amount: 'fixed', value });

describe('comparatives with an unstated standard', () => {
  it('keeps the standard symbolic in a blocking restriction (Ant-Man)', () => {
    assert.deepEqual(one('This creature can\'t be blocked by creatures with greater power.'), { kind: 'static', effect: { static: 'combat-rule',
      what: { ref: 'this', noun: 'creature' }, rule: { rule: 'blocker-filter', mode: 'exclude',
        blockers: { ref: 'each', filter: { types: ['creature'], implicitComparison: { attribute: 'power', op: 'gt' } } } } } });
  });
  it('describes targets with lesser toughness or mana value (Colfenor, Havi)', () => {
    assert.deepEqual(one('Destroy target creature with lesser toughness.').effects[0].steps[0].what, { ref: 'target', count: n(1),
      filter: { types: ['creature'], implicitComparison: { attribute: 'toughness', op: 'lt' } } });
    assert.deepEqual(one('Return target creature card with lesser mana value from your graveyard to your hand.').effects[0].steps[0].what, { ref: 'target', count: n(1),
      filter: { types: ['creature'], cls: 'card', implicitComparison: { attribute: 'mana-value', op: 'lt' }, zone: { zone: 'graveyard', owner: 'your' } } });
  });
  it('uses the stated object\'s attribute with "than"', () => {
    assert.deepEqual(one('Destroy target creature with lesser power than this creature.').effects[0].steps[0].what.filter, { types: ['creature'],
      properties: [{ property: 'power', comparison: { op: 'lt', value: { amount: 'attribute', of: { ref: 'this', noun: 'creature' }, attribute: 'power' } } }] });
  });
  it('differs from a fixed comparison', () => {
    assert.deepEqual(one('Destroy target creature with power 2 or less.').effects[0].steps[0].what.filter, { types: ['creature'],
      properties: [{ property: 'power', comparison: { op: 'le', value: n(2) } }] });
  });
  it('rejects compound and plural standards', () => {
    fails('Destroy target creature with greater power or toughness.');
    fails('Destroy target creature with greater power and toughness.');
    fails('Destroy target creature with lesser power than those creatures.');
    fails('Destroy target creature with lesser loyalty.');
  });
});

// Pack tactics (ability word): a total over the creatures attacked with.
describe('attacked with creatures with total power N', () => {
  it('constrains the attacking group, not each attacker (Gnoll Hunter)', () => {
    assert.deepEqual(one('Whenever this creature attacks, if you attacked with creatures with total power 6 or greater this combat, put a +1/+1 counter on this creature.').condition, {
      condition: 'attacked', who: { player: 'you' }, during: 'this-combat', with: { types: ['creature'] },
      groupConstraints: [{ constraint: 'attribute-sum', attribute: 'power', comparison: { op: 'ge', value: n(6) } }] });
  });
  it('differs from attacking with one creature with that power', () => {
    assert.deepEqual(one('Whenever this creature attacks, if you attacked with a creature with power 6 or greater this combat, put a +1/+1 counter on this creature.').condition, {
      condition: 'attacked', who: { player: 'you' }, during: 'this-combat',
      with: { types: ['creature'], properties: [{ property: 'power', comparison: { op: 'ge', value: n(6) } }] } });
  });
  it('rejects other group constraints and negation', () => {
    fails('Whenever this creature attacks, if you attacked with creatures with different powers this combat, draw a card.');
    fails('Whenever this creature attacks, if you didn\'t attack with creatures with total power 6 or greater this combat, draw a card.');
  });
});
