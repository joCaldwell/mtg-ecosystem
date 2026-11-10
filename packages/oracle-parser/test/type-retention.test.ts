import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 205.1b: "still a [type]" retains every prior card type and subtype.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const land = { ref: 'target', filter: { types: ['land'], control: 'you' }, count: { amount: 'fixed', value: 1 } };
const x = { amount: 'x' };
const becomes = (operation: string) => ({ effect: 'modify-characteristics', what: land, snapshot: 'effect-created', duration: { duration: 'end-of-turn' }, modifiers: [
  { modifier: 'types', change: { cardTypes: { operation, types: ['creature'] }, subtypes: [{ family: 'creature', operation, types: ['Elemental'] }] } },
  { modifier: 'base-pt', power: x, toughness: x, valueScope: 'each-affected-object' },
] });
const bindings = [{ variable: 'x', value: { amount: 'count', of: { subtypes: ['Ally'], control: 'you' } } }];

describe('type retention stated after a where-X definition', () => {
  it('turns the type setting into an addition (Vastwood Animist)', () => {
    assert.deepEqual(one("{T}: Target land you control becomes an X/X Elemental creature until end of turn, where X is the number of Allies you control. It's still a land."), {
      kind: 'activated', costs: [{ cost: 'tap-self' }], effects: [{ steps: [becomes('add')], bindings }] });
  });
  it('keeps replacement when no retention sentence follows', () => {
    assert.deepEqual(one('{T}: Target land you control becomes an X/X Elemental creature until end of turn, where X is the number of Allies you control.'), {
      kind: 'activated', costs: [{ cost: 'tap-self' }], effects: [{ steps: [becomes('set')], bindings }] });
  });
  it('rejects a mismatched pronoun, a non-type noun, and retention after an unrelated instruction', () => {
    fails("Target land you control becomes an X/X Elemental creature until end of turn, where X is the number of Allies you control. They're still lands.");
    fails("Target land you control becomes an X/X Elemental creature until end of turn, where X is the number of Allies you control. It's still tapped.");
    fails("Draw X cards, where X is the number of Allies you control. It's still a land.");
    fails("Target land you control becomes an X/X Elemental creature until end of turn, where X is the number of Allies you control. It's still a land and gains haste.");
  });
});
