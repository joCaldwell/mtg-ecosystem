import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 205.1a (a set card type replaces the others), 205.1b, 613.1f.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const enchanted = { ref: 'enchanted', noun: 'permanent' };
const modify = (modifiers: object[]) => ({ kind: 'static', effect: { static: 'modify', what: enchanted, modifiers } });

describe('"loses all other card types" riders', () => {
  it('states the same replacement as setting the card type (One with the Stars)', () => {
    const expected = modify([{ modifier: 'types', change: { cardTypes: { operation: 'set', types: ['enchantment'] } } }]);
    assert.deepEqual(one('Enchanted permanent is an enchantment and loses all other card types.'), expected);
    assert.deepEqual(one('Enchanted permanent is an enchantment.'), expected);
  });
  it('accepts the "and it loses" form after a granted keyword (Swift Reconfiguration)', () => {
    assert.deepEqual(one('Enchanted permanent is a Vehicle artifact with crew 5 and it loses all other card types.'), modify([
      { modifier: 'types', change: { cardTypes: { operation: 'set', types: ['artifact'] },
        subtypes: [{ family: 'artifact', operation: 'set', types: ['Vehicle'] }] } },
      { modifier: 'abilities', abilities: [{ gained: 'keyword', keyword: { keyword: 'crew', amount: 5 } }] },
    ]));
  });
  it('removes every ability the effect does not grant for "…and abilities" (Imprisoned in the Moon)', () => {
    assert.deepEqual(one('Enchanted permanent is a colorless land with "{T}: Add {C}" and loses all other card types and abilities.'), modify([
      { modifier: 'types', change: { cardTypes: { operation: 'set', types: ['land'] } } },
      { modifier: 'colors', operation: 'set', colors: [] },
      { modifier: 'abilities', abilities: [{ gained: 'quoted', ability: { kind: 'activated', costs: [{ cost: 'tap-self' }],
        effects: [{ steps: [{ effect: 'add-mana', who: { player: 'you' }, mana: { mana: 'fixed', symbols: ['C'] } }] }] } }] },
      { modifier: 'lose-abilities', removal: { removal: 'all-other', than: 'abilities-granted-by-this-effect' } },
    ]));
  });
  it('applies the rider to a stated resolving characteristic (Vraska, the Silencer)', () => {
    const result = one('Return target creature card from your graveyard to the battlefield. It\'s a Treasure artifact with "{T}, Sacrifice this artifact: Add one mana of any color," and it loses all other card types.');
    assert.deepEqual(result.effects[1], { steps: [{ effect: 'modify-characteristics', what: { ref: 'it' }, snapshot: 'effect-created', statedAs: 'is', modifiers: [
      { modifier: 'types', change: { cardTypes: { operation: 'set', types: ['artifact'] }, subtypes: [{ family: 'artifact', operation: 'set', types: ['Treasure'] }] } },
      { modifier: 'abilities', abilities: [{ gained: 'quoted', ability: { kind: 'activated', costs: [{ cost: 'tap-self' }, { cost: 'sacrifice', what: { ref: 'this', noun: 'artifact' } }],
        effects: [{ steps: [{ effect: 'add-mana', who: { player: 'you' }, mana: { mana: 'any-color', amount: { amount: 'fixed', value: 1 } } }] }] } }] },
    ] }] });
  });
  it('rejects the rider where it contradicts or lacks its card type, or has no granted ability', () => {
    fails('Enchanted permanent is an enchantment in addition to its other types and loses all other card types.');
    fails('Enchanted permanent is an artifact creature and loses all other card types.');
    fails('Enchanted permanent is a land and loses all other card types and abilities.');
    fails('Enchanted permanent is blue and loses all other card types.');
    fails('Enchanted permanent loses all other card types.');
    fails('Enchanted creature is an Insect artifact creature and loses all other abilities, card types, and creature types.');
  });
});
