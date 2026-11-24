import assert from 'node:assert/strict';
import test from 'node:test';
import type { PublicManifest, SeatResume, StreamPage } from '@multiverse/game-server-sdk';
import type { PresentationArtifact } from '../generated/presentation.d.ts';
import opening from './current-opening.ts';
import metadata from '../fixtures/foundations-opening/presentation.json' with { type: 'json' };
import { ReadSession, type Authorization, type SessionEffects, type ReadTransport } from '../web/src/session/controller.ts';
import { checkManifest, resumeRead, streamRead, SessionFailure, type ReadBinding } from '../web/src/session/read-state.ts';

const original = opening.resume as SeatResume;
const binding: ReadBinding = { protocol_version: 2, server_origin: 'http://127.0.0.1:18741', match_id: original.match_id, viewer: 0, engine_build_id: original.engine_build_id, server_build_id: original.server_build_id, catalog_sha256: original.catalog_sha256 };
// Only identity is consumed from the manifest by the session. SDK schema validation
// owns the actual wire catalog. This is explicitly an injected controller double.
const manifest = { ...binding, profile: opening.profile } as unknown as PublicManifest;
function deferred<T>() { let resolve!: (value: T) => void; let reject!: (error: unknown) => void; const promise = new Promise<T>((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; }
async function flush() { for (let i = 0; i < 30; i++) await Promise.resolve(); }
function rig(overrides: Partial<ReadTransport> = {}) {
  let acquired = false, releases = 0, aborts = 0, polls = 0;
  const sleeps: { milliseconds: number; done: ReturnType<typeof deferred<void>>; signal: AbortSignal }[] = [];
  const pollResults: ReturnType<typeof deferred<StreamPage>>[] = [];
  const calls: string[] = [];
  const transport: ReadTransport = {
    async manifest() { assert.ok(acquired); calls.push('manifest'); return structuredClone(manifest); },
    async presentation() { assert.ok(acquired); calls.push('presentation'); return structuredClone(metadata) as PresentationArtifact; },
    async resume() { assert.ok(acquired); calls.push('resume'); return structuredClone(original); },
    async poll(after) { assert.ok(acquired); assert.equal(after, original.next); polls++; const result = deferred<StreamPage>(); pollResults.push(result); return result.promise; },
    ...overrides,
  };
  const effects: SessionEffects<string> = {
    async authorize() { return { binding, transport }; },
    cancellation() { const controller = new AbortController(); return { signal: controller.signal, abort() { if (!controller.signal.aborted) aborts++; controller.abort(); } }; },
    async acquire() { acquired = true; return () => { acquired = false; releases++; }; },
    wait(milliseconds, signal) { const done = deferred<void>(); sleeps.push({ milliseconds, done, signal }); return done.promise; },
    classify(error) { return error instanceof SessionFailure ? error.kind : 'disconnected'; },
  };
  const controller = new ReadSession(effects);
  return { controller, effects, calls, sleeps, pollResults, transport, stats: () => ({ releases, aborts, polls }) };
}

test('one lock precedes all reads, stable immutable snapshots, one bounded polling owner', async () => {
  const r = rig();
  const running = r.controller.join('seat0'); await flush();
  assert.deepEqual(r.calls, ['manifest', 'presentation', 'resume']);
  assert.equal(r.controller.getSnapshot().connection, 'ready');
  assert.equal(r.controller.getSnapshot(), r.controller.getSnapshot());
  assert.ok(Object.isFrozen(r.controller.getSnapshot().read?.state));
  assert.equal(r.stats().polls, 0);
  r.sleeps[0]!.done.resolve(); await flush();
  assert.equal(r.stats().polls, 1);
  r.pollResults[0]!.resolve({ protocol_version: 2, entries: [], next: '1', has_more: false }); await flush();
  assert.equal(r.stats().polls, 1, 'even an immediate empty response must wait');
  r.controller.stop(); r.controller.stop();
  r.sleeps[1]!.done.resolve(); await running;
  assert.equal(r.stats().releases, 1);
  assert.equal(r.controller.getSnapshot().read, null);
});

test('duplicate-tab denial and unavailable locks never read the host', async () => {
  const r = rig(); r.effects.acquire = async () => null;
  await r.controller.join('seat0');
  assert.equal(r.controller.getSnapshot().connection, 'blocked'); assert.deepEqual(r.calls, []);
  r.effects.acquire = async () => { throw new SessionFailure('locks-unavailable'); };
  await r.controller.join('seat0');
  assert.equal(r.controller.getSnapshot().connection, 'locks-unavailable'); assert.deepEqual(r.calls, []);
});

test('late lock grant after stop is released without reading or disturbing replacement', async () => {
  const r = rig(); const lock = deferred<(() => void) | null>(); let released = 0;
  r.effects.acquire = () => lock.promise;
  const old = r.controller.join('old'); await flush(); r.controller.stop();
  lock.resolve(() => { released++; }); await old;
  assert.equal(released, 1); assert.deepEqual(r.calls, []); assert.equal(r.controller.getSnapshot().connection, 'idle');
});

test('seat switch fences late authorization, resume, polling and old cleanup', async () => {
  for (const stage of ['authorize', 'resume', 'poll'] as const) {
    const r = rig(); const late = deferred<Authorization | SeatResume | StreamPage>();
    if (stage === 'authorize') r.effects.authorize = () => late.promise as Promise<Authorization>;
    if (stage === 'resume') r.transport.resume = () => late.promise as Promise<SeatResume>;
    if (stage === 'poll') r.transport.poll = () => late.promise as Promise<StreamPage>;
    const old = r.controller.join('old'); await flush();
    if (stage === 'poll') { r.sleeps[0]!.done.resolve(); await flush(); }
    r.effects.authorize = async () => { throw new SessionFailure('invalid-file'); };
    await r.controller.join('new-seat');
    late.resolve(stage === 'authorize' ? { binding, transport: r.transport } : stage === 'resume' ? original : { protocol_version: 2, entries: [], next: '1', has_more: false });
    await old;
    assert.equal(r.controller.getSnapshot().connection, 'invalid-file', stage);
    assert.equal(r.controller.getSnapshot().read, null, stage);
  }
});

test('StrictMode-like cleanup and restart do not leave two active loops', async () => {
  const r = rig(); const first = r.controller.join('seat0'); await flush();
  r.controller.stop(); const second = r.controller.join('seat0'); await flush();
  r.sleeps[0]!.done.resolve(); await first;
  assert.equal(r.controller.getSnapshot().connection, 'ready');
  assert.equal(r.stats().releases, 1); assert.equal(r.stats().polls, 0);
  r.sleeps[1]!.done.resolve(); await flush();
  r.controller.stop(); r.pollResults[0]!.resolve({ protocol_version: 2, entries: [], next: '1', has_more: false }); await second;
  assert.equal(r.stats().releases, 2); assert.equal(r.stats().polls, 1);
});

test('every identity field, profile, viewer and observation version fail closed', () => {
  for (const key of ['match_id', 'engine_build_id', 'server_build_id', 'catalog_sha256'] as const) {
    assert.throws(() => checkManifest(binding, { ...manifest, [key]: 'different' }), SessionFailure);
    assert.throws(() => resumeRead(binding, null, { ...original, [key]: 'different' }), SessionFailure);
  }
  assert.throws(() => checkManifest(binding, { ...manifest, protocol_version: 1 }), SessionFailure);
  assert.throws(() => checkManifest(binding, { ...manifest, profile: 'unreviewed' }), SessionFailure);
  assert.throws(() => resumeRead(binding, null, { ...original, viewer: 1 }), SessionFailure);
  for (const patch of [{ observation_version: 99 }, { view: { ...(original.state.status === 'available' ? original.state.observation.view : {}), viewer: 1 } }]) {
    const value = structuredClone(original); if (value.state.status !== 'available') assert.fail();
    Object.assign(value.state.observation, patch);
    assert.throws(() => resumeRead(binding, null, value), SessionFailure);
  }
});

test('pending and lifecycle-only resume preserve observations; entire stream page installs atomically', () => {
  const current = resumeRead(binding, null, structuredClone(original));
  const paused = resumeRead(binding, current, { ...current, next: '2', state: { status: 'pending' }, lifecycle: { status: 'resource_paused' } });
  assert.equal(paused.state, current.state); assert.equal(paused.next, '2');
  const ended = streamRead(binding, paused, { protocol_version: 2, entries: [{ cursor: '3', publication: { kind: 'lifecycle', status: { status: 'faulted' } } }, { cursor: '4', publication: { kind: 'outcome', result: { outcome: 'win', player: 2 } } }], next: '4', has_more: false });
  assert.equal(ended.state, current.state); assert.equal(ended.outcome?.outcome, 'win');
  assert.equal(paused.next, '2'); assert.equal(paused.outcome, null);
  assert.throws(() => streamRead(binding, paused, { protocol_version: 2, entries: [{ cursor: '3', publication: { kind: 'lifecycle', status: { status: 'faulted' } } }], next: '4', has_more: false }), SessionFailure);
  assert.throws(() => resumeRead(binding, paused, original), SessionFailure);
});

test('pending, missing metadata, host pause/fault and rules outcome have distinct states', async () => {
  const r = rig({ async presentation() { return null; }, async resume() { return { ...original, state: { status: 'pending' }, lifecycle: { status: 'faulted' } }; } });
  const running = r.controller.join('seat0'); await flush();
  assert.equal(r.controller.getSnapshot().connection, 'pending');
  assert.equal(r.controller.getSnapshot().metadataMissing, true);
  assert.equal(r.controller.getSnapshot().read?.outcome, null);
  r.sleeps[0]!.done.resolve(); await flush();
  r.pollResults[0]!.resolve({ protocol_version: 2, entries: [{ cursor: '2', publication: { kind: 'outcome', result: { outcome: 'draw' } } }], next: '2', has_more: false }); await running;
  assert.equal(r.controller.getSnapshot().connection, 'ended'); assert.equal(r.stats().releases, 1);
});

test('auth and protocol failures clear retained views and never retry', async () => {
  for (const kind of ['unauthorized', 'incompatible'] as const) {
    const r = rig(); const running = r.controller.join('seat0'); await flush();
    r.sleeps[0]!.done.resolve(); await flush(); r.pollResults[0]!.reject(new SessionFailure(kind)); await running;
    assert.equal(r.controller.getSnapshot().connection, kind); assert.equal(r.controller.getSnapshot().read, null); assert.equal(r.sleeps.length, 1);
  }
});

test('429, 503 and disconnected failures use four bounded retries then release ownership', async () => {
  for (const kind of ['rate-limited', 'unavailable', 'disconnected'] as const) {
    let attempts = 0;
    const r = rig({ async resume() { attempts++; throw new SessionFailure(kind); } });
    const running = r.controller.join('seat0'); await flush();
    for (let i = 0; i < 4; i++) {
      assert.equal(r.controller.getSnapshot().connection, 'retrying'); assert.equal(r.controller.getSnapshot().failure, kind);
      assert.equal(r.sleeps[i]!.milliseconds, 500 * 2 ** i); r.sleeps[i]!.done.resolve(); await flush();
    }
    await running;
    assert.equal(attempts, 5); assert.equal(r.controller.getSnapshot().connection, kind); assert.equal(r.stats().releases, 1);
  }
});

test('a server cooldown waits without I/O, then revalidates the full committed boundary', async () => {
  const r = rig({ retryAfter: () => 35_000 }), running = r.controller.join('seat0'); await flush();
  r.sleeps[0]!.done.resolve(); await flush();
  r.pollResults[0]!.reject(new SessionFailure('rate-limited')); await flush();
  assert.equal(r.controller.getSnapshot().connection, 'retrying');
  assert.equal(r.sleeps[1]!.milliseconds, 35_000);
  assert.deepEqual(r.calls, ['manifest', 'presentation', 'resume']);
  await flush(); assert.equal(r.stats().polls, 1, 'no request before the cooldown completes');
  r.sleeps[1]!.done.resolve(); await flush();
  assert.deepEqual(r.calls, ['manifest', 'presentation', 'resume', 'manifest', 'presentation', 'resume']);
  r.controller.stop(); r.sleeps[2]!.done.resolve(); await running;
});

test('hinted 429s preserve newly verified recovery stages without renewing the retry budget', async () => {
  const cooldown = 35_000;
  const r = rig({ retryAfter: () => cooldown });
  const running = r.controller.join('seat0'); await flush();
  const initial = r.controller.getSnapshot();
  const emptyPage: StreamPage = { protocol_version: 2, entries: [], next: original.next, has_more: false };
  let sleep = 0;
  const wake = async (milliseconds: number) => {
    const wait = r.sleeps[sleep++]!;
    assert.equal(wait.milliseconds, milliseconds);
    wait.done.resolve(); await flush();
  };
  const initialCalls = ['manifest', 'presentation', 'resume'];
  assert.deepEqual(r.calls, initialCalls);
  await wake(100); r.pollResults[0]!.resolve(emptyPage); await flush();
  await wake(100); r.pollResults[1]!.reject(new SessionFailure('rate-limited')); await flush();
  assert.equal(r.controller.getSnapshot().retry, 1);
  assert.deepEqual(r.calls, initialCalls, 'no recovery read before cooldown');

  const readPresentation = r.transport.presentation;
  const readResume = r.transport.resume;
  r.transport.presentation = async () => {
    r.transport.presentation = readPresentation;
    r.calls.push('presentation');
    throw new SessionFailure('rate-limited');
  };
  r.transport.resume = async () => {
    r.transport.resume = readResume;
    r.calls.push('resume');
    throw new SessionFailure('rate-limited');
  };
  await wake(cooldown);
  assert.equal(r.controller.getSnapshot().retry, 2);
  assert.deepEqual(r.calls.slice(3), ['manifest', 'presentation']);
  await wake(cooldown);
  assert.equal(r.controller.getSnapshot().retry, 3);
  assert.deepEqual(r.calls.slice(3), ['manifest', 'presentation', 'presentation', 'resume']);
  assert.equal(r.controller.getSnapshot().metadata, initial.metadata, 'staged presentation is not published before resume');
  await wake(cooldown);
  assert.equal(r.controller.getSnapshot().connection, 'ready');
  assert.notEqual(r.controller.getSnapshot().metadata, initial.metadata);
  assert.deepEqual(r.calls.slice(3), ['manifest', 'presentation', 'presentation', 'resume', 'resume']);

  await wake(100); r.pollResults[2]!.reject(new SessionFailure('rate-limited')); await flush();
  assert.equal(r.controller.getSnapshot().retry, 4, 'successful resume alone cannot renew the retry budget');
  await wake(cooldown);
  assert.deepEqual(r.calls.slice(8), ['resume'], 'recovery remains open until a successful stream read');
  await wake(100); r.pollResults[3]!.resolve(emptyPage); await flush();
  await wake(100); r.pollResults[4]!.reject(new SessionFailure('rate-limited')); await flush();
  assert.equal(r.controller.getSnapshot().retry, 1, 'successful stream read renews the retry budget');
  await wake(cooldown);
  assert.deepEqual(r.calls.slice(9), initialCalls, 'a new recovery episode requires fresh revalidation');
  r.controller.stop(); await wake(100); await running;
});

test('hintless 429 and 503 discard partial revalidation from a hinted cooldown', async () => {
  for (const kind of ['rate-limited', 'unavailable'] as const) {
    let hint = 35_000;
    const r = rig({ retryAfter: () => hint });
    const running = r.controller.join('seat0'); await flush();
    const readPresentation = r.transport.presentation;
    r.transport.presentation = async () => {
      r.transport.presentation = readPresentation;
      r.calls.push('presentation');
      if (kind === 'rate-limited') hint = 0;
      throw new SessionFailure(kind);
    };
    r.sleeps[0]!.done.resolve(); await flush();
    r.pollResults[0]!.reject(new SessionFailure('rate-limited')); await flush();
    assert.equal(r.sleeps[1]!.milliseconds, 35_000);
    r.sleeps[1]!.done.resolve(); await flush();
    assert.equal(r.sleeps[2]!.milliseconds, 1_000);
    assert.deepEqual(r.calls.slice(3), ['manifest', 'presentation'], kind);
    r.sleeps[2]!.done.resolve(); await flush();
    assert.deepEqual(r.calls.slice(5), ['manifest', 'presentation', 'resume'], kind);
    assert.equal(r.controller.getSnapshot().connection, 'ready', kind);
    r.controller.stop(); r.sleeps[3]!.done.resolve(); await running;
  }
});

test('only successful polling after an explicit cooldown renews the rate-limit recovery budget', async () => {
  for (const hinted of [true, false]) {
    const r = rig({ ...(hinted ? { retryAfter: () => 35_000 } : {}) });
    const running = r.controller.join('seat0'); await flush();
    let sleep = 0, poll = 0;
    for (let cycle = 0; cycle < 5; cycle++) {
      r.sleeps[sleep++]!.done.resolve(); await flush();
      r.pollResults[poll++]!.reject(new SessionFailure('rate-limited')); await flush();
      if (!hinted && cycle === 4) break;
      assert.equal(r.controller.getSnapshot().connection, 'retrying');
      assert.equal(r.sleeps[sleep]!.milliseconds, hinted ? 35_000 : 500 * 2 ** cycle);
      r.sleeps[sleep++]!.done.resolve(); await flush();
      r.sleeps[sleep++]!.done.resolve(); await flush();
      r.pollResults[poll++]!.resolve({ protocol_version: 2, entries: [], next: original.next, has_more: false }); await flush();
      assert.equal(r.controller.getSnapshot().connection, 'ready');
    }
    if (hinted) { r.controller.stop(); r.sleeps[sleep]!.done.resolve(); }
    await running;
    assert.equal(r.stats().releases, 1);
    if (!hinted) assert.equal(r.controller.getSnapshot().connection, 'rate-limited');
  }
});

test('stop aborts a cooldown and prevents late recovery reads', async () => {
  const r = rig({ retryAfter: () => 35_000 }), running = r.controller.join('seat0'); await flush();
  r.sleeps[0]!.done.resolve(); await flush();
  r.pollResults[0]!.reject(new SessionFailure('rate-limited')); await flush();
  r.controller.stop();
  assert.equal(r.sleeps[1]!.signal.aborted, true);
  r.sleeps[1]!.done.resolve(); await running;
  assert.deepEqual(r.calls, ['manifest', 'presentation', 'resume']);
  assert.equal(r.stats().releases, 1);
});

test('reconnect replaces one committed boundary, retaining observation on pending resume', async () => {
  const r = rig(); const running = r.controller.join('seat0'); await flush();
  const previous = r.controller.getSnapshot().read!.state;
  r.sleeps[0]!.done.resolve(); await flush(); r.pollResults[0]!.reject(new SessionFailure('unavailable')); await flush();
  r.transport.resume = async () => ({ ...original, next: '9', state: { status: 'pending' }, lifecycle: { status: 'resource_paused' } });
  r.sleeps[1]!.done.resolve(); await flush();
  assert.equal(r.controller.getSnapshot().read?.next, '9'); assert.equal(r.controller.getSnapshot().read?.state, previous);
  assert.equal(r.controller.getSnapshot().read?.lifecycle?.status, 'resource_paused');
  assert.deepEqual(r.calls, ['manifest', 'presentation', 'resume', 'manifest', 'presentation']);
  r.controller.stop(); r.sleeps[2]!.done.resolve(); await running;
});

test('reconnect revalidates manifest, presentation and resume identities instead of falling back to verified assets', async () => {
  for (const stage of ['manifest', 'presentation', 'resume'] as const) {
    const r = rig(), running = r.controller.join('seat0'); await flush();
    if (stage === 'manifest') {
      const read = r.transport.manifest;
      r.transport.manifest = async signal => ({ ...await read(signal), catalog_sha256: 'changed-catalog' });
    } else if (stage === 'presentation') {
      const read = r.transport.presentation;
      r.transport.presentation = async signal => {
        const value = await read(signal); assert.ok(value);
        return { ...value, catalog_sha256: 'changed-catalog' };
      };
    } else {
      const read = r.transport.resume;
      r.transport.resume = async signal => ({ ...await read(signal), catalog_sha256: 'changed-catalog' });
    }
    r.sleeps[0]!.done.resolve(); await flush();
    r.pollResults[0]!.reject(new SessionFailure('disconnected')); await flush();
    assert.equal(r.controller.getSnapshot().connection, 'retrying');
    r.sleeps[1]!.done.resolve(); await running;
    assert.equal(r.controller.getSnapshot().connection, 'incompatible', stage);
    assert.equal(r.controller.getSnapshot().read, null); assert.equal(r.controller.getSnapshot().metadata, null);
    assert.deepEqual(r.calls.slice(3), stage === 'manifest' ? ['manifest'] : stage === 'presentation' ? ['manifest', 'presentation'] : ['manifest', 'presentation', 'resume']);
    assert.equal(r.stats().releases, 1);
  }
});

test('reconnect authorization failure at any read clears prior data without cached success', async () => {
  for (const stage of ['manifest', 'presentation', 'resume'] as const) {
    const r = rig(), running = r.controller.join('seat0'); await flush();
    r.transport[stage] = async () => { throw new SessionFailure('unauthorized'); };
    r.sleeps[0]!.done.resolve(); await flush();
    r.pollResults[0]!.reject(new SessionFailure('unavailable')); await flush();
    r.sleeps[1]!.done.resolve(); await running;
    assert.equal(r.controller.getSnapshot().connection, 'unauthorized', stage);
    assert.equal(r.controller.getSnapshot().read, null); assert.equal(r.controller.getSnapshot().metadata, null);
    assert.equal(r.sleeps.length, 2); assert.equal(r.stats().releases, 1);
  }
});

test('same-seat reimport refetches immutable reads and a failed replacement cannot expose previous assets', async () => {
  const r = rig(), first = r.controller.join('seat0'); await flush();
  assert.ok(r.controller.getSnapshot().metadata);
  const presentation = r.transport.presentation;
  r.transport.presentation = async signal => { await presentation(signal); return null; };
  const second = r.controller.join('seat0'); await flush();
  assert.deepEqual(r.calls, ['manifest', 'presentation', 'resume', 'manifest', 'presentation', 'resume']);
  assert.equal(r.controller.getSnapshot().metadata, null); assert.equal(r.controller.getSnapshot().metadataMissing, true);
  r.sleeps[0]!.done.resolve(); await first;
  r.transport.manifest = async () => { throw new SessionFailure('unauthorized'); };
  await r.controller.join('replacement');
  assert.equal(r.controller.getSnapshot().connection, 'unauthorized');
  assert.equal(r.controller.getSnapshot().read, null); assert.equal(r.controller.getSnapshot().metadata, null);
  r.sleeps[1]!.done.resolve(); await second;
  assert.equal(r.stats().releases, 3);
});

test('successful recovery resumes cannot hide a perpetually failing poll', async () => {
  const r = rig({ async poll() { throw new SessionFailure('unavailable'); } });
  const running = r.controller.join('seat0'); await flush();
  let cursor = 0;
  for (let failure = 1; failure <= 5; failure++) {
    assert.equal(r.sleeps[cursor]!.milliseconds, 100); r.sleeps[cursor++]!.done.resolve(); await flush();
    if (failure < 5) {
      assert.equal(r.sleeps[cursor]!.milliseconds, 500 * 2 ** (failure - 1));
      r.sleeps[cursor++]!.done.resolve(); await flush();
    }
  }
  await running;
  assert.equal(r.controller.getSnapshot().connection, 'unavailable'); assert.equal(r.stats().releases, 1);
});

test('metadata catalog mismatch rejects before installing a view', async () => {
  const r = rig({ async presentation() { return { ...metadata, catalog_sha256: 'mismatched' } as PresentationArtifact; } });
  await r.controller.join('seat0');
  assert.equal(r.controller.getSnapshot().connection, 'incompatible'); assert.equal(r.controller.getSnapshot().read, null);
  assert.ok(!r.calls.includes('resume'));
});

test('a foreign-view replacement rejects the whole page after earlier valid lifecycle entries', () => {
  const view = structuredClone(original);
  if (view.state.status !== 'available') assert.fail();
  view.state.observation.view.viewer = 1;
  const observation = view.state.observation;
  const current = resumeRead(binding, null, structuredClone(original));
  assert.throws(() => streamRead(binding, current, {
    protocol_version: 2, entries: [
      { cursor: '2', publication: { kind: 'lifecycle', status: { status: 'resource_paused' } } },
      { cursor: '3', publication: { kind: 'view_replaced', observation } },
    ], next: '3', has_more: false,
  }), SessionFailure);
  assert.equal(current.next, '1'); assert.equal(current.lifecycle, null);
});

test('late poll rejection from a released generation cannot fault its replacement', async () => {
  const r = rig(); const old = r.controller.join('old'); await flush();
  r.sleeps[0]!.done.resolve(); await flush();
  const current = r.controller.join('replacement'); await flush();
  r.pollResults[0]!.reject(new SessionFailure('unauthorized')); await old;
  assert.equal(r.controller.getSnapshot().connection, 'ready'); assert.notEqual(r.controller.getSnapshot().read, null);
  r.controller.stop(); r.sleeps[1]!.done.resolve(); await current;
});
