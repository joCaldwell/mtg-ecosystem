import assert from 'node:assert/strict';
import test from 'node:test';
import { bindingKey } from '../web/src/decisions/controls.ts';
import { liveDecisionBinding, originalDecisionIntent, type LiveDecisionDraft } from '../web/src/session/live-decision.ts';

const draft: LiveDecisionDraft = {
  context: { binding: { origin: 'http://127.0.0.1:18741', match: 'a1', seat: '0', engineBuild: 'engine', serverBuild: 'server', catalog: 'catalog', protocol: 'schema' }, incarnation: 7, revision: '9007199254740993', token: 'decision' },
  request: { reason:{reason:'discard'},token:'decision',player:0, candidates:[{candidate:'object',object:'a'}],shape:{shape:'pick',slots:[{role:{role:'choice'},subject:null,allowed:[0],min:1,max:1,repeat:false}],rules:[]} },
};
test('general decision completion retains the original full command context and rejects foreign binding', () => {
  const binding = liveDecisionBinding(draft); assert.ok(binding);
  const completion = { binding, response: { answer:'pick',picks:[[0]] } as const };
  const intent = originalDecisionIntent(draft, { ...completion, response: { answer:'pick',picks:[[0]] } });
  assert.ok(intent); assert.equal(intent.context, draft.context);
  assert.equal(intent.context.revision, '9007199254740993');
  for (const patch of [{ origin: 'http://127.0.0.1:9' }, { match: 'b1' }, { seat: 1 }, { incarnation: 8 }, { revision: '9007199254740994' }, { token: 'next' }]) {
    assert.notEqual(bindingKey({ ...binding, ...patch }), bindingKey(binding));
    assert.equal(originalDecisionIntent(draft, { binding: { ...binding, ...patch }, response: { answer:'pick',picks:[[0]] } }), null);
  }
  assert.equal(liveDecisionBinding({ ...draft, request: { ...draft.request, token: 'next' } }), null);
  assert.equal(liveDecisionBinding({ ...draft, request: { ...draft.request, player: 1 } }), null);
});
