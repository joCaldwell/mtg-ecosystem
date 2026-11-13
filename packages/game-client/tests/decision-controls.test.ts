import assert from 'node:assert/strict';
import test from 'node:test';
import { bindingKey, completeDecision, decisionSurface, draftError, emptyDraft, reconcileDraft, reorderDraft, toggleDraft, type DecisionBinding, type DecisionRequest, type LocalControl } from '../src/decisions/controls.ts';

// All requests in this file are synthetic structural samples. They contain no
// source/card facts and prove no hosted-profile reachability or semantic legality.
const base = { player: 0, token: 'synthetic-token' };
const binding: DecisionBinding = { origin: 'fixture:lab', match: 'synthetic', seat: 0, token: base.token, incarnation: 1, revision: '9007199254740993' };
const labels = Object.freeze([{ handle: 'copy-a', name: 'Same label' }, { handle: 'copy-b', name: 'Same label' }]);
const select: LocalControl = { kind: 'selection', title: 'Structural', description: '', minimum: 2, maximum: 2, ordered: true, options: labels.map(value => ({ id: value.handle, label: value.name, detail: value.handle })) };

test('decision binding fences origin, match, seat, token, incarnation and revision', () => {
  for (const changed of [{ origin: 'fixture:other' }, { match: 'other' }, { seat: 1 }, { token: 'next' }, { incarnation: 2 }, { revision: '9007199254740994' }]) {
    assert.notEqual(bindingKey({ ...binding, ...changed }), bindingKey(binding));
  }
  assert.equal(bindingKey({ ...binding }), bindingKey(binding));
});

test('explicit membership, count, same-label identity and ordering preserve immutable intent', () => {
  assert.equal(toggleDraft(select, emptyDraft, 'foreign'), emptyDraft);
  const first = toggleDraft(select, emptyDraft, 'copy-a');
  assert.equal(draftError(select, first), 'Choose 2 item(s).');
  const both = toggleDraft(select, first, 'copy-b');
  Object.freeze(both.selected); Object.freeze(both);
  assert.equal(draftError(select, both), null);
  assert.deepEqual(reorderDraft(both, 'copy-b', -1).selected, ['copy-b', 'copy-a']);
  assert.deepEqual(both.selected, ['copy-a', 'copy-b']);
  assert.equal(reorderDraft(both, 'foreign', 1), both);
  assert.equal(reorderDraft(both, 'copy-a', -1), both);
  assert.match(draftError(select, { ...both, selected: ['copy-a', 'copy-a'] })!, /no longer/);
  assert.match(draftError({ ...select, options: [select.options[0]!, select.options[0]!] }, first)!, /ambiguous/);
  assert.match(draftError({ ...select, maximum: Number.MAX_SAFE_INTEGER + 1 }, both)!, /count/);
  const replacement = { ...select, options: [{ id: 'copy-new', label: 'Same label', detail: '' }, select.options[1]!] };
  assert.deepEqual(reconcileDraft(both, replacement).selected, ['copy-b']);
});

test('mulligan requires explicit confirmation and never invents unavailable options', () => {
  const request = Object.freeze({ ...base, request: 'mulligan' as const, mulligans_taken: 0, bottom_if_mulligan: 0, can_mulligan: true });
  assert.equal(completeDecision(request, [], emptyDraft), null);
  assert.deepEqual(completeDecision(request, [], { ...emptyDraft, selected: ['keep'] }), { response: 'mulligan', take: false });
  assert.deepEqual(completeDecision(request, [], { ...emptyDraft, selected: ['mulligan'] }), { response: 'mulligan', take: true });
  assert.equal(completeDecision({ ...request, can_mulligan: false }, [], { ...emptyDraft, selected: ['mulligan'] }), null);
  assert.equal(completeDecision(request, [], { ...emptyDraft, selected: ['false'] }), null);
});

test('implemented handle adapters preserve exact supplied response fields without rule inference', () => {
  const cases: readonly DecisionRequest[] = [
    { ...base, request: 'bottom_opening', candidates: ['copy-a', 'copy-b'], count: 2 },
    { ...base, request: 'discard', candidates: ['copy-a', 'copy-b'], count: 2 },
    { ...base, request: 'resolve_discard', candidates: ['copy-a', 'copy-b'], count: 2 },
    { ...base, request: 'search_lands', candidates: [{ handle: 'copy-a', oracle_id: 'synthetic-not-card-data' }], maximum: 2 },
    { ...base, request: 'legend_rule', candidates: ['copy-a'], previous_choices: [] },
    { ...base, request: 'sacrifice_cost', candidates: ['copy-a'] },
    { ...base, request: 'forced_sacrifice', candidates: ['copy-a'], previous_choices: [] },
    { ...base, request: 'sacrifice', candidates: ['copy-a'] },
  ];
  for (const request of cases) {
    const surface = decisionSurface(request, labels);
    assert.equal(surface.status, 'supported');
    if (surface.status !== 'supported' || surface.control.kind !== 'selection') throw new Error('Expected selection');
    const selected = surface.control.options.slice(0, surface.control.maximum).map(value => value.id).reverse();
    const result = completeDecision(request, labels, { ...emptyDraft, selected });
    assert.equal(result?.response, request.request);
    if (result && 'objects' in result) assert.deepEqual(result.objects, selected);
    assert.equal(completeDecision(request, labels, { ...emptyDraft, selected: ['foreign'] }), null);
    assert.equal(surface.control.options[0]?.detail, 'copy-a');
  }
  const missing = decisionSurface(cases[0]!, []);
  assert.equal(missing.status === 'supported' && missing.control.kind === 'selection' && missing.control.options[0]?.label, 'Unidentified object');
});

test('missing source semantics block payload construction, including otherwise simple quantities and booleans', () => {
  for (const request of [
    { ...base, request: 'announce_x' }, { ...base, request: 'optional_effect' }, { ...base, request: 'optional_recursion' },
    { ...base, request: 'modes', candidates: [0, 1] }, { ...base, request: 'priority' },
  ] satisfies DecisionRequest[]) {
    assert.equal(decisionSurface(request, []).status, 'context-missing');
    assert.equal(completeDecision(request, [], { selected: ['0'], quantity: '9007199254740993' }), null);
  }
});

test('exact quantity controls keep canonical strings beyond Number precision', () => {
  const control: LocalControl = { kind: 'quantity', title: 'Synthetic', description: '', maxDigits: 80 };
  assert.equal(draftError(control, { selected: [], quantity: '900719925474099312345678901234567890' }), null);
  for (const value of ['', '-1', '1e20', '01', '1\n', ' '.repeat(2), '9'.repeat(81)]) assert.notEqual(draftError(control, { selected: [], quantity: value }), null);
});
