import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { Ability, Amount } from '../src/ast.ts';
import { parseOracleText } from '../src/index.ts';
import { finalizeCharacteristicDefinitions } from '../src/parser/characteristic-origin.ts';
import { analyzeCardBindings } from '../src/semantic/bindings.ts';

// Oracle clauses checked against the cached source. CR 604.3a(2),(4) and
// 613.4a-b distinguish intrinsic definitions from ordinary granted abilities.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };
const creature = { ref: 'this', noun: 'creature' };
const enchantment = { ref: 'this', noun: 'enchantment' };
const lands = { amount: 'count', of: { types: ['land'], control: 'you' } };
function one(text: string) {
  const result = parseOracleText(text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
function quotedSetter(value: object) {
  return { gained: 'quoted', ability: { kind: 'static', effect: { static: 'modify', what: creature,
    modifiers: [{ modifier: 'base-pt', power: value, toughness: value }] } } };
}
describe('characteristic definition origins', () => {
  it('recognizes Drop Bear has-power-and-toughness wording as an intrinsic definition', () => {
    const value = { amount: 'plus', a: { amount: 'count', of: { subtypes: ['Forest'], control: 'you' } },
      b: { amount: 'count', of: { subtypes: ['Bear'], control: 'you' } } };
    assert.deepEqual(one('This creature has power and toughness each equal to the number of Forests you control plus the number of Bears you control.'), {
      kind: 'static', effect: { static: 'cda-pt', what: creature, stat: 'both', value },
    });
    assert.deepEqual(one('As long as you control a Forest, this creature has power and toughness each equal to the number of Forests you control plus the number of Bears you control.'), {
      kind: 'static', effect: { static: 'conditional', condition: { condition: 'control', who: you,
        what: { ref: 'filter', filter: { subtypes: ['Forest'] }, count: n(1) } },
      effect: { static: 'modify', what: creature, modifiers: [{ modifier: 'base-pt', power: value, toughness: value }] } },
    });
  });
  it('keeps Myth Realized self-granted P/T in layer 7b, with dynamic counters', () => {
    const counters = { amount: 'counter-count', counter: { type: 'lore' }, on: { ref: 'it' } };
    const duration = { duration: 'end-of-turn' };
    assert.deepEqual(one('{W}: Until end of turn, this enchantment becomes a Monk Avatar creature in addition to its other types and gains "This creature\'s power and toughness are each equal to the number of lore counters on it."'), {
      kind: 'activated', costs: [{ cost: 'mana', symbols: ['W'] }], effects: [{ steps: [
        { effect: 'become', what: enchantment, spec: { types: ['creature'], subtypes: ['Monk', 'Avatar'], inAddition: 'types' }, duration },
        { effect: 'gain-abilities', what: enchantment, abilities: [quotedSetter(counters)], duration },
      ] }],
    });
  });
  it("keeps Beorn's Hospitality's granted definition dynamic and without an invented duration", () => {
    assert.deepEqual(one('{5}{G}{G}: This enchantment becomes a Bear creature in addition to its other types and gains "This creature\'s power and toughness are each equal to the number of lands you control."'), {
      kind: 'activated', costs: [{ cost: 'mana', symbols: ['5', 'G', 'G'] }], effects: [{ steps: [
        { effect: 'become', what: enchantment, spec: { types: ['creature'], subtypes: ['Bear'], inAddition: 'types' } },
        { effect: 'gain-abilities', what: enchantment, abilities: [quotedSetter(lands)] },
      ] }],
    });
  });
  it('preserves the CDA in the intrinsic token definition completed by Daxos the Returned', () => {
    assert.deepEqual(one('{1}{W}{B}: Create a white and black Spirit enchantment creature token. It has "This creature\'s power and toughness are each equal to the number of experience counters you have."'), {
      kind: 'activated', costs: [{ cost: 'mana', symbols: ['1', 'W', 'B'] }], effects: [{ steps: [{
        effect: 'create-token', count: n(1), token: { colors: ['white', 'black'], supertypes: [],
          types: ['enchantment', 'creature'], subtypes: ['Spirit'], keywords: [], abilities: [{ gained: 'quoted', ability: {
            kind: 'static', effect: { static: 'cda-pt', what: creature, stat: 'both', value: {
              amount: 'counter-count', counter: { type: 'experience' }, on: you,
            } },
          } }],
        },
      }] }],
    });
  });
  it('preserves one-stat definitions and never mutates the independently parsed tree', () => {
    const value: Amount = { amount: 'count', of: { types: ['land'], control: 'you' } };
    for (const stat of ['power', 'toughness'] as const) {
      const definition: Ability = { kind: 'static', effect: { static: 'cda-pt', what: { ref: 'this', noun: 'creature' }, stat, value } };
      const granted: Ability = { kind: 'spell', effects: [{ steps: [{ effect: 'gain-abilities', what: { ref: 'self' },
        abilities: [{ gained: 'quoted', ability: definition }] }] }] };
      const before: Ability = structuredClone(granted);
      assert.deepEqual(finalizeCharacteristicDefinitions(definition), definition);
      assert.deepEqual(finalizeCharacteristicDefinitions(granted), { kind: 'spell', effects: [{ steps: [{ effect: 'gain-abilities', what: { ref: 'self' },
        abilities: [{ gained: 'quoted', ability: { kind: 'static', effect: { static: 'modify', what: creature,
          modifiers: [{ modifier: 'base-pt', [stat]: value }] } } }],
      }] }] });
      assert.deepEqual(granted, before);
    }
  });
  it('keeps compound intrinsic color/type/unequal-P/T parts in one CDA group and converts every granted part', () => {
    const change = { subtypes: [{ family: 'creature', operation: 'set', types: ['Frog'] }] };
    const text = 'This creature is a blue Frog and has base power and toughness 2/3.';
    assert.deepEqual(one(text), { kind: 'static', effect: { static: 'group', effects: [
      { static: 'cda-types', what: creature, change },
      { static: 'cda-colors', what: creature, operation: 'set', colors: ['blue'] },
      { static: 'cda-pt', what: creature, stat: 'power', value: n(2) },
      { static: 'cda-pt', what: creature, stat: 'toughness', value: n(3) },
    ] } });
    assert.deepEqual(one(`This creature gains "${text}" until end of turn.`), {
      kind: 'spell', effects: [{ steps: [{ effect: 'gain-abilities', what: creature,
        abilities: [{ gained: 'quoted', ability: { kind: 'static', effect: { static: 'group', effects: [
          { static: 'modify', what: creature, modifiers: [{ modifier: 'types', change }] },
          { static: 'modify', what: creature, modifiers: [{ modifier: 'colors', operation: 'set', colors: ['blue'] }] },
          { static: 'modify', what: creature, modifiers: [{ modifier: 'base-pt', power: n(2) }] },
          { static: 'modify', what: creature, modifiers: [{ modifier: 'base-pt', toughness: n(3) }] },
        ] } } }], duration: { duration: 'end-of-turn' },
      }] }],
    });
    assert.deepEqual(one('This creature is blue in addition to its other colors.'), {
      kind: 'static', effect: { static: 'cda-colors', what: creature, operation: 'add', colors: ['blue'] },
    });
    assert.deepEqual(one('This creature gains "This creature is blue in addition to its other colors." until end of turn.'), {
      kind: 'spell', effects: [{ steps: [{ effect: 'gain-abilities', what: creature, abilities: [{ gained: 'quoted', ability: {
        kind: 'static', effect: { static: 'modify', what: creature, modifiers: [{ modifier: 'colors', operation: 'add', colors: ['blue'] }] },
      } }], duration: { duration: 'end-of-turn' } }] }],
    });
  });
  it('preserves definitions added by token copy exceptions but not later ordinary grants', () => {
    const definition = { gained: 'quoted', ability: { kind: 'static', effect: {
      static: 'cda-pt', what: creature, stat: 'both', value: lands,
    } } };
    assert.deepEqual(one('Create a token that\'s a copy of target creature, except it has "This creature\'s power and toughness are each equal to the number of lands you control."'), {
      kind: 'spell', effects: [{ steps: [{ effect: 'create-token', count: n(1), token: {
        copyOf: { ref: 'target', filter: { types: ['creature'] }, count: n(1) },
        modifications: [{ modification: 'abilities', abilities: [definition] }],
      } }] }],
    });
    assert.deepEqual(one('Create a token that\'s a copy of target creature. It gains "This creature\'s power and toughness are each equal to the number of lands you control."'), {
      kind: 'spell', effects: [
        { steps: [{ effect: 'create-token', count: n(1), token: { copyOf: { ref: 'target', filter: { types: ['creature'] }, count: n(1) } } }] },
        { steps: [{ effect: 'gain-abilities', what: { ref: 'it' }, abilities: [quotedSetter(lands)] }] },
      ],
    });
  });
  it('does not falsely bind a dynamic per-object characteristic to the source', () => {
    const result = parseOracleText('Each noncreature artifact is an artifact creature with power and toughness each equal to its mana value.', 'March of the Machines');
    assert.equal(result.ok, true);
    const analysis = analyzeCardBindings({ identity: 'characteristic-scope', source: { name: 'March of the Machines', layout: 'normal', type_line: 'Enchantment' },
      faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(analysis.fullyBound, false);
    assert.ok(analysis.diagnostics.some(d => d.code === 'unsupported-scope' && d.message.includes('dynamic characteristic value')));
  });
});
