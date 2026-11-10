import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { Ability } from '../src/ast.ts';
import { lex } from '../src/lexer.ts';
import { parseOracleText } from '../src/index.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { parseEntryCopy } from '../src/parser/entry-copy.ts';
import { parseGainedAbilities } from '../src/parser/effects.ts';
import { finalizeCharacteristicDefinitions } from '../src/parser/characteristic-origin.ts';
import { analyzeCardBindings } from '../src/semantic/bindings.ts';

// Oracle clauses checked in the pinned corpus: Clone, Body Double, Vesuva,
// Echoing Deeps, Metamorph, Effigy, Quicksilver Gargantuan, Auton Soldier,
// Altered Ego, Cursed Mirror, Essence of the Wild, and Lazotep Convert.
// CR614.12,707.2,707.5,707.9:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you = { player: 'you' };
const creature = { ref: 'this', noun: 'creature' };
const n = (value: number) => ({ amount: 'fixed', value });
const battlefield = { types: ['creature'], zone: { zone: 'battlefield' } };
const graveyard = { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'any' } };
const base = (filter: object = battlefield, what: object = creature) => ({ static: 'entry-copy', what,
  model: { model: 'choose', who: you, filter, count: 1, when: 'before-entry' }, optional: { who: you } });
const keyword = (keyword: string) => ({ gained: 'keyword', keyword: { keyword } });
const start = 'You may have this creature enter as a copy of any creature on the battlefield';
function one(text: string, typeLine?: string) {
  const result = parseOracleText(text, '', { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const staticAbility = (effect: object) => ({ kind: 'static', effect });

describe('entry copy replacements', () => {
  it('chooses one model before entry without targeting and retains exact eligibility', () => {
    assert.deepEqual(one(start + '.', 'Creature — Shapeshifter'), staticAbility(base()));
    assert.deepEqual(one('You may have this creature enter as a copy of any creature card in a graveyard.'), staticAbility(base(graveyard)));
    assert.deepEqual(one('You may have this creature enter as a copy of a creature you control.'), staticAbility(base({ types: ['creature'], control: 'you' })));
    assert.deepEqual(one('You may have this creature enter as a copy of a creature card in exile with a takeover counter on it.'), staticAbility(base({
      types: ['creature'], cls: 'card', zone: { zone: 'exile' }, counterConstraints: [{ counter: { type: 'takeover' }, comparison: { op: 'ge', value: n(1) } }],
    })));
  });
  it('keeps entering tapped separate from copiable exceptions and additional counters', () => {
    const land = { ref: 'this', noun: 'land' };
    assert.deepEqual(one('You may have this land enter tapped as a copy of any land on the battlefield.'), staticAbility({
      ...base({ types: ['land'], zone: { zone: 'battlefield' } }, land), entry: { tapped: true },
    }));
    assert.deepEqual(one("You may have this land enter tapped as a copy of any land card in a graveyard, except it's a Cave in addition to its other types."), staticAbility({
      ...base({ types: ['land'], cls: 'card', zone: { zone: 'graveyard', owner: 'any' } }, land), entry: { tapped: true },
      changes: [{ change: 'types', types: { subtypes: [{ family: 'land', operation: 'add', types: ['Cave'] }] } }],
    }));
    assert.deepEqual(one(start + ', except it enters with X additional +1/+1 counters on it.'), staticAbility({
      ...base(), entryExceptions: [{ exception: 'counters', count: { amount: 'x' }, counter: { type: '+1/+1' }, additional: true }],
    }));
  });
  it('distinguishes type addition, type replacement, subtype addition and P/T setting', () => {
    assert.deepEqual(one("You may have this creature enter as a copy of any artifact or creature on the battlefield, except it's an artifact in addition to its other types."), staticAbility({
      ...base({ allOf: [{ anyOf: [{ types: ['artifact'] }, { types: ['creature'] }] }], zone: { zone: 'battlefield' } }),
      changes: [{ change: 'types', types: { cardTypes: { operation: 'add', types: ['artifact'] } } }],
    }));
    assert.deepEqual(one(start + ", except it's 7/7."), staticAbility({ ...base(), changes: [{ change: 'power-toughness', power: n(7), toughness: n(7) }] }));
    assert.deepEqual(one("You may have this creature enter as a copy of a creature you control, except it's a Shapeshifter Rogue in addition to its other types."), staticAbility({
      ...base({ types: ['creature'], control: 'you' }), changes: [{ change: 'types', types: { subtypes: [{ family: 'creature', operation: 'add', types: ['Shapeshifter', 'Rogue'] }] } }],
    }));
    assert.deepEqual(one("You may have this creature enter as a copy of any creature card in a graveyard, except it's a 4/4 black Zombie in addition to its other colors and types."), staticAbility({
      ...base(graveyard), changes: [{ change: 'power-toughness', power: n(4), toughness: n(4) },
        { change: 'types', types: { subtypes: [{ family: 'creature', operation: 'add', types: ['Zombie'] }] } },
        { change: 'colors', operation: 'add', colors: ['black'] }],
    }));
  });
  it('stores keyword and complete quoted ability exceptions as copiable characteristics', () => {
    assert.deepEqual(one(start + ", except it isn't legendary, is an artifact in addition to its other types, and has myriad."), staticAbility({
      ...base(), changes: [{ change: 'types', types: { supertypes: { operation: 'remove', types: ['legendary'] } } },
        { change: 'types', types: { cardTypes: { operation: 'add', types: ['artifact'] } } },
        { change: 'abilities', abilities: [keyword('myriad')] }],
    }));
    assert.deepEqual(one('You may have this artifact enter as a copy of any creature on the battlefield, except it\'s an artifact and it has "{T}: Add {U}."'), staticAbility({
      ...base(battlefield, { ref: 'this', noun: 'artifact' }), changes: [
        { change: 'types', types: { cardTypes: { operation: 'set', types: ['artifact'] } } },
        { change: 'abilities', abilities: [{ gained: 'quoted', ability: { kind: 'activated', costs: [{ cost: 'tap-self' }], effects: [{ steps: [
          { effect: 'add-mana', who: you, mana: { mana: 'fixed', symbols: ['U'] } },
        ] }] } }] },
      ],
    }));
  });
  it('preserves Cursed Mirror copy and haste as one duration-limited copy effect', () => {
    assert.deepEqual(one('As this artifact enters, you may have it become a copy of any creature on the battlefield until end of turn, except it has haste.'), staticAbility({
      ...base(battlefield, { ref: 'this', noun: 'artifact' }), duration: { duration: 'end-of-turn' }, changes: [{ change: 'abilities', abilities: [keyword('haste')] }],
    }));
  });
  it('keeps external source and entering permanent queries separate', () => {
    assert.deepEqual(one('Creatures you control enter as a copy of this creature.'), staticAbility({
      static: 'entry-copy', what: { ref: 'each', filter: { types: ['creature'], control: 'you' } }, model: { model: 'reference', what: creature },
    }));
    assert.deepEqual(one('Nontoken creatures you control enter as a copy of enchanted creature.'), staticAbility({
      static: 'entry-copy', what: { ref: 'each', filter: { nonToken: true, types: ['creature'], control: 'you' } }, model: { model: 'reference', what: { ref: 'enchanted', noun: 'creature' } },
    }));
  });
  it('preserves copy-defined CDAs and leaves ordinary grants in layer 7b without mutating input', () => {
    const definition = 'This creature\'s power and toughness are each equal to the number of lands you control.';
    const value = { amount: 'count', of: { types: ['land'], control: 'you' } };
    const copy = one(start + `, except it has "${definition}"`);
    assert.deepEqual(copy, staticAbility({ ...base(), changes: [{ change: 'abilities', abilities: [{ gained: 'quoted', ability: {
      kind: 'static', effect: { static: 'cda-pt', what: creature, stat: 'both', value },
    } }] }] }));
    assert.deepEqual(one(`This creature gains "${definition}" until end of turn.`), { kind: 'spell', effects: [{ steps: [{
      effect: 'gain-abilities', what: creature, abilities: [{ gained: 'quoted', ability: { kind: 'static', effect: {
        static: 'modify', what: creature, modifiers: [{ modifier: 'base-pt', power: value, toughness: value }],
      } } }], duration: { duration: 'end-of-turn' },
    }] }] });
    const before = structuredClone(copy);
    assert.deepEqual(finalizeCharacteristicDefinitions(copy as Ability), copy);
    assert.deepEqual(copy, before);
  });
  it('rejects unrepresented conditional, retained, multiple-model and linked-rider semantics', () => {
    for (const text of [
      start + ', except it has vanishing 3 if that creature doesn\'t have vanishing.',
      start + ", except it enters with an additional +1/+1 counter on it if it's a creature.",
      start + ', except its power and toughness are equal to this creature\'s power and toughness.',
      start + ', except it has its other abilities.',
      start + ', except it has protection from the color of your choice.',
      start + ", except it's a Wall in addition to its other types and has defender. When you do, tap the copied creature.",
      'You may have this creature enter as a copy of two creatures on the battlefield.',
      'You may have this creature enter as a copy of target creature.',
      start + ', except it has flying and a mystery ability.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
    // The card's own move into that graveyard is now a represented filter
    // predicate (ObjectFilter.zoneChangeHistory), so this model is complete.
    assert.deepEqual(one('You may have this creature enter as a copy of any creature card in a graveyard that was put there from the battlefield this turn.'), staticAbility(base({
      ...graveyard, zoneChangeHistory: { event: 'put-into-zone', to: { zone: 'graveyard', owner: 'any' }, from: { zone: 'battlefield' }, during: 'this-turn' } })));
  });
  it('rolls back unsupported tails and refuses to certify unresolved copy scopes', () => {
    const c = new Cursor(lex(start + ", except it's a creature if you control an artifact."));
    assert.equal(parseEntryCopy(c, parseGainedAbilities), null);
    assert.equal(c.pos, 0);
    const result = parseOracleText(start + '.', 'Copy fixture', { includeSourceProvenance: true });
    assert.equal(result.ok, true);
    const analysis = analyzeCardBindings({ identity: 'copy', source: { name: 'Copy fixture', layout: 'normal' }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(analysis.fullyBound, false);
    assert.ok(analysis.diagnostics.some(d => d.code === 'unsupported-scope' && d.message.includes('Entry-copy')));
  });
});
