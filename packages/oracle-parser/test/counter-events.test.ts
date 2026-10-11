import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { Ability, CounterChangeMatch, CounterChangeRecipient, CounterSpec, Effect, Trigger } from '../src/ast.ts';
import { parseOracleText } from '../src/index.ts';
import { parseTrigger } from '../src/parser/triggers.ts';
import { parseCounterChangeTrigger } from '../src/parser/counter-events.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { lex } from '../src/lexer.ts';
import { analyzeCardBindings } from '../src/semantic/bindings.ts';

// Actual text pinned to Scryfall. CR122.2/6/7 and603.2c define removal,
// entry with counters, threshold crossings, and occurrence grouping:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value }) as const;
const you = { player: 'you' } as const;
const self = { ref: 'this', noun: 'creature' } as const;
const object: CounterChangeRecipient = { recipient: 'object', what: self };
const plus = { type: '+1/+1' };
const grouped: CounterChangeMatch = { counters: 'one-or-more', recipients: 'each' };
const event = (counter: CounterSpec | 'any' = plus, on: CounterChangeRecipient = object, match: CounterChangeMatch = grouped, direction: 'put' | 'remove' = 'put'): Extract<Trigger, { trigger: 'counter-change' }> =>
  ({ trigger: 'counter-change', direction, counter, on, match });
const ability = (trigger: Trigger, steps: Effect[]): Ability => ({ kind: 'triggered', abilityWord: undefined, condition: undefined, trigger, effects: [{ steps }] });
const draw: Effect = { effect: 'draw', who: you, amount: n(1) };
const oneCreature: CounterChangeRecipient = { recipient: 'object', what: { ref: 'filter', filter: { types: ['creature'], control: 'you' }, count: n(1) } };
function parse(text: string, name = 'Counter fixture', provenance = false) {
  const result = parseOracleText(text, name, { includeSourceProvenance: provenance });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return result;
}
function header(text: string): Trigger {
  const c = new Cursor(lex(text)); const result = parseTrigger(c);
  assert.ok(result && c.done(), text + ': ' + c.errorMessage());
  return result;
}

describe('counter-change triggers preserve occurrence, actor, and recipient scopes', () => {
  it('groups multiple matching counters on the source into one occurrence (Herd Baloth)', () => {
    assert.deepEqual(parse('Whenever one or more +1/+1 counters are put on this creature, you may create a 4/4 green Beast creature token.', 'Herd Baloth').abilities, [
      ability(event(), [{ effect: 'create-token', count: n(1), optional: true, token: { power: n(4), toughness: n(4), colors: ['green'], supertypes: [], types: ['creature'], subtypes: ['Beast'], keywords: [] } }]),
    ]);
  });
  it('distinguishes per-counter occurrences from grouped counters (Fathom Mage)', () => {
    assert.deepEqual(parse('Evolve\nWhenever a +1/+1 counter is put on this creature, you may draw a card.', 'Fathom Mage').abilities, [
      { kind: 'keywords', keywords: [{ keyword: 'evolve' }] },
      ability(event(plus, object, { counters: 'each', recipients: 'each' }), [{ ...draw, optional: true }]),
    ]);
    assert.deepEqual(header('Whenever a counter is put on a creature you control'), event('any', oneCreature, { counters: 'each', recipients: 'each' }));
    assert.deepEqual(header('Whenever one or more counters are put on a creature you control'), event('any', oneCreature));
  });
  it('groups recipients only when explicitly printed (Cloaked Cadet)', () => {
    const trigger = event(plus, { recipient: 'object', what: { ref: 'each', filter: { subtypes: ['Human'], control: 'you' } } }, { counters: 'one-or-more', recipients: 'one-or-more' });
    assert.deepEqual(parse('Training\nWhenever one or more +1/+1 counters are put on one or more Humans you control, draw a card. This ability triggers only once each turn.', 'Cloaked Cadet').abilities, [
      { kind: 'keywords', keywords: [{ keyword: 'training' }] },
      { ...ability(trigger, [draw]), limit: { maximum: 1, period: 'turn' } },
    ]);
    assert.deepEqual(header('Whenever one or more +1/+1 counters are put on a creature you control'), event(plus, oneCreature));
  });
  it('retains the placing player without inferring the actor of passive placement (Stocking the Pantry)', () => {
    const source = { ref: 'this', noun: 'enchantment' } as const;
    assert.deepEqual(parse('Whenever you put one or more +1/+1 counters on a creature you control, put a supply counter on this enchantment.\n{2}, Remove a supply counter from this enchantment: Draw a card.', 'Stocking the Pantry').abilities, [
      ability({ ...event(plus, oneCreature), by: you }, [{ effect: 'put-counters', counter: { type: 'supply' }, on: source, count: n(1) }]),
      { kind: 'activated', abilityWord: undefined, restriction: undefined, costs: [{ cost: 'mana', symbols: ['2'] }, { cost: 'remove-counters', counter: { type: 'supply' }, count: n(1), from: source }], effects: [{ steps: [draw] }] },
    ]);
    assert.deepEqual(header('Whenever an opponent puts one or more counters on a creature they control'), {
      ...event('any', { recipient: 'object', what: { ref: 'filter', filter: { types: ['creature'], controlledBy: { player: 'they' } }, count: n(1) } }), by: { player: 'an-opponent' },
    });
  });
  it('retains object/player alternative eligibility and the exact player recipient', () => {
    assert.deepEqual(header('Whenever you put one or more counters on a permanent or player'), {
      ...event('any', { recipient: 'any-of', recipients: [
        { recipient: 'object', what: { ref: 'filter', filter: { cls: 'permanent' }, count: n(1) } },
        { recipient: 'player', who: { player: 'a-player' } },
      ] }), by: you,
    });
    assert.deepEqual(header('Whenever one or more poison counters are put on you'), event({ type: 'poison' }, { recipient: 'player', who: you }));
    assert.deepEqual(header('Whenever an experience counter is removed from a player'), event({ type: 'experience' }, { recipient: 'player', who: { player: 'a-player' } }, { counters: 'each', recipients: 'each' }, 'remove'));
  });
  it('represents Nth-counter wording as an upward count crossing, separate from event ordinals', () => {
    assert.deepEqual(header('When the fourth plan counter is put on this enchantment'), event({ type: 'plan' }, { recipient: 'object', what: { ref: 'this', noun: 'enchantment' } }, { transition: 'crosses-upward', threshold: 4 }));
    assert.deepEqual(header('When the twelfth hour counter is put on this artifact'), event({ type: 'hour' }, { recipient: 'object', what: { ref: 'this', noun: 'artifact' } }, { transition: 'crosses-upward', threshold: 12 }));
    assert.deepEqual(parse('Whenever one or more +1/+1 counters are put on this creature for the first time each turn, create a Treasure token.', 'Axgard Artisan').abilities, [
      ability({ ...event(), occurrence: { ordinal: 1, period: 'turn' } }, [{ effect: 'create-token', count: n(1), token: { predefined: 'Treasure' } }]),
    ]);
  });
  it('separates removal, last-counter transition, and exile-state qualification', () => {
    const source: CounterChangeRecipient = { recipient: 'object', what: { ref: 'this', noun: 'card' } };
    const gate = { gate: 'condition', condition: { condition: 'is-filter', what: { ref: 'it' }, filter: { zone: { zone: 'exile' } } } } as const;
    assert.deepEqual(header("When the last time counter is removed from this card while it's exiled"), { trigger: 'event-gated', event: event({ type: 'time' }, source, { transition: 'last-removed' }, 'remove'), gate });
    assert.deepEqual(header("Whenever you remove a time counter from this card while it's exiled"), { trigger: 'event-gated', event: { ...event({ type: 'time' }, source, { counters: 'each', recipients: 'each' }, 'remove'), by: you }, gate });
    assert.deepEqual(header('Whenever one or more loyalty counters are removed from this planeswalker'), event({ type: 'loyalty' }, { recipient: 'object', what: { ref: 'this', noun: 'planeswalker' } }, grouped, 'remove'));
  });
  it('retains event quantity references and original body target declaration identities', () => {
    const text = 'Whenever one or more +1/+1 counters are put on this creature, tap target creature.';
    assert.deepEqual(parse(text, undefined, true).abilities, [ability(event(), [{ effect: 'tap', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:17' } }])]);
    assert.deepEqual(parse('Whenever one or more charge counters are removed from this artifact, draw that many cards.').abilities, [
      ability(event({ type: 'charge' }, { recipient: 'object', what: { ref: 'this', noun: 'artifact' } }, grouped, 'remove'), [{ effect: 'draw', who: you, amount: { amount: 'that-much' } }]),
    ]);
  });
  it('does not guess unsupported quantity, kind, recipient-group, or target-announcement scopes', () => {
    for (const text of [
      'Whenever two counters are put on this creature, draw a card.',
      'Whenever one or more +1/+1 and -1/-1 counters are put on this creature, draw a card.',
      'Whenever two different kinds of counters are put on this creature, draw a card.',
      'Whenever a counter is put on one or more creatures, draw a card.',
      'Whenever one or more loyalty counters are put on planeswalkers you control, draw a card.',
      'When the fourth plan counter is removed from this enchantment, draw a card.',
      'When the last time counter is put on this card, draw a card.',
      'When the last time counter is removed from one or more cards, draw a card.',
      'Whenever one or more counters are put on target creature, draw a card.',
      'Whenever target player puts a counter on this creature, draw a card.',
      'Whenever a counter is put on a permanent or, draw a card.',
      'Whenever one or more counters would be put on this creature, draw a card.',
      'Whenever a counter disappears from this creature, draw a card.',
      'Whenever one or more counters are put on a creature you control for the first time each turn, draw a card.',
      'Whenever one or more counters are put on one or more creatures you control for the first time each turn, draw a card.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
    const c = new Cursor(lex('one or more counters are put on target creature'));
    assert.equal(parseCounterChangeTrigger(c), null);
    assert.equal(c.pos, 0);
  });
  it('keeps the last-removal exile gate on the whole suspended-card trigger (Veiling Oddity)', () => {
    const counterEvent = event({ type: 'time' }, { recipient: 'object', what: { ref: 'this', noun: 'card' } }, { transition: 'last-removed' }, 'remove');
    assert.deepEqual(parse("Suspend 4—{1}{U}\nWhen the last time counter is removed from this card while it's exiled, creatures can't be blocked this turn.", 'Veiling Oddity').abilities, [
      { kind: 'keywords', keywords: [{ keyword: 'suspend', amount: 4, cost: [{ cost: 'mana', symbols: ['1', 'U'] }] }] },
      ability({ trigger: 'event-gated', event: counterEvent, gate: { gate: 'condition', condition: { condition: 'is-filter', what: { ref: 'it' }, filter: { zone: { zone: 'exile' } } } } }, [
        { effect: 'cant', what: { ref: 'each', filter: { types: ['creature'] } }, action: 'be-blocked', duration: { duration: 'this-turn' } },
      ]),
    ]);
  });
  it('keeps event participants and removed-counter quantities unresolved, never substituting the source', () => {
    const result = parse('Whenever one or more counters are removed from a creature, draw that many cards.');
    const bound = analyzeCardBindings({ identity: 'counter-test', source: { name: 'Counter fixture', layout: 'normal', type_line: 'Artifact' }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(bound.fullyBound, false);
    assert.ok(bound.diagnostics.some(d => d.code === 'unsupported-scope' && /Counter-change/.test(d.message)));
    assert.ok(bound.references.some(r => 'amount' in r.expression && r.expression.amount === 'that-much' && r.binding === 'unresolved'));
  });
});
