import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
import type { Effect, ObjectRef } from '../src/ast.ts';

// Named wording checked against the frozen Scryfall source corpus. CR 608.2c
// explicitly gives regeneration bans as an example of later text modifying
// an earlier action; CR 122.6 distinguishes placement from entering counters.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const creature = { types: ['creature'] };
const target: ObjectRef = { ref: 'target', count: n(1), filter: creature };
const counter = (count: number, type: string, on: ObjectRef): Effect => ({ effect: 'put-counters', count: n(count), counter: { type }, on });
function effects(text: string, provenance = false) {
  const result = parseOracleText(text, undefined, { includeSourceProvenance: provenance });
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  const ability = result.abilities[0];
  assert.equal(ability.kind, 'spell');
  if (ability.kind !== 'spell') throw new Error('expected spell');
  return ability.effects;
}

describe('compound counter instructions', () => {
  it('places Sagu Pummeler counter kinds simultaneously on one target', () => {
    const on = { ...target, selectionId: 'line:0:13' };
    assert.deepEqual(effects('Put two +1/+1 counters and a reach counter on target creature.', true), [
      { steps: [{ effect: 'simultaneous', steps: [counter(2, '+1/+1', on), counter(1, 'reach', on)] }] },
    ]);
  });
  it('keeps Juniper Order Ranger recipients and quantities distinct', () => {
    assert.deepEqual(effects('Put a +1/+1 counter on that creature and a +1/+1 counter on this creature.'), [
      { steps: [{ effect: 'simultaneous', steps: [counter(1, '+1/+1', { ref: 'that', filter: creature }), counter(1, '+1/+1', { ref: 'this', noun: 'creature' })] }] },
    ]);
    assert.deepEqual(effects('Put a +1/+1 counter on target creature, then put two -1/-1 counters on target artifact.'), [
      { steps: [counter(1, '+1/+1', target), counter(2, '-1/-1', { ref: 'target', count: n(1), filter: { types: ['artifact'] } })] },
    ]);
  });
  it('distinguishes one optional instruction and a separately stated optional instruction', () => {
    assert.deepEqual(effects('You may put two +1/+1 counters and a reach counter on target creature.'), [
      { steps: [{ effect: 'simultaneous', optional: true, steps: [counter(2, '+1/+1', target), counter(1, 'reach', target)] }] },
    ]);
    assert.deepEqual(effects('Put another +1/+1 counter on this creature.'), [{ steps: [counter(1, '+1/+1', { ref: 'this', noun: 'creature' })] }]);
  });
  it('rejects incomplete and mixed recipient scopes', () => {
    for (const text of ['Put two +1/+1 counters and a reach counter.', 'Put two +1/+1 counters on target creature and a reach counter.', 'Put two +1/+1 counters or a reach counter on target creature.'])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe('destruction without regeneration', () => {
  it('folds Oxidize and Wrath of God riders into destruction itself', () => {
    assert.deepEqual(effects("Destroy target artifact. It can't be regenerated."), [{ steps: [{ effect: 'destroy', what: { ...target, filter: { types: ['artifact'] } }, regenerationForbidden: true }] }]);
    assert.deepEqual(effects("Destroy all creatures. They can't be regenerated."), [{ steps: [{ effect: 'destroy', what: { ref: 'all', filter: creature }, regenerationForbidden: true }] }]);
    assert.deepEqual(effects("Destroy target creature. That creature can't be regenerated."), [{ steps: [{ effect: 'destroy', what: target, regenerationForbidden: true }] }]);
  });
  it('applies a plural rider to all simultaneous destruction targets', () => {
    assert.deepEqual(effects("Destroy target creature and target land. They can't be regenerated."), [{ steps: [{ effect: 'simultaneous', steps: [
      { effect: 'destroy', what: target, regenerationForbidden: true },
      { effect: 'destroy', what: { ...target, filter: { types: ['land'] } }, regenerationForbidden: true },
    ] }] }]);
  });
  it('retains a leading condition and allows subsequent independent instructions', () => {
    assert.deepEqual(effects("Destroy target creature. It can't be regenerated. Draw a card."), [
      { steps: [{ effect: 'destroy', what: target, regenerationForbidden: true }] },
      { steps: [{ effect: 'draw', who: { player: 'you' }, amount: n(1) }] },
    ]);
    assert.deepEqual(effects("If you control a Swamp, destroy target creature. It can't be regenerated."), [{
      condition: { condition: 'control', who: { player: 'you' }, what: { ref: 'filter', filter: { subtypes: ['Swamp'] }, count: n(1) } }, conditionKind: 'if',
      steps: [{ effect: 'destroy', what: target, regenerationForbidden: true }],
    }]);
  });
  it('preserves independently timed and static regeneration prohibitions', () => {
    assert.deepEqual(effects("Target creature can't be regenerated this turn."), [{ steps: [{ effect: 'cant', what: target, action: 'be-regenerated', duration: { duration: 'this-turn' } }] }]);
    const result = parseOracleText("Creatures can't be regenerated.");
    assert.ok(result.ok);
    assert.deepEqual(result.abilities, [{ kind: 'static', abilityWord: undefined, effect: { static: 'cant', what: { ref: 'each', filter: creature }, action: 'be-regenerated', condition: undefined } }]);
  });
  it('rejects unproved rider referents and late or lasting rider scope', () => {
    for (const text of ["Destroy target creature. They can't be regenerated.", "Destroy all creatures. It can't be regenerated.", "Destroy target artifact. That creature can't be regenerated.", "Destroy target creature. Draw a card. It can't be regenerated.", "Destroy target creature. It can't be regenerated this turn.", "If you control a Swamp, destroy target creature. Otherwise, destroy target artifact. It can't be regenerated."])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe('optional resolution payments', () => {
  it('offers the Justice payment before its conditional sacrifice', () => {
    assert.deepEqual(effects('Sacrifice this enchantment unless you pay {W}{W}.'), [{
      steps: [{ effect: 'sacrifice', who: { player: 'you' }, what: { ref: 'this', noun: 'enchantment' } }],
      condition: { condition: 'pays', who: { player: 'you' }, cost: [{ cost: 'mana', symbols: ['W', 'W'] }] }, conditionKind: 'unless',
    }]);
  });
  it('preserves discard and remove-counter payments without making them simultaneous with the alternative', () => {
    assert.deepEqual(effects('Draw three cards. Then discard two cards unless you discard a creature card.'), [
      { steps: [{ effect: 'draw', who: { player: 'you' }, amount: n(3) }] },
      { steps: [{ effect: 'discard', who: { player: 'you' }, what: { ref: 'filter', filter: { cls: 'card' }, count: n(2) }, random: undefined }],
        condition: { condition: 'pays', who: { player: 'you' }, cost: [{ cost: 'discard', what: { ref: 'filter', filter: { ...creature, cls: 'card' }, count: n(1) } }] }, conditionKind: 'unless' },
    ]);
    assert.deepEqual(effects('Sacrifice this creature unless you remove a +1/+1 counter from it.'), [{
      steps: [{ effect: 'sacrifice', who: { player: 'you' }, what: { ref: 'this', noun: 'creature' } }],
      condition: { condition: 'pays', who: { player: 'you' }, cost: [{ cost: 'remove-counters', counter: { type: '+1/+1' }, count: n(1), from: { ref: 'it' } }] }, conditionKind: 'unless',
    }]);
  });
  it('keeps a distinct paying player and the return destination', () => {
    assert.deepEqual(effects("Sacrifice this creature unless you return an untapped Island you control to its owner's hand."), [{
      steps: [{ effect: 'sacrifice', who: { player: 'you' }, what: { ref: 'this', noun: 'creature' } }],
      condition: { condition: 'pays', who: { player: 'you' }, cost: [{ cost: 'return', what: { ref: 'filter', filter: { status: ['untapped'], subtypes: ['Island'], control: 'you' }, count: n(1) }, to: 'hand', owner: 'owner' }] }, conditionKind: 'unless',
    }]);
    assert.deepEqual(effects('Counter target spell unless its controller sacrifices a creature.'), [{
      steps: [{ effect: 'counter', what: { ...target, filter: { cls: 'spell' } } }],
      condition: { condition: 'pays', who: { player: 'controller', of: { ref: 'it' } }, cost: [{ cost: 'sacrifice', what: { ref: 'filter', filter: creature, count: n(1) } }] }, conditionKind: 'unless',
    }]);
  });
  it('rejects incomplete or unsupported payments', () => {
    for (const text of ['Sacrifice this creature unless you pay.', 'Counter target spell unless its controller discards.', 'Sacrifice this creature unless you pay {T}.', 'Sacrifice this creature unless you pay {1} for each creature you control.'])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe('shuffling objects into libraries', () => {
  it('preserves the full Game Plan move group and follows it with the draw', () => {
    assert.deepEqual(effects('Each player shuffles their hand and graveyard into their library, then draws seven cards.'), [{ steps: [
      { effect: 'shuffle-into-library', who: { player: 'each-player' }, what: [
        { ref: 'zone-cards', zone: 'hand', owner: { player: 'they' } }, { ref: 'zone-cards', zone: 'graveyard', owner: { player: 'they' } },
      ], library: { zone: 'library', owner: 'their' } },
      { effect: 'draw', who: { player: 'each-player' }, amount: n(7) },
    ] }]);
  });
  it('preserves Black Sun\'s Zenith destination and the distinct Dwell on the Past targets', () => {
    assert.deepEqual(effects("Shuffle this spell into its owner's library."), [{ steps: [
      { effect: 'shuffle-into-library', who: { player: 'you' }, what: [{ ref: 'this', noun: 'spell' }], library: { zone: 'library', owner: 'its-owner' } },
    ] }]);
    assert.deepEqual(effects('Target player shuffles up to four target cards from their graveyard into their library.', true), [{ steps: [
      { effect: 'shuffle-into-library', who: { player: 'target-player', selectionId: 'line:0:0' }, what: [
        { ref: 'target', count: n(4), upTo: true, selectionId: 'line:0:6', filter: { cls: 'card', zone: { zone: 'graveyard', owner: 'their' } } },
      ], library: { zone: 'library', owner: 'their' } },
    ] }]);
  });
  it('keeps library ownership separate from the shuffling actor', () => {
    assert.deepEqual(effects('Target player shuffles your library.'), [{ steps: [
      { effect: 'shuffle', who: { player: 'target-player' }, library: { zone: 'library', owner: 'your' } },
    ] }]);
    assert.deepEqual(effects('Shuffle.'), [{ steps: [
      { effect: 'shuffle', who: { player: 'you' }, library: { zone: 'library', owner: { player: 'you' } } },
    ] }]);
  });
  it('rejects unrepresented library distributions, positioned destinations and incomplete groups', () => {
    for (const text of ["Shuffle those cards into their owners' libraries.", 'Shuffle this card into the top of your library.', 'Shuffle your hand and into your library.', 'Shuffle your graveyard.', 'Shuffle it into a graveyard.'])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe('explicit actors in coordinated instructions', () => {
  it('preserves Strategic Betrayal player choice and both simultaneous exile groups', () => {
    const who = { player: 'target-opponent', selectionId: 'line:0:0' };
    assert.deepEqual(effects('Target opponent exiles a creature they control and their graveyard.', true), [{ steps: [
      { effect: 'simultaneous', steps: [
        { effect: 'exile', who, what: { ref: 'filter', filter: { types: ['creature'], controlledBy: { player: 'they' } }, count: n(1) } },
        { effect: 'exile', who, what: { ref: 'zone-cards', zone: 'graveyard', owner: { player: 'they' } } },
      ] },
    ] }]);
  });
  it('carries the actor through conjugated tapping and untapping without changing targets', () => {
    assert.deepEqual(effects('That player taps target creature and untaps target land.'), [{ steps: [
      { effect: 'tap', who: { player: 'that-player' }, what: target },
      { effect: 'untap', who: { player: 'that-player' }, what: { ...target, filter: { types: ['land'] } } },
    ] }]);
    assert.deepEqual(effects("That player returns target creature to its owner's hand."), [{ steps: [
      { effect: 'return-to-hand', who: { player: 'that-player' }, what: target, to: 'owner' },
    ] }]);
  });
  it('distinguishes Pharika\'s Cure next actor from an additional damage recipient', () => {
    const result = parseOracleText("Pharika's Cure deals 2 damage to target creature and you gain 2 life.", "Pharika's Cure");
    assert.ok(result.ok);
    assert.deepEqual(result.abilities, [{ kind: 'spell', abilityWord: undefined, effects: [{ steps: [
      { effect: 'damage', source: { ref: 'self' }, amount: n(2), to: { to: 'object', ref: target } },
      { effect: 'gain-life', who: { player: 'you' }, amount: n(2) },
    ] }] }]);
    assert.deepEqual(effects('This creature deals 2 damage to each creature and you.'), [{ steps: [
      { effect: 'damage', source: { ref: 'this', noun: 'creature' }, amount: n(2), to: { to: 'each', refs: [{ ref: 'each', filter: creature }, { player: 'you' }] } },
    ] }]);
    assert.deepEqual(effects('This creature deals 2 damage to target creature and target player draws a card.'), [{ steps: [
      { effect: 'damage', source: { ref: 'this', noun: 'creature' }, amount: n(2), to: { to: 'object', ref: target } },
      { effect: 'draw', who: { player: 'target-player' }, amount: n(1) },
    ] }]);
  });
  it('rejects unsupported destinations and incomplete actor clauses', () => {
    for (const text of ['Target player returns a creature they control to their hand.', 'Target opponent exiles and their graveyard.', 'That player taps target creature and untaps.'])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});
