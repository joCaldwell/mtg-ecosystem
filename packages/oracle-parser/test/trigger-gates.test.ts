import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { Ability, Condition, Effect, Trigger } from '../src/ast.ts';
import { parseOracleText } from '../src/parser/index.ts';
import { parseTrigger } from '../src/parser/triggers.ts';
import { parseTriggerGate } from '../src/parser/trigger-gates.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { lex } from '../src/lexer.ts';
import { analyzeCardBindings } from '../src/semantic/bindings.ts';

// Pinned actual Oracle text; CR603.1/4/10 and702.171 distinguish event-time
// gates, intervening-if checks, event snapshots, and saddled designations.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value }) as const;
const you = { player: 'you' } as const;
const self = { ref: 'this', noun: 'creature' } as const;
const draw: Effect = { effect: 'draw', who: you, amount: n(1) };
const attack: Trigger = { trigger: 'attacks', what: self, alone: undefined, whom: undefined };
const controlCreature: Condition = { condition: 'control', who: you, what: { ref: 'filter', filter: { types: ['creature'] }, count: n(1) } };
const gate = (event: Trigger, condition: Condition): Trigger => ({ trigger: 'event-gated', event, gate: { gate: 'condition', condition } });
const saddled = (event: Trigger): Trigger => ({ trigger: 'event-gated', event, gate: { gate: 'subject-status', status: 'saddled' } });
const ability = (trigger: Trigger, steps: Effect[], condition?: Condition): Ability => ({ kind: 'triggered', abilityWord: undefined,
  trigger, condition, effects: [{ steps }] });
function parse(text: string, provenance = false) {
  const result = parseOracleText(text, 'Trigger fixture', { typeLine: 'Creature — Beast', includeSourceProvenance: provenance });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return result;
}

describe('while-qualified trigger events retain local timing and subject scope', () => {
  it('gates only the attack branch of Autarch Mammoth, preserving its unconditional entry trigger', () => {
    const text = 'When this creature enters and whenever it attacks while saddled, create a 3/3 green Elephant creature token.\nSaddle 5 (Tap any number of other creatures you control with total power 5 or more: This Mount becomes saddled until end of turn. Saddle only as a sorcery.)';
    assert.deepEqual(parse(text).abilities, [
      ability({ trigger: 'any-of', events: [{ trigger: 'enters', what: self }, saddled({ trigger: 'attacks', what: { ref: 'it' }, alone: undefined, whom: undefined })] }, [{ effect: 'create-token', count: n(1), token: { colors: ['green'], supertypes: [], types: ['creature'], subtypes: ['Elephant'], keywords: [], power: n(3), toughness: n(3) } }]),
      { kind: 'keywords', keywords: [{ keyword: 'saddle', amount: 5 }] },
    ]);
    assert.deepEqual(parse('Whenever this creature attacks while saddled and whenever a creature dies, draw a card.').abilities, [
      ability({ trigger: 'any-of', events: [saddled(attack), { trigger: 'dies', what: { ref: 'filter', filter: { types: ['creature'] }, count: n(1) } }] }, [draw]),
    ]);
  });
  it('preserves total power as an event-time aggregate, not a resolving intervening condition (Tactical Officer)', () => {
    const result = parse('Whenever you attack while creatures you control have total power 8 or greater, put a +1/+1 counter on this creature. Untap it.');
    assert.deepEqual(result.abilities, [{ kind: 'triggered', abilityWord: undefined, condition: undefined,
      trigger: gate({ trigger: 'you-attack' }, { condition: 'compare', value: { amount: 'attribute-sum', attribute: 'power', among: { ref: 'each', filter: { types: ['creature'], control: 'you' } } }, comparison: { op: 'ge', value: n(8) } }),
      effects: [{ steps: [{ effect: 'put-counters', on: self, counter: { type: '+1/+1' }, count: n(1) }] }, { steps: [{ effect: 'untap', what: { ref: 'it' } }] }],
    }]);
  });
  it('keeps shared-subject event alternatives together when qualified by one while clause (Burning Sun Cavalry)', () => {
    const condition: Condition = { condition: 'control', who: you, what: { ref: 'filter', filter: { subtypes: ['Dinosaur'] }, count: n(1) } };
    assert.deepEqual(parse('Whenever this creature attacks or blocks while you control a Dinosaur, this creature gets +1/+1 until end of turn.').abilities, [
      ability(gate({ trigger: 'any-of', events: [attack, { trigger: 'blocks', what: self }] }, condition), [{ effect: 'pump', what: self, power: { sign: 1, amount: n(1) }, toughness: { sign: 1, amount: n(1) }, duration: { duration: 'end-of-turn' } }]),
    ]);
  });
  it('preserves a source-in-graveyard gate separately from the dying event object (Boneyard Scourge)', () => {
    const text = "Whenever a Dragon you control dies while this card is in your graveyard, you may pay {1}{B}. If you do, return this card from your graveyard to the battlefield.";
    assert.deepEqual(parse(text).abilities, [{ kind: 'triggered', abilityWord: undefined, condition: undefined,
      trigger: gate({ trigger: 'dies', what: { ref: 'filter', filter: { subtypes: ['Dragon'], control: 'you' }, count: n(1) } }, { condition: 'is-filter', what: { ref: 'this', noun: 'card' }, filter: { zone: { zone: 'graveyard', owner: 'your' } } }),
      effects: [{ steps: [{ effect: 'pay-cost', who: you, costs: [{ cost: 'mana', symbols: ['1', 'B'] }], optional: true }] },
        { condition: { condition: 'you-do' }, conditionKind: 'if', steps: [{ effect: 'move-zone', what: { ref: 'this', noun: 'card' }, from: { zone: 'graveyard', owner: 'your' }, to: { zone: 'battlefield' } }] }],
    }]);
  });
  it('distinguishes while gates, intervening-if checks, and their conjunction on one printed ability', () => {
    assert.deepEqual(parse('Whenever this creature attacks while you control a creature, draw a card.').abilities, [ability(gate(attack, controlCreature), [draw])]);
    assert.deepEqual(parse('Whenever this creature attacks, if you control a creature, draw a card.').abilities, [ability(attack, [draw], controlCreature)]);
    assert.deepEqual(parse('Whenever this creature attacks while saddled, if you control a creature, draw a card.').abilities, [ability(saddled(attack), [draw], controlCreature)]);
    assert.deepEqual(parse('Whenever this creature attacks while you control a creature and you have no cards in hand, draw a card.').abilities, [ability(gate(attack, { condition: 'all-of', conditions: [controlCreature, { condition: 'cards-in-hand', who: you, comparison: { op: 'eq', value: n(0) } }] }), [draw])]);
  });
  it('does not apply a singular subject-status gate to a grouped attack event', () => {
    assert.deepEqual(parse('Whenever a creature you control attacks while saddled, draw a card.').abilities, [
      ability(saddled({ trigger: 'attacks', what: { ref: 'filter', filter: { types: ['creature'], control: 'you' }, count: n(1) }, alone: undefined, whom: undefined }), [draw]),
    ]);
    // A player-state condition can qualify a group without choosing an
    // individual participant; saddle eligibility cannot use that same shape.
    assert.deepEqual(parse('Whenever one or more creatures you control attack while you control a creature, draw a card.').abilities, [
      ability(gate({ trigger: 'attacks', what: { ref: 'filter', filter: { types: ['creature'], control: 'you' }, count: n(1) }, alone: undefined, whom: undefined, grouping: 'one-or-more' }, controlCreature), [draw]),
    ]);
    for (const text of [
      'Whenever one or more creatures you control attack while saddled, draw a card.',
      'When this creature enters and whenever one or more creatures you control attack while saddled, draw a card.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
    const c = new Cursor(lex('while saddled'));
    assert.equal(parseTriggerGate(c, { ...attack, grouping: 'one-or-more' }), null);
    assert.equal(c.pos, 0);
  });
  it('retains exact original body target provenance without replacing symbolic gate references', () => {
    const result = parse('Whenever this creature attacks while this creature is tapped, tap target creature.', true);
    assert.deepEqual(result.abilities, [ability(gate(attack, { condition: 'status', what: self, status: 'tapped' }), [{ effect: 'tap', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:11' } }])]);
    const parsed = parse('Whenever you cast a spell while this card is exiled, put a stall counter on it.');
    assert.deepEqual(parsed.abilities, [ability(gate({ trigger: 'cast', who: you, what: { cls: 'spell' } }, { condition: 'is-filter', what: { ref: 'this', noun: 'card' }, filter: { zone: { zone: 'exile' } } }), [{ effect: 'put-counters', on: { ref: 'it' }, counter: { type: 'stall' }, count: n(1) }])]);
  });
  it('does not widen scoped copy-it proofs or accept unsupported context, ordinal, or status qualifiers', () => {
    for (const text of [
      'Whenever you cast a spell while you control a creature, copy it.',
      'Whenever this creature attacks while target creature is tapped, draw a card.',
      'Whenever this creature attacks while saddled and renowned, draw a card.',
      'Whenever you attack while saddled, draw a card.', 'Whenever this creature attacks while tapped sideways, draw a card.',
      'Whenever this creature attacks for the first time each turn while you control a creature, draw a card.',
      'Whenever this creature attacks while you control a creature for the first time each turn, draw a card.',
      'When you sacrifice this creature while casting a spell with emerge, draw a card.',
      'Whenever this creature attacks while you control a creature or have no cards in hand and you control a land, draw a card.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
    const c = new Cursor(lex('while inventing a mechanic'));
    assert.equal(parseTriggerGate(c, attack), null);
    assert.equal(c.pos, 0);
    const header = new Cursor(lex('Whenever this creature attacks while inventing a mechanic'));
    assert.equal(parseTrigger(header), null);
    assert.equal(header.pos, 0);
  });
  it('keeps event snapshot and subject-status scopes explicitly unresolved by the binder', () => {
    const result = parse('Whenever this creature attacks while saddled, tap target creature.', true);
    const analysis = analyzeCardBindings({ identity: 'trigger-fixture', source: { name: 'Trigger fixture', layout: 'normal', type_line: 'Creature — Beast' }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(analysis.fullyBound, false);
    assert.ok(analysis.diagnostics.some(d => d.code === 'unsupported-scope' && /event-time conditions/.test(d.message)));
  });
});
