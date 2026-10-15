import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 205 (types, subtypes, supertypes), 607.2d (linked chosen type).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const n = (value: number) => ({ amount: 'fixed', value });
const spell = (...steps: object[]) => ({ kind: 'spell', effects: [{ steps }] });

describe('negated relative predicates', () => {
  it('excludes each listed subtype (Summon: Leviathan)', () => {
    assert.deepEqual(one("Return each creature that isn't a Kraken, Leviathan, Merfolk, Octopus, or Serpent to its owner's hand."),
      spell({ effect: 'return-to-hand', to: 'owner', what: { ref: 'each',
        filter: { types: ['creature'], nonSubtypes: ['Kraken', 'Leviathan', 'Merfolk', 'Octopus', 'Serpent'] } } }));
  });
  it('is the same filter as the non- prefix and the opposite of the positive clause', () => {
    assert.deepEqual(one("Destroy target creature that isn't a Demon."),
      spell({ effect: 'destroy', what: { ref: 'target', count: n(1), filter: { types: ['creature'], nonSubtypes: ['Demon'] } } }));
    assert.deepEqual(one("Destroy target creature that's a Demon."),
      spell({ effect: 'destroy', what: { ref: 'target', count: n(1), filter: { types: ['creature'], subtypes: ['Demon'] } } }));
  });
  it('negates a card type and a supertype, and keeps a following zone positive', () => {
    assert.deepEqual(one("Exile all nonland permanents that aren't legendary."),
      spell({ effect: 'exile', what: { ref: 'all', filter: { nonTypes: ['land'], cls: 'permanent', nonSupertypes: ['legendary'] } } }));
    assert.deepEqual(one("Return target enchantment card that isn't a God from your graveyard to your hand."),
      spell({ effect: 'return-to-hand', to: 'your', what: { ref: 'target', count: n(1),
        filter: { types: ['enchantment'], cls: 'card', nonSubtypes: ['God'], zone: { zone: 'graveyard', owner: 'your' } } } }));
  });
  it('records objects that lack the linked chosen type (Kindred Dominance)', () => {
    assert.deepEqual(one("Choose a creature type. Destroy all creatures that aren't of the chosen type."), { kind: 'spell', effects: [
      { steps: [{ effect: 'choose-value', who: { player: 'you' }, category: 'creature-type' }] },
      { steps: [{ effect: 'destroy', what: { ref: 'all', filter: { types: ['creature'], notChosenType: { choice: 'chosen-type' } } } }] },
    ] });
    assert.deepEqual(one('Choose a creature type. Destroy all creatures of the chosen type.').effects[1],
      { steps: [{ effect: 'destroy', what: { ref: 'all', filter: { types: ['creature'], chosenType: { choice: 'chosen-type' } } } }] });
  });
  it('rejects negations with no conjunctive form', () => {
    fails("Destroy each creature that isn't all colors.");
    fails("Destroy each permanent that isn't an artifact creature.");
    // A status without an antonym is represented as nonStatus.
    assert.deepEqual(one("Destroy each creature that isn't enchanted."),
      spell({ effect: 'destroy', what: { ref: 'each', filter: { types: ['creature'], nonStatus: ['enchanted'] } } }));
    fails("Destroy each creature that isn't red and white.");
    fails("Destroy all creatures that aren't of a type chosen this way.");
  });
});
