import assert from 'node:assert/strict';
import test from 'node:test';
import { GameClient, prepareCommand, parsePreparedCommand } from '@multiverse/game-server-sdk/browser';
import type { CommandEnvelope, CommandExecution, SubmissionResult } from '@multiverse/game-server-sdk';
import type { CommandJournal, JournalRecord } from '../src/session/contracts.ts';
import { CommandLifecycle, type CommandContext, type CommandEffects, type CommandSnapshot, type DecisionIntent } from '../src/session/command.ts';
import { JournalFailure } from '../src/session/journal.ts';

// Synthetic command transport/status doubles, not native legality evidence.
// Decision vocabulary is Rust-generated; actual host actions use producer fixtures.
const binding = { origin: 'http://127.0.0.1:18741', match: 'a1', seat: '0', engineBuild: '1'.repeat(64), serverBuild: '2'.repeat(64), catalog: '3'.repeat(64), protocol: '4'.repeat(64) };
const codec = { prepare: prepareCommand, parse: parsePreparedCommand };
const context: CommandContext = { binding, incarnation: 1, revision: '1', token: 'v0-d0' };
const intent: DecisionIntent = { kind: 'decision', source: 'human', context, response: { response: 'mulligan', take: false } };
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { resolve, promise }; }
function rig(initial: JournalRecord<CommandEnvelope> | null = null) {
  let record = initial, active = true, refreshed = 0, newIds = 0;
  let current: CommandContext | null = context;
  let state: CommandSnapshot | null = null;
  let onPublish: (value: CommandSnapshot) => void = () => {};
  const events: string[] = [], bodies: string[] = [];
  const journal: CommandJournal<CommandEnvelope> = {
    async unresolved(seat) { events.push('scan'); assert.equal(seat.seat, binding.seat); return record ? [record] : []; },
    async persist(value) { events.push('persist'); assert.equal(record, null); record = value; },
    async resolve(value, disposition) { events.push(`resolve:${disposition}`); assert.equal(record, value); record = null; },
  };
  const effects: CommandEffects = { journal, codec, protocol: binding.protocol, protocolVersion: 1,
    newId: () => (++newIds).toString(16).padStart(32, '0'), transport: {
      async submitExact(body) { events.push('send'); bodies.push(body); assert.equal(record?.exactBody, body); return { status: 'accepted', command_id: record.envelope.command_id }; },
      async execution(id) { events.push('execution'); return { command_id: id, status: { status: 'pending' } }; },
      async cancelQueued(id) { events.push('cancel'); return { command_id: id, status: 'accepted' }; },
    } };
  const lifecycle = new CommandLifecycle({ effects, binding, signal: new AbortController().signal, active: () => active,
    context: () => current, publish: value => { state = value; onPublish(value); }, refresh: () => { refreshed++; } });
  return { lifecycle, effects, events, bodies, state: () => state!, record: () => record, refreshes: () => refreshed,
    publish: (handler: (value: CommandSnapshot) => void) => { onPublish = handler; },
    stop: () => { active = false; }, context: (value: CommandContext | null) => { current = value; } };
}
async function pending() { const r = rig(); await r.lifecycle.recover(); await r.lifecycle.submitIntent(intent); return r; }

test('all journal failures fail closed before send; only explicit current human intent persists', async () => {
  for (const kind of ['unavailable', 'blocked', 'full', 'corrupt', 'conflict', 'failed'] as const) {
    const r = rig(); await r.lifecycle.recover();
    r.effects.journal.persist = async () => { throw new JournalFailure(kind); };
    await r.lifecycle.submitIntent(intent);
    assert.equal(r.state().failure, kind); assert.equal(r.bodies.length, 0);
  }
  const r = rig(); await r.lifecycle.recover();
  for (const patch of [{ incarnation: 2 }, { revision: '2' }, { token: 'other' }, { binding: { ...binding, seat: '1' } }]) {
    await r.lifecycle.submitIntent({ ...intent, context: { ...context, ...patch } });
  }
  await r.lifecycle.submitIntent({ ...intent, source: 'policy' } as unknown as DecisionIntent);
  assert.deepEqual(r.events, ['scan']);
});

test('atomic persistence precedes actual SDK transport bytes; duplicate clicks cannot replace', async () => {
  const r = rig(), commit = deferred<void>(); await r.lifecycle.recover();
  const persist = r.effects.journal.persist;
  r.effects.journal.persist = async record => { await commit.promise; await persist(record); };
  const bodies: string[] = [];
  const sdk = new GameClient({ baseURL: binding.origin, matchId: binding.match, capability: 'test-only', fetch: async (_url, options) => {
    assert.ok(r.record()); bodies.push(options!.body as string);
    return new Response(JSON.stringify({ command_id: r.record()!.envelope.command_id, status: 'accepted' }), { headers: { 'content-type': 'application/json' } });
  } });
  r.effects.transport.submitExact = (body, signal) => sdk.submitExact(body, { signal });
  const first = r.lifecycle.submitIntent(intent);
  await r.lifecycle.submitIntent(intent); assert.equal(bodies.length, 0);
  commit.resolve(); await first;
  assert.equal(bodies[0], r.record()!.exactBody); assert.deepEqual(r.events, ['scan', 'persist']);
  await r.lifecycle.submitIntent(intent); assert.equal(bodies.length, 1);
});

test('persist-before-send crash recovers exact envelope and revision, including whitespace', async () => {
  const r = rig(); await r.lifecycle.recover();
  const persist = r.effects.journal.persist;
  r.effects.journal.persist = async record => { await persist(record); r.stop(); };
  await r.lifecycle.submitIntent(intent); assert.equal(r.bodies.length, 0);
  const original = r.record()!;
  const exactBody = ` \n${original.exactBody}  `;
  const recovered = rig({ ...original, ...parsePreparedCommand(exactBody) });
  await recovered.lifecycle.recover(); recovered.effects.transport.execution = async () => null;
  await recovered.lifecycle.reconcile();
  assert.equal(recovered.bodies[0], exactBody); assert.equal(recovered.record()!.envelope.expected_revision, '1');
});

test('committed response lost, accepted ack + refresh before consumption, and missing row retain slot', async () => {
  const r = rig(); await r.lifecycle.recover();
  r.effects.transport.submitExact = async () => { throw new Error('body lost'); };
  await r.lifecycle.submitIntent(intent); const record = r.record()!;
  assert.equal(r.state().phase, 'unresolved'); assert.ok(record);
  const next = rig(record); await next.lifecycle.recover(); await next.lifecycle.reconcile();
  assert.equal(next.state().phase, 'unresolved'); assert.equal(next.record(), record);
  next.context({ ...context, revision: '99', token: 'new' });
  await next.lifecycle.submitIntent({ ...intent, context: { ...context, revision: '99', token: 'new' } });
  assert.equal(next.record(), record); assert.equal(next.bodies.length, 0);
  next.effects.transport.execution = async id => ({ command_id: id, status: { status: 'consumed_by_rules' } });
  await next.lifecycle.reconcile(); assert.equal(next.record(), null); assert.equal(next.state().phase, 'awaiting-refresh');
  await next.lifecycle.submitIntent(intent); assert.equal(next.bodies.length, 0);
  next.lifecycle.refreshed(); assert.equal(next.state().phase, 'ready');
});

test('delayed original arrival and absent execution exact retry reach same actual transport bytes', async () => {
  const r = rig(); await r.lifecycle.recover(); const original = deferred<SubmissionResult>();
  r.effects.transport.submitExact = async body => { r.bodies.push(body); return original.promise; };
  const sending = r.lifecycle.submitIntent(intent); await Promise.resolve(); await Promise.resolve();
  const record = r.record()!; assert.ok(record); r.stop();
  const next = rig(record); await next.lifecycle.recover(); next.effects.transport.execution = async () => null;
  const sdkBodies: string[] = [];
  const sdk = new GameClient({ baseURL: binding.origin, matchId: binding.match, capability: 'test-only', fetch: async (_url, options) => {
    sdkBodies.push(options!.body as string); return new Response(JSON.stringify({ status: 'accepted', command_id: record.envelope.command_id }));
  } });
  next.effects.transport.submitExact = (body, signal) => sdk.submitExact(body, { signal });
  await next.lifecycle.reconcile(); original.resolve({ status: 'accepted', command_id: record.envelope.command_id }); await sending;
  assert.equal(sdkBodies[0], r.bodies[0]); assert.equal(next.record()!.exactBody, r.bodies[0]);
});

test('definitive stale rejection waits for refreshed view and never replays old intent', async () => {
  const r = rig(); await r.lifecycle.recover();
  r.effects.transport.submitExact = async body => { r.bodies.push(body); return { command_id: parsePreparedCommand(body).envelope.command_id, status: 'rejected', reason: 'stale_revision' }; };
  await r.lifecycle.submitIntent(intent);
  assert.equal(r.record(), null); assert.equal(r.state().disposition, 'rejected'); assert.equal(r.state().rejection, 'stale_revision'); assert.equal(r.refreshes(), 1);
  await r.lifecycle.submitIntent(intent); assert.equal(r.bodies.length, 1);
  r.context({ ...context, revision: '2' }); r.lifecycle.refreshed();
  await r.lifecycle.reconcile(); await r.lifecycle.submitIntent(intent); assert.equal(r.bodies.length, 1);
  await r.lifecycle.submitIntent({ ...intent, context: { ...context, revision: '2' } }); assert.equal(r.bodies.length, 2);
  assert.notEqual(parsePreparedCommand(r.bodies[0]!).envelope.command_id, parsePreparedCommand(r.bodies[1]!).envelope.command_id);
});

test('queued cancellation accepted/missing/error is not universal Undo; only definitive status resolves', async () => {
  const r = await pending(); const record = r.record();
  await r.lifecycle.cancelQueued(); assert.equal(r.record(), record);
  r.effects.transport.cancelQueued = async () => { throw new Error('not queued or missing'); };
  await r.lifecycle.cancelQueued(); assert.equal(r.record(), record);
  r.effects.transport.execution = async id => ({ command_id: id, status: { status: 'cancelled' } });
  await r.lifecycle.reconcile(true); assert.equal(r.record(), null); assert.equal(r.state().disposition, 'cancelled');
});

test('every definitive server disposition resolves only its own exact durable record', async () => {
  for (const [status, disposition] of [[{ status: 'consumed_by_rules' }, 'consumed'], [{ status: 'discarded_by_departure' }, 'departure-discarded'],
    [{ status: 'cancelled' }, 'cancelled'], [{ status: 'rejected', reason: 'invalid_response' }, 'rejected']] as const) {
    const r = await pending(); r.effects.transport.execution = async id => ({ command_id: id, status });
    await r.lifecycle.reconcile(); assert.equal(r.state().disposition, disposition); assert.equal(r.record(), null);
  }
});

test('scan across changed engine/server/catalog/protocol bindings blocks replay and replacement', async () => {
  const original = (await pending()).record()!;
  for (const key of ['origin', 'match', 'seat', 'engineBuild', 'serverBuild', 'catalog', 'protocol'] as const) {
    const record = { ...original, binding: { ...binding, [key]: 'f'.repeat(64) } };
    const r = rig(record); await r.lifecycle.recover();
    assert.equal(r.state().failure, 'binding');
    await r.lifecycle.reconcile(true); await r.lifecycle.cancelQueued(); await r.lifecycle.submitIntent(intent);
    assert.equal(r.record(), record); assert.deepEqual(r.events, ['scan']);
  }
});

test('unknown outcomes use bounded auto reconciliation; stop and late completions cannot publish/resolve', async () => {
  const r = await pending(); for (let i = 0; i < 10; i++) await r.lifecycle.reconcile();
  assert.equal(r.events.filter(event => event === 'execution').length, 4);
  const late = deferred<CommandExecution>(); r.effects.transport.execution = () => late.promise;
  const operation = r.lifecycle.reconcile(true); const snapshot = r.state(); r.stop();
  late.resolve({ command_id: r.record()!.envelope.command_id, status: { status: 'consumed_by_rules' } }); await operation;
  assert.equal(r.state(), snapshot); assert.ok(r.record());
});

test('durable resolve failure, corrupt recovery and foreign status all preserve uncertainty', async () => {
  const r = await pending();
  r.effects.transport.execution = async id => ({ command_id: id, status: { status: 'consumed_by_rules' } });
  r.effects.journal.resolve = async () => { throw new JournalFailure('failed'); };
  await r.lifecycle.reconcile(); assert.ok(r.record()); assert.equal(r.state().failure, 'failed');
  r.effects.transport.execution = async () => ({ command_id: 'f'.repeat(32), status: { status: 'consumed_by_rules' } });
  await r.lifecycle.reconcile(true); assert.ok(r.record()); assert.equal(r.state().failure, 'transport');
  const corrupt = rig(); corrupt.effects.journal.unresolved = async () => { throw new JournalFailure('corrupt'); };
  await corrupt.lifecycle.recover(); await corrupt.lifecycle.submitIntent(intent); assert.equal(corrupt.bodies.length, 0);
});


test('synchronous subscription cleanup at persisting/sending/rejection cannot initiate effects after stop', async () => {
  for (const phase of ['persisting', 'sending', 'rejection'] as const) {
    const r = rig(); await r.lifecycle.recover();
    if (phase === 'rejection') r.effects.transport.submitExact = async body => ({ status: 'rejected', reason: 'stale_revision', command_id: parsePreparedCommand(body).envelope.command_id });
    r.publish(snapshot => { if (snapshot.phase === phase || (phase === 'rejection' && snapshot.rejection)) r.stop(); });
    await r.lifecycle.submitIntent(intent);
    assert.ok(!r.events.some(event => event.startsWith('resolve:')));
    if (phase === 'persisting') assert.equal(r.record(), null);
    else assert.ok(r.record());
    assert.equal(r.bodies.length, 0);
  }
});
