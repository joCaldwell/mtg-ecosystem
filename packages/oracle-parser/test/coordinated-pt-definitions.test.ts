import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 604.3 (characteristic-defining abilities), 613.4a.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const self = { ref: 'self' };

describe('coordinated power and toughness definitions', () => {
  it('repeats the stated power number in "that number plus 1" (Tarmogoyf)', () => {
    const types = { amount: 'distinct-count', characteristic: 'card-type',
      among: { ref: 'each', filter: { cls: 'card', zone: { zone: 'graveyard', owner: 'all-players' } } } };
    assert.deepEqual(one("~'s power is equal to the number of card types among cards in all graveyards and its toughness is equal to that number plus 1."), {
      kind: 'static', effect: { static: 'group', effects: [
        { static: 'cda-pt', what: self, stat: 'power', value: types },
        { static: 'cda-pt', what: self, stat: 'toughness', value: { amount: 'plus', a: types, b: { amount: 'fixed', value: 1 } } },
      ] },
    });
  });
  it('keeps the ability word and a counted filter (Souls of the Lost)', () => {
    const count = { amount: 'count', of: { cls: 'card', permanent: true, zone: { zone: 'graveyard', owner: 'your' } } };
    assert.deepEqual(one("Fathomless descent — ~'s power is equal to the number of permanent cards in your graveyard and its toughness is equal to that number plus 1."), {
      kind: 'static', abilityWord: 'Fathomless descent', effect: { static: 'group', effects: [
        { static: 'cda-pt', what: self, stat: 'power', value: count },
        { static: 'cda-pt', what: self, stat: 'toughness', value: { amount: 'plus', a: count, b: { amount: 'fixed', value: 1 } } },
      ] },
    });
  });
  it("defines only the power of a Vehicle (Lumbering Worldwagon)", () => {
    assert.deepEqual(one("This Vehicle's power is equal to the number of lands you control."), {
      kind: 'static', effect: { static: 'cda-pt', what: { ref: 'this', noun: 'vehicle' }, stat: 'power',
        value: { amount: 'count', of: { types: ['land'], control: 'you' } } },
    });
  });
  it('rejects a second value it cannot state and a toughness-first coordination', () => {
    // "their" is the same exiled cards (Sutured Ghoul).
    assert.equal(parseOracleText("~'s power is equal to the total power of the exiled cards and its toughness is equal to their total toughness.", "Fixture", { typeLine: "Creature — Horror" }).ok, true);
    fails("~'s toughness is equal to the number of lands you control and its toughness is equal to that number plus 1.");
    fails("~'s power is equal to the number of lands you control and its toughness is equal to that number plus 1 and it has flying.");
  });
});
