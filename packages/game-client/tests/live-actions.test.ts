import assert from 'node:assert/strict';
import test from 'node:test';
import type { ActionQueryRequest, ActionQueryResponse, CommandEnvelope, PublicManifest, SeatResume, StreamPage } from '@multiverse/game-server-sdk';
import { parsePreparedCommand, prepareCommand } from '@multiverse/game-server-sdk/browser';
import opening from '../fixtures/foundations-opening/opening-seat0-http.json' with { type: 'json' };
import { ReadSession, type SessionEffects } from '../src/session/controller.ts';
import type { CommandEffects } from '../src/session/command.ts';
import type { JournalRecord } from '../src/session/contracts.ts';
import { SessionFailure, type ReadBinding } from '../src/session/read-state.ts';
import { actionResponse } from '../src/session/live-action.ts';

// Synthetic priority over a permitted fixture tests orchestration, never native legality.
const original = structuredClone(opening.resume) as SeatResume;
if (original.state.status !== 'available') throw new Error('fixture');
original.state.observation.decision = { request: 'priority', player: 0, token: 'v0-d0' };
const binding: ReadBinding = { protocol_version: 1, server_origin: 'http://127.0.0.1:18741', match_id: original.match_id, viewer: 0, engine_build_id: original.engine_build_id, server_build_id: original.server_build_id, catalog_sha256: original.catalog_sha256 };
const pass = { initiation: { action: 'pass' as const }, label: 'unavailable' as const };
const cursor = 'f'.repeat(64);
const complete = (): ActionQueryResponse => ({ protocol_version: 1, match_id: binding.match_id, viewer: 0, outcome: { status: 'complete', page: { token: 'v0-d0', scope: { scope: 'window' }, context: { window: 'priority' }, beginnings: [pass] } } });
const incomplete = (): ActionQueryResponse => { const result = complete(); if (result.outcome.status !== 'complete') throw new Error(); return { ...result, outcome: { ...result.outcome, status: 'incomplete', cursor, required_minimum: 1 } }; };
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { resolve, promise }; }
async function flush() { for (let i = 0; i < 100; i++) await Promise.resolve(); }
function rig() {
  let owns = false, record: JournalRecord<CommandEnvelope> | null = null, sends = 0, authorizations = 0;
  const events: string[] = [], waits: ReturnType<typeof deferred<void>>[] = [], operations: ReturnType<typeof deferred<void>>[] = [];
  const queries: ActionQueryRequest[] = [];
  let handler = async (_request: ActionQueryRequest): Promise<ActionQueryResponse> => complete();
  let poll = async (after: string): Promise<StreamPage> => ({ protocol_version: 1, next: after, entries: [], has_more: false });
  const commands: CommandEffects = {
    protocol: '4'.repeat(64), protocolVersion: 1, codec: { prepare: prepareCommand, parse: parsePreparedCommand }, newId: () => 'a'.repeat(32),
    journal: {
      async unresolved() { assert.ok(owns); events.push('scan'); return record ? [record] : []; },
      async persist(value) { assert.ok(owns); assert.equal(record, null); events.push('persist'); record = value; },
      async resolve() { assert.ok(owns); events.push('resolve'); record = null; },
    },
    transport: {
      async submitExact(body) { assert.ok(owns); assert.equal(record?.exactBody, body); events.push('send'); sends++; return { status: 'accepted', command_id: record!.envelope.command_id }; },
      async execution(id) { return { command_id: id, status: { status: 'pending' } }; },
      async cancelQueued(id) { return { status: 'accepted', command_id: id }; },
    },
  };
  const effects: SessionEffects<string> = {
    cancellation() { const c = new AbortController(); return { signal: c.signal, abort: () => c.abort() }; },
    async acquire() { assert.equal(owns, false); events.push('lock'); owns = true; return () => { events.push('release'); owns = false; }; },
    async authorize() { authorizations++; return { binding, commands, actions: {
      protocolSchema: commands.protocol, now: () => 0, classify: () => 'request-transport',
      operation() { const c = new AbortController(), expired = deferred<void>(); operations.push(expired); return { signal: c.signal, expired: expired.promise, abort: () => c.abort(), dispose() {} }; },
      async query(request) { assert.ok(owns); events.push(request.operation); queries.push(request); return handler(request); },
    }, transport: {
      async manifest() { events.push('manifest'); return { ...binding, profile: opening.profile } as unknown as PublicManifest; },
      async presentation() { return null; }, async resume() { events.push('resume'); return structuredClone(original); },
      poll(after) { return poll(after); },
    } }; },
    wait() { const next = deferred<void>(); waits.push(next); return next.promise; },
    classify: () => 'disconnected',
  };
  const session = new ReadSession(effects);
  return { session, effects, commands, events, queries, waits, operations,
    handler(value: typeof handler) { handler = value; }, poll(value: typeof poll) { poll = value; },
    stats: () => ({ owns, sends, authorizations, record }),
    async stop(running: Promise<void>) { session.stop(); await flush(); for (const wait of waits) wait.resolve(); await running; await flush(); },
  };
}

test('live discovery shares verified owner; original complete intent journals exact bytes before one send', async () => {
  const r = rig(), running = r.session.join('seat'); await flush();
  assert.deepEqual(r.events, ['lock', 'manifest', 'resume', 'scan']);
  const originalContext = r.session.commandContext();
  const draft = await r.session.startActions(); assert.ok(draft);
  assert.deepEqual(draft.command, originalContext); assert.equal(Object.isFrozen(draft), true);
  assert.equal(r.session.getSnapshot().actions?.phase, 'complete');
  await r.session.submitAction({ ...draft }, 0); assert.equal(r.stats().sends, 0, 'unissued draft identity');
  await r.session.submitAction(draft, 2); assert.equal(r.stats().sends, 0);
  await r.session.submitAction(draft, 0);
  assert.deepEqual(r.events.slice(-2), ['persist', 'send']); assert.equal(r.stats().sends, 1);
  assert.deepEqual(r.stats().record?.envelope.command, { kind: 'decision', token: draft.command.token, response: { response: 'pass' } });
  assert.equal(r.stats().record?.envelope.expected_revision, draft.command.revision);
  await r.session.submitAction(draft, 0); assert.equal(r.stats().sends, 1);
  await r.stop(running); assert.ok(r.stats().record, 'stop keeps unresolved original command');
});

test('same-token read revision replacement synchronously revokes pages and original draft', async () => {
  const r = rig(), running = r.session.join('seat'); await flush();
  const draft = await r.session.startActions(); assert.ok(draft);
  r.poll(async () => ({ protocol_version: 1, next: '2', entries: [{ cursor: '2', publication: { kind: 'lifecycle', status: { status: 'resource_paused' } } }], has_more: false }));
  r.waits[0]!.resolve(); await flush();
  assert.equal(r.session.getSnapshot().actions?.phase, 'stale');
  assert.deepEqual(r.session.getSnapshot().actions?.beginnings, []);
  await r.session.submitAction(draft, 0); assert.equal(r.stats().sends, 0);
  const next = await r.session.startActions(); assert.ok(next); assert.notEqual(next.query.generation, draft.query.generation);
  assert.equal(next.command.revision, '2');
  await r.stop(running);
});

test('seat replacement waits for captured SDK cancellation while lock is held; stopped queued import never restarts', async () => {
  const r = rig(), cancellation = deferred<ActionQueryResponse>();
  r.handler(async request => request.operation === 'cancel' ? cancellation.promise : incomplete());
  const running = r.session.join('seat'); await flush(); const discovery = r.session.startActions(); await flush();
  assert.equal(r.session.getSnapshot().actions?.phase, 'incomplete');
  const replacement = r.session.join('other'); await flush();
  assert.deepEqual(r.queries.map(query => query.operation), ['start', 'cancel']);
  assert.equal(r.stats().owns, true); assert.equal(r.stats().authorizations, 1);
  r.session.stop(); cancellation.resolve({ protocol_version: 1, match_id: binding.match_id, viewer: 0, outcome: { status: 'cancelled' } });
  for (const wait of r.waits) wait.resolve(); await flush(); await discovery; await replacement; await running;
  assert.equal(r.stats().owns, false); assert.equal(r.stats().authorizations, 1);
  assert.equal(r.session.getSnapshot().connection, 'idle');
});

test('late in-flight own cursor cleanup finishes before release and uses old authorization', async () => {
  const r = rig(), result = deferred<ActionQueryResponse>(), cancellation = deferred<ActionQueryResponse>();
  r.handler(async request => request.operation === 'cancel' ? cancellation.promise : result.promise);
  const running = r.session.join('seat'); await flush(); const discovery = r.session.startActions(); await flush();
  r.session.stop(); await flush(); assert.equal(r.stats().owns, true);
  result.resolve(incomplete()); await flush();
  assert.deepEqual(r.queries.map(query => query.operation), ['start', 'cancel']); assert.equal(r.stats().owns, true);
  cancellation.resolve({ protocol_version: 1, match_id: binding.match_id, viewer: 0, outcome: { status: 'cancelled' } });
  await discovery; await r.stop(running); assert.equal(r.stats().owns, false);
});

test('unexplained beginnings and mana payment context cannot construct live responses', () => {
  assert.deepEqual(actionResponse(pass, { window: 'priority' }, { scope: 'window' }), { response: 'pass' });
  assert.equal(actionResponse(pass, { window: 'priority' }, { scope: 'object', object: 'a' }), null);
  assert.equal(actionResponse(pass, null, { scope: 'window' }), null);
  assert.equal(actionResponse(pass, { window: 'mana', cost: { generic: '1', colored: [] }, source: { status: 'unavailable' } }, { scope: 'window' }), null);
  for (const initiation of [{ action: 'play_land', object: 'a' }, { action: 'cast', card: 'a' }, { action: 'activate_mana', object: 'a' }, { action: 'activate_ability', object: 'a', ability: 0 }, { action: 'finish_mana', undo_mana: false }] as const) {
    assert.equal(actionResponse({ initiation, label: 'unavailable' }, { window: 'priority' }, { scope: 'window' }), null);
  }
});

test('failed durable persist blocks discovered pass submission', async () => {
  const r = rig(), running = r.session.join('seat'); await flush();
  const draft = await r.session.startActions(); assert.ok(draft);
  r.commands.journal.persist = async () => { throw new SessionFailure('unavailable'); };
  await r.session.submitAction(draft, 0); assert.equal(r.stats().sends, 0);
  assert.equal(r.session.getSnapshot().command.phase, 'blocked'); await r.stop(running);
});

test('bounded in-flight cleanup releases ownership and never sends a late cancel after release', async () => {
  const r = rig(), result = deferred<ActionQueryResponse>();
  r.handler(async () => result.promise);
  const running = r.session.join('seat'); await flush();
  const discovery = r.session.startActions(); await flush();
  r.session.stop(); await flush(); assert.equal(r.stats().owns, true);
  // Controller and captured transport wrapper each own an independent deadline.
  // Expire the wrapper to simulate a transport that ignores abort indefinitely.
  r.operations[1]!.resolve(); await flush();
  assert.equal(r.stats().owns, false); await discovery;
  result.resolve(incomplete()); await flush();
  assert.deepEqual(r.queries.map(query => query.operation), ['start']);
  await r.stop(running);
});

test('no action I/O before ownership/actual viewer validation or with mismatched schema qualification', async () => {
  for (const mode of ['lock', 'viewer', 'schema'] as const) {
    const r = rig(), authorize = r.effects.authorize;
    if (mode === 'lock') r.effects.acquire = async () => null;
    r.effects.authorize = async (input, signal) => {
      const value = await authorize(input, signal);
      if (mode === 'schema') return { ...value, actions: { ...value.actions!, protocolSchema: '9'.repeat(64) } };
      if (mode === 'viewer') return { ...value, transport: { ...value.transport, async resume() { return { ...original, viewer: 1 }; } } };
      return value;
    };
    const running = r.session.join('seat'); await flush();
    assert.equal(await r.session.startActions(), null); assert.equal(r.queries.length, 0);
    await r.stop(running);
  }
});
