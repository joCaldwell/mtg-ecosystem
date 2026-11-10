import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 707.9a-d: exceptions to a copy effect are part of the copiable values.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const n = (value: number) => ({ amount: 'fixed', value });
const eot = { duration: 'end-of-turn' };
const target = (filter: object) => ({ ref: 'target', filter, count: n(1) });

describe('"becomes a copy of … except" exceptions', () => {
  it('keeps the duration printed before the exception and an additive type (Saheeli, Sublime Artificer)', () => {
    assert.deepEqual(one("-2: Target artifact you control becomes a copy of another target artifact or creature you control until end of turn, except it's an artifact in addition to its other types."), {
      kind: 'loyalty', cost: { sign: -1, amount: 2 }, effects: [{ steps: [{ effect: 'become', duration: eot,
        what: target({ types: ['artifact'], control: 'you' }),
        spec: { copyOf: target({ allOf: [{ anyOf: [{ types: ['artifact'] }, { types: ['creature'] }] }], control: 'you', other: true }),
          copyExceptions: [{ change: 'types', types: { cardTypes: { operation: 'add', types: ['artifact'] } } }] } }] }] });
  });
  it('sets power/toughness and keeps "this ability" as a reference to the enclosing ability (Volrath, the Shapestealer)', () => {
    assert.deepEqual(one("{1}: Until your next turn, ~ becomes a copy of target creature with a counter on it, except it's 7/5 and it has this ability."), {
      kind: 'activated', costs: [{ cost: 'mana', symbols: ['1'] }], effects: [{ steps: [{ effect: 'become', what: { ref: 'self' },
        duration: { duration: 'your-next-turn' },
        spec: { copyOf: target({ types: ['creature'], counterConstraints: [{ counter: 'any', comparison: { op: 'ge', value: n(1) } }] }),
          copyExceptions: [{ change: 'power-toughness', power: n(7), toughness: n(5) },
            { change: 'abilities', abilities: [{ gained: 'this-ability' }] }] } }] }] });
  });
  it('keeps a quoted ability and the optional causative (Cephalid Facetaker)', () => {
    const ability = one('At the beginning of combat on your turn, you may have this creature become a copy of another target creature until end of turn, except it\'s 1/4 and has "This creature can\'t be blocked."');
    assert.deepEqual(ability.effects, [{ steps: [{ effect: 'become', optional: true, duration: eot, what: { ref: 'this', noun: 'creature' },
      spec: { copyOf: target({ types: ['creature'], other: true }), copyExceptions: [
        { change: 'power-toughness', power: n(1), toughness: n(4) },
        { change: 'abilities', abilities: [{ gained: 'quoted', ability: { kind: 'static',
          effect: { static: 'cant', what: { ref: 'this', noun: 'creature' }, action: 'be-blocked' } } }] }] } }] }]);
  });
  it('leaves an exception-free copy unchanged and accepts plural and distributive subjects', () => {
    assert.deepEqual(one('{1}: This creature becomes a copy of target creature until end of turn.').effects, [{ steps: [{ effect: 'become',
      what: { ref: 'this', noun: 'creature' }, spec: { copyOf: target({ types: ['creature'] }) }, duration: eot }] }]);
    assert.deepEqual(one('Return target creature to its owner\'s hand. You may have Shapeshifters you control become copies of that creature until end of turn.').effects[1],
      { steps: [{ effect: 'become', optional: true, duration: eot, what: { ref: 'each', filter: { subtypes: ['Shapeshifter'], control: 'you' } },
        spec: { copyOf: { ref: 'that', filter: { types: ['creature'] } } } }] });
    assert.deepEqual(one('Choose a creature on the battlefield. Any number of target creatures you control each become a copy of that creature until end of turn.').effects[1],
      { steps: [{ effect: 'become', duration: eot, what: { ref: 'target', filter: { types: ['creature'], control: 'you' }, count: { amount: 'chosen', minimum: 0 } },
        spec: { copyOf: { ref: 'that', filter: { types: ['creature'] } } } }] });
  });
  it('rejects entry-only and unrepresented exceptions', () => {
    fails('{1}: This creature becomes a copy of target creature, except it enters with a +1/+1 counter on it.');
    fails("{1}: This creature becomes a copy of target creature, except it doesn't copy that creature's color.");
    fails('{1}: This creature becomes a copy of target creature, except its name is Bob.');
    fails('{1}: This creature becomes a copy of target creature, except.');
  });
});

describe('copies that keep the printed name of their own card', () => {
  it('records the name exception before the other exceptions (Sakashima the Impostor)', () => {
    const ability = one('You may have ~ enter as a copy of any creature on the battlefield, except its name is ~, it\'s legendary in addition to its other types, and it has flying.');
    assert.deepEqual(ability, { kind: 'static', effect: { static: 'entry-copy', what: { ref: 'self' }, optional: { who: { player: 'you' } },
      model: { model: 'choose', who: { player: 'you' }, filter: { types: ['creature'], zone: { zone: 'battlefield' } }, count: 1, when: 'before-entry' },
      changes: [{ change: 'name', name: 'source-printed-name' },
        { change: 'types', types: { supertypes: { operation: 'add', types: ['legendary'] } } },
        { change: 'abilities', abilities: [{ gained: 'keyword', keyword: { keyword: 'flying' } }] }] } });
  });
  it('accepts the self-possessive form and rejects other names (Chameleon, Master of Disguise)', () => {
    assert.deepEqual(one("You may have ~ enter as a copy of a creature you control, except ~'s name is ~.").effect.changes, [{ change: 'name', name: 'source-printed-name' }]);
    fails('You may have ~ enter as a copy of a creature you control, except its name is Bob.');
    fails('You may have ~ enter as a copy of a creature you control, except its name is.');
  });
});

describe('token copies that name the token in the exception', () => {
  it('treats "the token isn\'t legendary" as the not-legendary exception (Miirym, Sentinel Wyrm)', () => {
    const expected = (text: string) => assert.deepEqual(one(text).effects, [{ steps: [{ effect: 'create-token', count: n(1),
      token: { copyOf: { ref: 'it' }, modifications: [{ modification: 'not-legendary' }] } }] }]);
    expected("Whenever another nontoken Dragon you control enters, create a token that's a copy of it, except the token isn't legendary.");
    expected("Whenever another nontoken Dragon you control enters, create a token that's a copy of it, except it isn't legendary.");
  });
});
