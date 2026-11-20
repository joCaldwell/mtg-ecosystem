import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// "the power of [object]" is the of-form of "[object]'s power" (CR 107.2, 208).
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

describe('attribute amounts stated with "of"', () => {
  it('keeps the target identity inside the amount (Soul\'s Majesty)', () => {
    assert.deepEqual(one('Draw cards equal to the power of target creature you control.'), { kind: 'spell', effects: [{ steps: [{
      effect: 'draw', who: you, amount: { amount: 'attribute', attribute: 'power',
        of: { ref: 'target', filter: { types: ['creature'], control: 'you' }, count: n(1) } } }] }] });
  });
  it('produces the same amount as the possessive form', () => {
    const of = { ref: 'that', filter: { types: ['creature'] } };
    const expected = (attribute: string) => ({ kind: 'spell', effects: [{ steps: [{ effect: 'gain-life', who: you, amount: { amount: 'attribute', of, attribute } }] }] });
    assert.deepEqual(one('You gain life equal to the toughness of that creature.'), expected('toughness'));
    assert.deepEqual(one("You gain life equal to that creature's toughness."), expected('toughness'));
    assert.deepEqual(one('You gain life equal to the mana value of that creature.'), expected('mana-value'));
  });
  it('binds X to the mana value of an exile-event result (Ruinous Intrusion)', () => {
    assert.deepEqual(one('Exile target artifact or enchantment. Put X +1/+1 counters on target creature you control, where X is the mana value of the permanent exiled this way.'), {
      kind: 'spell', effects: [
        { steps: [{ effect: 'exile', what: { ref: 'target', filter: { allOf: [{ anyOf: [{ types: ['artifact'] }, { types: ['enchantment'] }] }] }, count: n(1) } }] },
        { steps: [{ effect: 'put-counters', on: { ref: 'target', filter: { types: ['creature'], control: 'you' }, count: n(1) },
          counter: { type: '+1/+1' }, count: { amount: 'x' } }],
          bindings: [{ variable: 'x', value: { amount: 'attribute', attribute: 'mana-value', of: { ref: 'related', number: 'singular',
            filter: { cls: 'permanent' }, relation: { relation: 'event-result', event: 'exile', scope: 'this-way' } } } }] },
      ],
    });
  });
  it('rejects plural, quantified, or unselected objects, which have no single value', () => {
    fails('Draw cards equal to the power of creatures you control.');
    fails('Draw cards equal to the power of each creature you control.');
    fails('Draw cards equal to the power of a creature you control.');
    fails('Draw cards equal to the power of up to one target creature.');
    fails('Draw cards equal to the power of those creatures.');
  });
});

it("strict characteristic comparisons exclude the exact identified creature", () => {
  const text = "Whenever another creature enters, its controller may draw a card if its toughness is less than each other creature's toughness.";
  const result = one(text, "Opaline Marshal");
  assert.deepEqual(result.effects[0].condition, {
    condition: "strict-attribute-extremum", what: {ref:"it"}, attribute:"toughness",
    extremum:"minimum", among:{types:["creature"]},
  });
  assert.equal(parseOracleText(text.replace("creature's toughness", "creature's power")).ok, false);
});
