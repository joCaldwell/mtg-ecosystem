import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// Named text checked in the cached Scryfall source. CR 700.2h-i, 702.172 and
// 702.183 distinguish choice count, casting costs, and weighted limits.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };
const draw = { effect: 'draw', who: you, amount: n(1) };
const end = { duration: 'end-of-turn' };
const signed = (value: number) => ({ sign: 1, amount: n(value) });
function one(text: string, name?: string) {
  const result = parseOracleText(text, name, { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}

describe('modal casting costs and weighted budgets', () => {
  it('keeps Three Steps Ahead mode costs separate from resolving instructions', () => {
    assert.deepEqual(one("Spree (Choose one or more additional costs.)\n+ {1}{U} — Counter target spell.\n+ {3} — Create a token that's a copy of target artifact or creature you control.\n+ {2} — Draw two cards, then discard a card."), {
      kind: 'spell', effects: [{ steps: [{ effect: 'modal', keyword: 'spree', count: { atLeast: 1 }, options: [
        { additionalCost: [{ cost: 'mana', symbols: ['1', 'U'] }], effects: [{ steps: [{ effect: 'counter', what: { ref: 'target', filter: { cls: 'spell' }, count: n(1), selectionId: 'line:1:5' } }] }] },
        { additionalCost: [{ cost: 'mana', symbols: ['3'] }], effects: [{ steps: [{ effect: 'create-token', count: n(1), token: { copyOf: { ref: 'target', filter: { allOf: [{ anyOf: [{ types: ['artifact'] }, { types: ['creature'] }] }], control: 'you' }, count: n(1), selectionId: 'line:2:10' } } }] }] },
        { additionalCost: [{ cost: 'mana', symbols: ['2'] }], effects: [{ steps: [{ effect: 'draw', who: you, amount: n(2) }, { effect: 'discard', who: you, what: { ref: 'filter', filter: { cls: 'card' }, count: n(1) } }] }] },
      ] }] }],
    });
  });
  it('keeps all Thunder Magic labels, additional costs, damage amounts and target identities', () => {
    assert.deepEqual(one('Tiered (Choose one additional cost.)\n• Thunder — {0} — Thunder Magic deals 2 damage to target creature.\n• Thundara — {3} — Thunder Magic deals 4 damage to target creature.\n• Thundaga — {5}{R} — Thunder Magic deals 8 damage to target creature.', 'Thunder Magic'), {
      kind: 'spell', effects: [{ steps: [{ effect: 'modal', keyword: 'tiered', count: { exactly: 1 }, options: [
        { label: 'Thunder', additionalCost: [{ cost: 'mana', symbols: ['0'] }], effects: [{ steps: [{ effect: 'damage', source: { ref: 'self' }, amount: n(2), to: { to: 'object', ref: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:1:10' } } }] }] },
        { label: 'Thundara', additionalCost: [{ cost: 'mana', symbols: ['3'] }], effects: [{ steps: [{ effect: 'damage', source: { ref: 'self' }, amount: n(4), to: { to: 'object', ref: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:2:10' } } }] }] },
        { label: 'Thundaga', additionalCost: [{ cost: 'mana', symbols: ['5', 'R'] }], effects: [{ steps: [{ effect: 'damage', source: { ref: 'self' }, amount: n(8), to: { to: 'object', ref: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:3:11' } } }] }] },
      ] }] }],
    });
  });
  it('preserves Season of Weaving weights, repetition, and choices made during resolution', () => {
    assert.deepEqual(one("Choose up to five {P} worth of modes. You may choose the same mode more than once.\n{P} — Draw a card.\n{P}{P} — Choose an artifact or creature you control. Create a token that's a copy of it.\n{P}{P}{P} — Return each nonland, nontoken permanent to its owner's hand."), {
      kind: 'spell', effects: [{ steps: [{ effect: 'modal', count: { weightBudget: 5 }, allowRepeat: true, options: [
        { weight: 1, effects: [{ steps: [draw] }] },
        { weight: 2, effects: [
          { steps: [{ effect: 'choose-object', who: you, what: { ref: 'filter', count: n(1), filter: { allOf: [{ anyOf: [{ types: ['artifact'] }, { types: ['creature'] }] }], control: 'you' } } }] },
          { steps: [{ effect: 'create-token', count: n(1), token: { copyOf: { ref: 'it' } } }] },
        ] },
        { weight: 3, effects: [{ steps: [{ effect: 'return-to-hand', what: { ref: 'each', filter: { nonTypes: ['land'], nonToken: true, cls: 'permanent' } }, to: 'owner' }] }] },
      ] }] }],
    });
  });
  it('keeps Kargan Intimidator per-turn mode history on the modal ability', () => {
    assert.deepEqual(one("{1}: Choose one that hasn't been chosen this turn —\n• This creature gets +1/+1 until end of turn.\n• Target creature becomes a Coward until end of turn.\n• Target Warrior gains trample until end of turn."), {
      kind: 'activated', costs: [{ cost: 'mana', symbols: ['1'] }], effects: [{ steps: [{ effect: 'modal', count: { exactly: 1 }, unchosen: { period: 'turn' }, options: [
        { effects: [{ steps: [{ effect: 'pump', what: { ref: 'this', noun: 'creature' }, power: signed(1), toughness: signed(1), duration: end }] }] },
        { effects: [{ steps: [{ effect: 'become', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:2:1' }, spec: { subtypes: ['Coward'] }, duration: end }] }] },
        { effects: [{ steps: [{ effect: 'gain-abilities', what: { ref: 'target', filter: { subtypes: ['Warrior'] }, count: n(1), selectionId: 'line:3:1' }, abilities: [{ gained: 'keyword', keyword: { keyword: 'trample' } }], duration: end }] }] },
      ] }] }],
    });
  });
  it('distinguishes source-object history from a per-turn reset', () => {
    assert.deepEqual(one("Choose one that hasn't been chosen.\n• Draw a card.\n• You gain 2 life."), {
      kind: 'spell', effects: [{ steps: [{ effect: 'modal', count: { exactly: 1 }, unchosen: { period: 'source-object' }, options: [
        { effects: [{ steps: [draw] }] }, { effects: [{ steps: [{ effect: 'gain-life', who: you, amount: n(2) }] }] },
      ] }] }],
    });
  });
  it('fails the entire block for missing costs, unsupported bodies, or mismatched mode formats', () => {
    for (const text of [
      'Spree\n+ Draw a card.\n+ {1} — You gain 2 life.',
      'Spree\n+ {1} — Draw a card.\n+ {2} — Draw a banana.',
      'Tiered\n• Fast — {1} — Draw a card.\n• Slow — You gain 2 life.',
      'Choose up to five {P} worth of modes.\n• Draw a card.',
      'Choose one —\n{P} — Draw a card.',
      'Choose five {P} worth of modes.\n{P} — Draw a card.',
      "Choose one that hasn't been chosen. You may choose the same mode more than once.\n• Draw a card.",
      'Spree', 'Tiered', '+ {1} — Draw a card.', '{P} — Draw a card.',
      '{1}: Tiered\n• Fast — {1} — Draw a card.\n• Slow — {2} — You gain 2 life.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});
