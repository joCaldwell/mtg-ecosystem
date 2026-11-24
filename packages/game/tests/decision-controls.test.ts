import assert from 'node:assert/strict';
import test from 'node:test';
import { bindingKey, completeDecision, draftError, emptyDraft, reconcileDraft, reorderDraft, toggleDraft, type DecisionBinding, type DecisionRequest, type LocalControl } from '../web/src/decisions/controls.ts';

// All requests in this file are synthetic structural samples. They contain no
// source/card facts and prove no hosted-profile reachability or semantic legality.
const base = { player: 0, token: 'synthetic-token' };
const binding: DecisionBinding = { origin: 'fixture:lab', match: 'synthetic', seat: 0, token: base.token, incarnation: 1, revision: '9007199254740993' };
const labels = Object.freeze([{ handle: 'copy-a', name: 'Same label' }, { handle: 'copy-b', name: 'Same label' }]);
const select: LocalControl = { shape:{shape:'order',items:[0,1],min:2,max:2}, kind: 'selection', title: 'Structural', description: '', minimum: 2, maximum: 2, ordered: true, options: labels.map(value => ({ id: value.handle, label: value.name, detail: value.handle })) };

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

test('one indexed choice uses the declared slot and ignores display labels', () => {
  const decision: DecisionRequest = { ...base, reason:{reason:'mulligan',mulligans_taken:0,bottom_if_mulligan:0}, candidates:[{candidate:'flag',flag:'keep'},{candidate:'flag',flag:'mulligan'}],
    shape:{shape:'pick',slots:[{role:{role:'choice'},subject:null,allowed:[1,0],min:1,max:1,repeat:false}],rules:[]} };
  assert.equal(completeDecision(decision, [], emptyDraft),null);
  assert.deepEqual(completeDecision(decision, [], {...emptyDraft, selected:['1']}),{answer:'pick',picks:[[1]]});
  assert.equal(completeDecision(decision, [], {...emptyDraft, selected:['5']}),null);
  const order: DecisionRequest = {...decision,shape:{shape:'order',items:[0,1],min:2,max:2}};
  assert.deepEqual(completeDecision(order, [], {...emptyDraft,selected:['1','0']}),{answer:'order',order:[1,0]});
  assert.equal(completeDecision(order, [], {...emptyDraft,selected:['0','0']}),null);
});
test('number preserves canonical quantities and enforces the declared inclusive bounds', () => {
  const d: DecisionRequest = {...base,reason:{reason:'announce_x'},candidates:[],shape:{shape:'number',min:'2',max:'9007199254740993'}};
  const complete=(quantity:string)=>completeDecision(d,[],{...emptyDraft,quantity});
  assert.deepEqual(complete('9007199254740993'),{answer:'number',value:'9007199254740993'});
  for (const value of ['', '0','-1','01','1e20','9007199254740994']) assert.equal(complete(value),null);
});
