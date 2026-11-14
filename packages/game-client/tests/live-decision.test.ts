import assert from 'node:assert/strict';
import test from 'node:test';
import { bindingKey } from '../src/decisions/controls.ts';
import { liveDecisionBinding, originalDecisionIntent, type LiveDecisionDraft } from '../src/session/live-decision.ts';
import { actionDescription, actionResponse } from '../src/session/live-action.ts';
import type { ActionBeginning, ActionPage } from '@multiverse/game-server-sdk';

const draft: LiveDecisionDraft = {
  context: { binding: { origin: 'http://127.0.0.1:18741', match: 'a1', seat: '0', engineBuild: 'engine', serverBuild: 'server', catalog: 'catalog', protocol: 'schema' }, incarnation: 7, revision: '9007199254740993', token: 'decision' },
  request: { request: 'discard', token: 'decision', player: 0, count: 1, candidates: ['a'] },
};
test('general decision completion retains the original full command context and rejects foreign binding', () => {
  const binding = liveDecisionBinding(draft); assert.ok(binding);
  const completion = { binding, response: { response: 'discard', objects: ['a'] } as const };
  const intent = originalDecisionIntent(draft, { ...completion, response: { response: 'discard', objects: ['a'] } });
  assert.ok(intent); assert.equal(intent.context, draft.context);
  assert.equal(intent.context.revision, '9007199254740993');
  for (const patch of [{ origin: 'http://127.0.0.1:9' }, { match: 'b1' }, { seat: 1 }, { incarnation: 8 }, { revision: '9007199254740994' }, { token: 'next' }]) {
    assert.notEqual(bindingKey({ ...binding, ...patch }), bindingKey(binding));
    assert.equal(originalDecisionIntent(draft, { binding: { ...binding, ...patch }, response: { response: 'discard', objects: ['a'] } }), null);
  }
  assert.equal(liveDecisionBinding({ ...draft, request: { ...draft.request, token: 'next' } }), null);
  assert.equal(liveDecisionBinding({ ...draft, request: { ...draft.request, player: 1 } }), null);
});

const priority: ActionPage['context'] = { window: 'priority' };
const mana: ActionPage['context'] = { window: 'mana', cost: { generic: '1', colored: [] }, source: { status: 'available', object: 'spell' } };
test('source-bound action beginnings preserve exact payloads and scope without inferring affordability', () => {
  const cases: [ActionBeginning, ActionPage['context'], unknown][] = [
    [{ initiation: { action: 'play_land', object: 'a' }, label: { available: { text: 'Play land' } } }, priority, { response: 'play_land', object: 'a' }],
    [{ initiation: { action: 'cast', card: 'a' }, label: { available: { text: 'Cast spell', source: { kind: 'spell', oracle_id: 'synthetic', object: 'a', text: '' } } } }, priority, { response: 'cast', card: 'a' }],
    [{ initiation: { action: 'activate_mana', object: 'a' }, label: { available: { text: 'Activate mana ability', source: { kind: 'mana_ability', oracle_id: 'synthetic', object: 'a', text: 'Synthetic bound mana source' } } } }, mana, { response: 'activate_mana', object: 'a' }],
    [{ initiation: { action: 'activate_ability', object: 'a', ability: 2 }, label: { available: { text: 'Activate ability', source: { kind: 'activated', oracle_id: 'synthetic', object: 'a', ability: 2, text: 'Synthetic bound activated source' } } } }, priority, { response: 'activate_ability', object: 'a', ability: 2 }],
  ];
  for (const [beginning, context, response] of cases) {
    assert.deepEqual(actionResponse(beginning, context, { scope: 'window' }), response);
    assert.deepEqual(actionResponse(beginning, context, { scope: 'object', object: 'a' }), response);
    assert.equal(actionResponse(beginning, context, { scope: 'object', object: 'foreign' }), null);
    assert.equal(actionResponse({ ...beginning, label: 'unavailable' }, context, { scope: 'window' }), null);
    assert.match(actionDescription(beginning, [{ handle: 'a', name: 'Synthetic object' }]), /Synthetic object \(a\)/);
  }
  const unknownMana: ActionBeginning = { initiation: { action: 'activate_mana', object: 'a' }, label: { available: { text: 'Activate mana ability' } } };
  assert.equal(actionResponse(unknownMana, mana, { scope: 'window' }), null);
  const mismatch: ActionBeginning = { initiation: { action: 'activate_ability', object: 'a', ability: 2 }, label: { available: { text: 'Activate ability', source: { kind: 'activated', oracle_id: 'synthetic', object: 'a', ability: 1, text: 'Other ability' } } } };
  assert.equal(actionResponse(mismatch, priority, { scope: 'window' }), null);
});

test('both explicit finish-mana flags continue payment and preserve producer-owned failure policy', () => {
  for (const undo_mana of [false, true]) {
    const beginning: ActionBeginning = { initiation: { action: 'finish_mana', undo_mana }, label: { available: { text: undo_mana ? 'Finish adding mana; reverse reversible mana if payment fails' : 'Finish adding mana; keep mana if payment fails' } } };
    assert.deepEqual(actionResponse(beginning, mana, { scope: 'window' }), { response: 'finish_mana', undo_mana });
    assert.equal(actionResponse(beginning, priority, { scope: 'window' }), null);
    assert.equal(actionResponse(beginning, { ...mana!, window: 'mana', source: { status: 'unavailable' } }, { scope: 'window' }), null);
    assert.equal(actionResponse(beginning, mana, { scope: 'object', object: 'spell' }), null);
    assert.doesNotMatch(actionDescription(beginning), /cancel/i);
  }
});
