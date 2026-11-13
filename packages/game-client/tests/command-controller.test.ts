import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareCommand, parsePreparedCommand } from '@multiverse/game-server-sdk/browser';
import type { CommandEnvelope, CommandExecution, PublicManifest, SeatResume } from '@multiverse/game-server-sdk';
import opening from '../fixtures/foundations-opening/opening-seat0-http.json' with { type: 'json' };
import { ReadSession, type SessionEffects } from '../src/session/controller.ts';
import { type CommandEffects, type DecisionIntent } from '../src/session/command.ts';
import type { JournalRecord } from '../src/session/contracts.ts';
import { SessionFailure, type ReadBinding } from '../src/session/read-state.ts';
import { JournalFailure } from '../src/session/journal.ts';
const original = opening.resume as SeatResume;
const binding: ReadBinding = { protocol_version: 1, server_origin: 'http://127.0.0.1:18741', match_id: original.match_id, viewer: 0, engine_build_id: original.engine_build_id, server_build_id: original.server_build_id, catalog_sha256: original.catalog_sha256 };
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { resolve, promise }; }
async function flush() { for (let i = 0; i < 50; i++) await Promise.resolve(); }
function rig() {
  let record: JournalRecord<CommandEnvelope> | null = null, owns = false, resumes = 0, polls = 0, sends = 0, ids = 0;
  const events: string[] = [], waits: ReturnType<typeof deferred<void>>[] = [];
  const commands: CommandEffects = {
    protocol: '4'.repeat(64), protocolVersion: 1, codec: { prepare: prepareCommand, parse: parsePreparedCommand }, newId: () => (++ids).toString(16).padStart(32, '0'),
    journal: {
      async unresolved() { assert.ok(owns); assert.ok(resumes > 0, 'actual authorized viewer is checked before journal access'); events.push('scan'); return record ? [record] : []; },
      async persist(value) { assert.ok(owns); assert.equal(record, null); record = value; events.push('persist'); },
      async resolve(value) { assert.ok(owns); assert.equal(value, record); record = null; events.push('resolve'); },
    },
    transport: {
      async submitExact(body) { assert.ok(owns); sends++; assert.equal(body, record!.exactBody); return { status: 'accepted', command_id: record!.envelope.command_id }; },
      async execution(id) { assert.ok(owns); return { command_id: id, status: { status: 'pending' } }; },
      async cancelQueued(id) { return { status: 'accepted', command_id: id }; },
    },
  };
  const effects: SessionEffects<string> = {
    cancellation() { const c = new AbortController(); return { signal: c.signal, abort: () => c.abort() }; },
    async acquire() { events.push('lock'); owns = true; return () => { owns = false; events.push('release'); }; },
    async authorize() { return { binding, commands, transport: {
      async manifest() { events.push('manifest'); return { ...binding, profile: opening.profile } as unknown as PublicManifest; },
      async presentation() { return null; },
      async resume() { resumes++; events.push('resume'); return structuredClone(original); },
      async poll(after) { polls++; return { protocol_version: 1, entries: [], next: after, has_more: false }; },
    } }; },
    wait() { const wait = deferred<void>(); waits.push(wait); return wait.promise; },
    classify(error) { return error instanceof SessionFailure ? error.kind : 'disconnected'; },
  };
  const session = new ReadSession(effects);
  const intent = (): DecisionIntent => ({ kind: 'decision', source: 'human', context: session.commandContext()!, response: { response: 'mulligan', take: false } });
  return { session, commands, effects, events, waits, intent, record: () => record, stats: () => ({ sends, resumes, polls }) };
}

test('authorization, Web Lock, manifest and actual viewer precede scan; one cursor/read owner', async () => {
  const r = rig(), running = r.session.join('seat'); await flush();
  assert.deepEqual(r.events.slice(0, 4), ['lock', 'manifest', 'resume', 'scan']);
  const context = r.session.commandContext()!; assert.equal(context.revision, original.next); assert.equal(context.token, 'v0-d0');
  await r.session.submitIntent(r.intent()); assert.ok(r.record()); assert.equal(r.session.commandContext(), null);
  r.waits[0]!.resolve(); await flush(); assert.equal(r.stats().polls, 1); assert.equal(r.stats().resumes, 1);
  r.session.stop(); r.waits[1]!.resolve(); await running; assert.ok(r.record(), 'local stop retains unresolved slot');
});

test('read-only API has no journal access and denied lock or wrong current viewer never scans', async () => {
  const r = rig(); r.effects.acquire = async () => null;
  await r.session.join('seat'); assert.equal(r.session.getSnapshot().connection, 'blocked'); assert.ok(!r.events.includes('scan'));
  const authorize = r.effects.authorize;
  r.effects.acquire = async () => () => {};
  r.effects.authorize = async (input, signal) => {
    const value = await authorize(input, signal);
    return { ...value, transport: { ...value.transport, async resume() { return { ...original, viewer: 1 }; } } };
  };
  await r.session.join('wrong-viewer'); assert.equal(r.session.getSnapshot().connection, 'incompatible'); assert.ok(!r.events.includes('scan'));
});

test('strict-mode stop during persist leaves journal but no send; fresh controller incarnation rejects old intent', async () => {
  const r = rig(), first = r.session.join('seat'); await flush(); const oldIntent = r.intent();
  const commit = deferred<void>(), persist = r.commands.journal.persist;
  r.commands.journal.persist = async record => { await persist(record); await commit.promise; };
  const sending = r.session.submitIntent(oldIntent);
  await flush();
  r.session.stop(); // Completion may arrive after lock release.
  commit.resolve();
  await sending; r.waits[0]!.resolve(); await first;
  assert.equal(r.stats().sends, 0); assert.ok(r.record());
  const fresh = rig(), second = fresh.session.join('seat'); await flush();
  assert.notEqual(fresh.session.commandContext()!.incarnation, oldIntent.context.incarnation);
  await fresh.session.submitIntent(oldIntent); assert.equal(fresh.stats().sends, 0);
  fresh.session.stop(); fresh.waits[0]!.resolve(); await second;
});

test('definitive rejection forces resume before new intent; old intent is never auto-replayed', async () => {
  const r = rig(), running = r.session.join('seat'); await flush(); const stale = r.intent();
  r.commands.transport.submitExact = async body => ({ command_id: parsePreparedCommand(body).envelope.command_id, status: 'rejected', reason: 'stale_revision' });
  await r.session.submitIntent(stale); assert.equal(r.session.getSnapshot().command.phase, 'awaiting-refresh');
  assert.equal(r.session.commandContext(), null);
  r.waits[0]!.resolve(); await flush();
  assert.equal(r.stats().resumes, 2); assert.equal(r.stats().polls, 0); assert.equal(r.session.getSnapshot().command.phase, 'ready');
  await r.session.submitIntent(stale); assert.equal(r.record(), null);
  assert.equal(r.events.filter(value => value === 'persist').length, 1);
  r.session.stop(); r.waits[1]!.resolve(); await running;
});

test('seat replacement fences late execution and never clears the previous durable command', async () => {
  const r = rig(), first = r.session.join('seat'); await flush(); await r.session.submitIntent(r.intent());
  const status = deferred<CommandExecution>(); r.commands.transport.execution = () => status.promise;
  r.waits[0]!.resolve(); await flush(); const oldRecord = r.record()!;
  r.effects.authorize = async () => { throw new SessionFailure('invalid-file'); };
  await r.session.join('replacement');
  status.resolve({ command_id: oldRecord.envelope.command_id, status: { status: 'consumed_by_rules' } }); await first;
  assert.equal(r.session.getSnapshot().connection, 'invalid-file'); assert.equal(r.record(), oldRecord);
});

test('journal corruption blocks commands while the existing read view remains available', async () => {
  const r = rig(); r.commands.journal.unresolved = async () => { throw new JournalFailure('corrupt'); };
  const running = r.session.join('seat'); await flush();
  assert.equal(r.session.getSnapshot().connection, 'ready'); assert.equal(r.session.getSnapshot().command.failure, 'corrupt');
  assert.equal(r.session.commandContext(), null); assert.ok(r.session.getSnapshot().read);
  r.session.stop(); r.waits[0]!.resolve(); await running;
});

test('failed refresh resume keeps the post-rejection gate until a later successful resume', async () => {
  const r = rig(), authorize = r.effects.authorize; let attempts = 0;
  r.effects.authorize = async (input, signal) => {
    const value = await authorize(input, signal);
    return { ...value, transport: { ...value.transport, async resume(readSignal) {
      attempts++; if (attempts === 2) throw new SessionFailure('unavailable'); return value.transport.resume(readSignal);
    } } };
  };
  const running = r.session.join('seat'); await flush(); const stale = r.intent();
  r.commands.transport.submitExact = async body => ({ command_id: parsePreparedCommand(body).envelope.command_id, status: 'rejected', reason: 'stale_revision' });
  await r.session.submitIntent(stale); r.waits[0]!.resolve(); await flush();
  assert.equal(r.session.getSnapshot().command.phase, 'awaiting-refresh'); assert.equal(r.session.commandContext(), null);
  r.waits[1]!.resolve(); await flush();
  assert.equal(attempts, 3); assert.equal(r.session.getSnapshot().command.phase, 'ready');
  await r.session.submitIntent(stale); assert.equal(r.events.filter(event => event === 'persist').length, 1);
  r.session.stop(); r.waits[2]!.resolve(); await running;
});

test('lost command ack recovers from authoritative status through the existing read loop', async () => {
  const r = rig(), running = r.session.join('seat'); await flush();
  r.commands.transport.submitExact = async () => { throw new Error('response lost'); };
  await r.session.submitIntent(r.intent()); assert.equal(r.session.getSnapshot().command.phase, 'unresolved');
  r.commands.transport.execution = async id => ({ command_id: id, status: { status: 'consumed_by_rules' } });
  r.waits[0]!.resolve(); await flush(); assert.equal(r.record(), null); assert.equal(r.stats().polls, 1);
  r.waits[1]!.resolve(); await flush(); assert.equal(r.stats().resumes, 2); assert.equal(r.session.getSnapshot().command.phase, 'ready');
  r.session.stop(); r.waits[2]!.resolve(); await running;
});
