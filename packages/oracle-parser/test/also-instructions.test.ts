import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// "Also" marks one more instruction after the earlier ones; instructions are
// followed in order (CR 608.2c), so it adds no operand.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const fixed = (value: number) => ({ amount: 'fixed', value });
const targetCreature = { ref: 'target', filter: { types: ['creature'] }, count: fixed(1) };
const thatCreature = { ref: 'that', filter: { types: ['creature'] } };

describe('"also" before an additional instruction', () => {
  it('reads "~ also deals" as an ordinary conditional damage instruction (Misthios\'s Fury)', () => {
    const text = "~ deals 3 damage to target creature. If you control an Equipment, ~ also deals 2 damage to that creature's controller.";
    const expected = { kind: 'spell', effects: [
      { steps: [{ effect: 'damage', source: { ref: 'self' }, amount: fixed(3), to: { to: 'object', ref: targetCreature } }] },
      { steps: [{ effect: 'damage', source: { ref: 'self' }, amount: fixed(2), to: { to: 'player', ref: { player: 'controller', of: thatCreature } } }],
        condition: { condition: 'control', who: { player: 'you' }, what: { ref: 'filter', filter: { types: ['artifact'], subtypes: ['Equipment'] }, count: fixed(1) } }, conditionKind: 'if' },
    ] };
    const actual = one(text, "Misthios's Fury");
    assert.deepEqual(actual, one(text.replace(' also', ''), "Misthios's Fury"));
    assert.equal(actual.effects.length, 2);
    assert.deepEqual(actual.effects[0], expected.effects[0]);
    assert.deepEqual(actual.effects[1].steps, expected.effects[1].steps);
  });
  it('reads "each opponent also discards" with the stated actor (Liliana\'s Triumph)', () => {
    const text = 'Each opponent sacrifices a creature of their choice. If you control a Liliana planeswalker, each opponent also discards a card.';
    const actual = one(text);
    assert.deepEqual(actual, one(text.replace(' also', '')));
    assert.deepEqual(actual.effects[1].steps, [{ effect: 'discard', who: { player: 'each-opponent' }, what: { ref: 'filter', filter: { cls: 'card' }, count: fixed(1) } }]);
  });
  it('reads imperative "also create" and "it also gets"', () => {
    const create = 'Create X 2/2 white Soldier creature tokens with vigilance. If X is 10 or more, also create X 4/4 white Angel creature tokens with flying and vigilance.';
    assert.deepEqual(one(create), one(create.replace(' also', '')));
    const gets = 'Target creature gains haste until end of turn. If that creature is a Goat, it also gets +3/+0 until end of turn.';
    const actual = one(gets);
    assert.deepEqual(actual, one(gets.replace(' also', '')));
    assert.deepEqual(actual.effects[1].steps, [{ effect: 'pump', what: { ref: 'it' }, power: { sign: 1, amount: fixed(3) }, toughness: { sign: 1, amount: fixed(0) }, duration: { duration: 'end-of-turn' } }]);
  });
  it('rejects "also" outside the subject-verb position', () => {
    fails("~ deals also 2 damage to target creature.");
    fails("~ deals 2 damage to target creature also.");
    fails("Each opponent discards also a card.");
  });
});

describe('"also" without an earlier instruction', () => {
  it('rejects new "also" verbs at the start of the text', () => {
    fails('~ also deals 2 damage to target creature.');
    fails('Each opponent also discards a card.');
    fails('Also create a 1/1 white Soldier creature token.');
  });
});
