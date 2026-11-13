import assert from 'node:assert/strict';
import test from 'node:test';
import { GenerationFence } from '../src/session/generation.ts';
import { applyPublication, type Projection } from '../src/session/projection.ts';

test('same authorization start is idempotent; stop/restart fences late work', async () => {
  const fence = new GenerationFence();
  const authorization = { identity: {} };
  const first = fence.start(authorization);
  assert.equal(fence.start(authorization), first);
  let finish!: () => void;
  let applied = false;
  const pending = new Promise<void>(resolve => { finish = resolve; })
    .then(() => { if (fence.isCurrent(first)) applied = true; });
  fence.stop(); fence.stop();
  const restarted = fence.start(authorization);
  finish(); await pending;
  assert.equal(applied, false);
  assert.equal(fence.isCurrent(first), false);
  assert.equal(fence.isCurrent(restarted), true);
});

test('seat switch and same-seat reauthorization both invalidate outstanding generations', () => {
  const fence = new GenerationFence();
  const identity = {};
  const old = fence.start({ identity });
  const reimport = fence.start({ identity });
  assert.equal(fence.isCurrent(old), false);
  const otherSeat = fence.start({ identity: {} });
  assert.equal(fence.isCurrent(reimport), false);
  assert.equal(fence.isCurrent(otherSeat), true);
});

test('lifecycle and outcome advance cursor without deleting the last permitted observation', () => {
  const view = Object.freeze({ permitted: 'fixture' });
  const initial: Projection<typeof view, string, string, string> = Object.freeze({ cursor: '2', observation: view, lifecycle: null, outcome: null });
  const paused = applyPublication(initial, { kind: 'lifecycle', cursor: '3', value: 'resource-paused' });
  assert.equal(paused.observation, view);
  assert.equal(paused.outcome, null);
  assert.equal(paused.cursor, '3');
  const ended = applyPublication(paused, { kind: 'outcome', cursor: '4', value: 'source-provided-result' });
  assert.equal(ended.observation, view);
  assert.equal(initial.lifecycle, null);
  const nextView = { permitted: 'replacement' };
  assert.equal(applyPublication(ended, { kind: 'observation', cursor: '5', value: nextView }).observation, nextView);
});
