import assert from 'node:assert/strict';
import test from 'node:test';
import type { DecisionContext } from '@multiverse/game-server-sdk';
import { completeDecision, decisionSurface, draftError, emptyDraft, reconcileDraft, setDraftValue, type DecisionRequest, type LocalDraft } from '../web/src/decisions/controls.ts';

// Synthetic structural requests/context only, with no card facts or admission claim.
const base = { player: 0, token: 'synthetic' };
const labels = [{ handle: 'a', name: 'Same label' }, { handle: 'b', name: 'Same label' }];
const source = { kind: 'spell', oracle_id: 'synthetic-source', text: 'Synthetic complete source block' } as const;
const context: DecisionContext = { source, instruction: { source_block: 0, text: 'Synthetic current instruction' } };
const values = (...rows: readonly (readonly [string, string])[]): LocalDraft => ({ ...emptyDraft, values: rows });

test('combat declaration requires each explicit choice, preserves exact candidates and never supplies automatic attacks', () => {
  const request: DecisionRequest = Object.freeze({ ...base, request: 'attackers', candidates: ['a', 'b'], defenders: [1, 2] });
  assert.equal(completeDecision(request, labels, emptyDraft), null);
  assert.deepEqual(completeDecision(request, labels, values(['a', '[1]'], ['b', 'unused'])), { response: 'attackers', attacks: [['a', 1]] });
  assert.deepEqual(completeDecision(request, labels, values(['a', 'unused'], ['b', 'unused'])), { response: 'attackers', attacks: [] });
  for (const draft of [values(['a', '[0]'], ['b', 'unused']), values(['a', '[1]'], ['a', '[2]']), values(['a', '[1]'], ['b', 'unused'], ['foreign', 'unused'])]) assert.equal(completeDecision(request, labels, draft), null);
  const surface = decisionSurface(request, labels);
  assert.equal(surface.status, 'supported');
  if (surface.status !== 'supported') throw new Error('Expected declaration');
  assert.deepEqual(reconcileDraft(values(['a', '[1]'], ['foreign', '[2]']), surface.control).values, [['a', '[1]']]);
});

test('block declaration never falls back from missing supplied eligibility to all attackers', () => {
  const request: DecisionRequest = { ...base, request: 'blockers', candidates: ['a'], attackers: ['b', 'c'], legal_attackers: [{ blocker: 'a', attackers: ['b'] }], menace_attackers: ['b'] };
  assert.deepEqual(completeDecision(request, labels, values(['a', '["b"]'])), { response: 'blockers', blocks: [['a', 'b']] });
  assert.equal(completeDecision(request, labels, values(['a', '["c"]'])), null);
  assert.equal(decisionSurface({ ...request, legal_attackers: [] }, labels).status, 'context-missing');
  assert.equal(decisionSurface({ ...request, legal_attackers: [{ blocker: 'a', attackers: ['b'] }, { blocker: 'a', attackers: ['c'] }] }, labels).status, 'context-missing');
});

test('supplied payment quantities remain exact, explicit and bounded without deriving payment legality', () => {
  const request: DecisionRequest = { ...base, request: 'spend_mana', cost: { generic: '9007199254740993', colored: ['U'] }, available: { U: '1', C: '9007199254740993' } };
  assert.equal(completeDecision(request, labels, values(['U', '1'], ['C', '9007199254740993'])), null);
  assert.equal(completeDecision(request, labels, values(['U', '1']), context), null);
  assert.deepEqual(completeDecision(request, labels, values(['U', '1'], ['C', '9007199254740993']), context), { response: 'spend_mana', spending: { U: '1', C: '9007199254740993' } });
  assert.deepEqual(completeDecision(request, labels, values(['U', '1'], ['C', '9007199254740993']), { source: { ...source, text: '' } }), { response: 'spend_mana', spending: { U: '1', C: '9007199254740993' } });
  assert.deepEqual(completeDecision({ ...request, cost: { generic: '1', colored: [] } }, labels, values(['U', '0'], ['C', '1']), context), { response: 'spend_mana', spending: { C: '1' } });
  for (const amount of ['9007199254740994', '01', '1e20', '1\n', '-1']) assert.equal(completeDecision(request, labels, values(['U', '1'], ['C', amount]), context), null);
});

test('combat allocation conserves supplied exact total and keeps handle identities separate from player fields', () => {
  const request: DecisionRequest = { ...base, request: 'damage', source: 'a', power: '9007199254740993', recipients: ['player'], player_recipient: 2, lethal_requirements: [['player', '2']] };
  assert.equal(completeDecision(request, labels, values(['["player"]', '2'], ['player', '9007199254740990'])), null);
  assert.deepEqual(completeDecision(request, labels, values(['["player"]', '2'], ['player', '9007199254740991'])), { response: 'trample_damage', assignments: [['player', '2']], player: 2, amount: '9007199254740991' });
  assert.equal(completeDecision(request, labels, values(['["player"]', '2'], ['player', '9007199254740991'], ['extra', '0'])), null);
});

test('semantic choices require the corresponding producer context and preserve original exact values', () => {
  const x: DecisionRequest = { ...base, request: 'announce_x' };
  assert.equal(completeDecision(x, [], { ...emptyDraft, quantity: '9007199254740993' }), null);
  assert.deepEqual(completeDecision(x, [], { ...emptyDraft, quantity: '9007199254740993' }, context), { response: 'announce_x', value: '9007199254740993' });
  assert.equal(completeDecision(x, [], { ...emptyDraft, quantity: '2' }, { source: { ...source, kind: 'activated' } }), null);
  const optional: DecisionRequest = { ...base, request: 'optional_effect' };
  assert.equal(completeDecision(optional, [], { ...emptyDraft, selected: ['perform'] }, { source }), null);
  assert.deepEqual(completeDecision(optional, [], { ...emptyDraft, selected: ['decline'] }, context), { response: 'optional_effect', perform: false });
  const mode: DecisionRequest = { ...base, request: 'modes', candidates: [0, 2] };
  assert.equal(completeDecision(mode, [], { ...emptyDraft, selected: ['2'] }, context), null);
  const modes = [{ mode: 0, text: 'Synthetic first mode' }, { mode: 2, text: 'Synthetic second mode' }];
  assert.deepEqual(completeDecision(mode, [], { ...emptyDraft, selected: ['2'] }, { ...context, modes }), { response: 'modes', mode: 2 });
  assert.equal(completeDecision(mode, [], { ...emptyDraft, selected: ['2'] }, { ...context, modes: [...modes, modes[1]!] }), null);
  for (const constraint of [{ one_or_more: true }, { exactly: 2 }, { allow_repeat: true }]) {
    assert.equal(completeDecision({ ...mode, ...constraint }, [], { ...emptyDraft, selected: ['2'] }, { ...context, modes }), null);
  }
});

test('ordered mixed targets distinguish object and player identity and do not inspect target rules', () => {
  const request: DecisionRequest = { ...base, request: 'entity_targets', candidates: [{ entity: 'player', player: 1 }, { entity: 'object', object: '1' }], occurrences: [{ chooser: 'controller', domain: 'battlefield_creature', selection_id: 's', slot: 0, stage: 'announcement' }, { chooser: 'controller', domain: 'battlefield_creature', selection_id: 's', slot: 1, stage: 'announcement' }] };
  const draft = values(['0', '["1"]'], ['1', '[1]']);
  assert.equal(completeDecision(request, [], draft), null);
  assert.deepEqual(completeDecision(request, [], draft, context), { response: 'entity_targets', entities: [{ entity: 'object', object: '1' }, { entity: 'player', player: 1 }] });
  assert.deepEqual(completeDecision(request, [], draft, { source }), { response: 'entity_targets', entities: [{ entity: 'object', object: '1' }, { entity: 'player', player: 1 }] });
});

test('field reconciliation removes retired candidate choices and ambiguity fails closed', () => {
  const control = { kind: 'mapping', title: 'Synthetic', description: '', rows: [{ id: 'a', label: 'a', detail: '', options: [{ id: 'x', label: 'x', detail: '' }] }] } as const;
  const draft = setDraftValue(emptyDraft, 'a', 'x');
  Object.freeze(draft.values); Object.freeze(draft);
  assert.equal(draftError(control, draft), null);
  assert.deepEqual(reconcileDraft(draft, { ...control, rows: [{ ...control.rows[0], options: [] }] }).values, []);
  assert.match(draftError({ ...control, rows: [...control.rows, ...control.rows] }, draft)!, /ambiguous/);
  assert.deepEqual(setDraftValue(draft, 'a', 'y').values, [['a', 'y']]);
  assert.deepEqual(draft.values, [['a', 'x']]);
});

test('replacement-effect choices require per-occurrence labels and retain the exact effect/damage pair', () => {
  const candidate = { amount_before: '4', amount_after: '8', damage: 'd1', damage_oracle_id: 'synthetic-damage', damage_source: { announced: 'a', current: 'a' }, effect: 'e1', effect_controller: 0, effect_oracle_id: 'synthetic-effect', effect_source: { announced: 'b', current: 'b' }, kind: 'double_damage' as const };
  const request: DecisionRequest = { ...base, request: 'damage_effect', recipient: { entity: 'player', player: 0 }, candidates: [{ ...candidate, label: 'Synthetic exact effect source block' }, { ...candidate, damage: 'd2', label: 'Synthetic exact effect source block' }] };
  const missingLabel: DecisionRequest = { ...base, request: 'damage_effect', recipient: { entity: 'player', player: 0 }, candidates: [candidate] };
  assert.equal(decisionSurface(missingLabel, []).status, 'context-missing');
  assert.deepEqual(completeDecision(request, labels, { ...emptyDraft, selected: ['["e1","d2"]'] }), { response: 'damage_effect', effect: 'e1', damage: 'd2' });
  assert.equal(completeDecision(request, labels, { ...emptyDraft, selected: ['["foreign","d2"]'] }), null);
});
