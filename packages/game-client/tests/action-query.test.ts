import assert from 'node:assert/strict';
import test from 'node:test';
import type { ActionPage, ActionQueryRequest, ActionQueryResponse, ActionScope } from '@multiverse/game-server-sdk';
import { ActionQueryController, ACTION_QUERY_LIMITS } from '../src/session/action-query.ts';
import type { ActionQueryContext, ActionQueryEffects, ActionQueryFailure, ActionQueryLimits, QueryOperation } from '../src/session/action-query.ts';

// Synthetic generated-wire shapes test policy only; no card facts or legality.
const cursor = (n: number) => n.toString(16).padStart(64, '0');
const windowScope: ActionScope = { scope: 'window' };
const context = (): ActionQueryContext => ({ authorization: {}, generation: {}, protocolSchema: 'd'.repeat(64), token: 'synthetic-token',
  binding: { server_origin: 'http://127.0.0.1:18971', match_id: 'a1', viewer: 0, protocol_version: 1,
    engine_build_id: 'a'.repeat(64), server_build_id: 'b'.repeat(64), catalog_sha256: 'c'.repeat(64) } });
const page = (beginnings: ActionPage['beginnings'] = [], patch: Partial<ActionPage> = {}): ActionPage => ({ token: 'synthetic-token', scope: windowScope, context: { window: 'priority' }, beginnings, ...patch });
const pass: ActionPage['beginnings'][number] = { initiation: { action: 'pass' }, label: 'unavailable' };
const finish: ActionPage['beginnings'][number] = { initiation: { action: 'finish_mana', undo_mana: false }, label: 'unavailable' };
const result = (outcome: ActionQueryResponse['outcome'], patch: Partial<ActionQueryResponse> = {}): ActionQueryResponse => ({ protocol_version: 1, match_id: 'a1', viewer: 0, outcome, ...patch });
const complete = (p = page([pass])) => result({ status: 'complete', page: p });
const incomplete = (n = 1, p = page()) => result({ status: 'incomplete', cursor: cursor(n), required_minimum: 1, page: p });
const deferred = <T>() => { let resolve!: (value: T) => void; let reject!: (error: unknown) => void; const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const flush = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); };
function harness(handler: (request: ActionQueryRequest, signal: AbortSignal) => Promise<ActionQueryResponse>, limits: Partial<ActionQueryLimits> = {}) {
  let current: ActionQueryContext | null = context();
  let now = 0;
  const requests: ActionQueryRequest[] = [];
  const signals: AbortSignal[] = [];
  const waits: number[] = [];
  const operations: (QueryOperation & { expire(): void; disposed: boolean; milliseconds: number })[] = [];
  const effects: ActionQueryEffects = {
    current: () => current,
    authorize: owner => { assert.equal(owner, current); return async (request, signal) => { requests.push(request); signals.push(signal); return handler(request, signal); }; },
    now: () => now,
    operation: milliseconds => {
      const controller = new AbortController(); const expired = deferred<void>();
      const operation = { signal: controller.signal, expired: expired.promise, milliseconds, disposed: false,
        abort: () => controller.abort(), dispose() { this.disposed = true; }, expire() { now += milliseconds; expired.resolve(); } };
      operations.push(operation); return operation;
    },
    wait: async (milliseconds, signal) => { assert.equal(signal.aborted, false); waits.push(milliseconds); now += milliseconds; },
    classify: error => error as ActionQueryFailure,
  };
  const controller = new ActionQueryController(effects, limits);
  return { controller, requests, signals, waits, operations, effects, owner: current,
    replace(value: ActionQueryContext | null) { current = value; }, advance(ms: number) { now += ms; },
    start(scope = windowScope) { return controller.start(current!, scope); } };
}

test('complete snapshot is immutable, faithful, private and does not touch read/journal state', async () => {
  const supplied = complete(); const h = harness(async () => supplied);
  const unrelated = { readCursor: '99', journal: 'unresolved' };
  await h.start(); const snapshot = h.controller.getSnapshot();
  assert.equal(snapshot.phase, 'complete'); assert.deepEqual(snapshot.beginnings, [pass]);
  assert.deepEqual(snapshot.context, { window: 'priority' }); assert.equal(snapshot.calls, 1);
  assert.equal(Object.isFrozen(snapshot.beginnings[0]?.initiation), true);
  supplied.outcome = { status: 'stale' }; assert.equal(h.controller.getSnapshot(), snapshot);
  assert.doesNotMatch(JSON.stringify(snapshot), /synthetic-token|cursor|capability|authorization|engine_build/);
  assert.deepEqual(unrelated, { readCursor: '99', journal: 'unresolved' });
});

test('initialization null context, incremental pages and producer complete aggregate exactly once', async () => {
  let call = 0;
  const h = harness(async () => ++call === 1 ? incomplete(1, page([], { context: null })) : call === 2 ? incomplete(2, page([pass])) : complete(page([finish])));
  const phases: string[] = []; h.controller.subscribe(() => phases.push(h.controller.getSnapshot().phase));
  await h.start(); assert.deepEqual(h.controller.getSnapshot().beginnings, [pass, finish]);
  assert.equal(h.controller.getSnapshot().phase, 'complete'); assert.ok(phases.includes('incomplete'));
  assert.deepEqual(h.requests.map(r => r.operation), ['start', 'continue', 'continue']);
  assert.deepEqual(h.waits, [100, 100]); assert.equal(h.controller.getSnapshot().work, 750);
});

test('context and unavailable source are retained without fabricated labels', async () => {
  const context: ActionPage['context'] = { window: 'mana', cost: { generic: '9007199254740993', colored: ['G'] }, source: { status: 'unavailable' } };
  const h = harness(async () => complete(page([finish], { context })));
  await h.start(); assert.deepEqual(h.controller.getSnapshot().context, context); assert.equal(h.controller.getSnapshot().beginnings[0]?.label, 'unavailable');
});

test('all immutable authorization identity fields fence old intents', async () => {
  for (const field of ['server_origin', 'protocol_version', 'match_id', 'viewer', 'engine_build_id', 'server_build_id', 'catalog_sha256'] as const) {
    const h = harness(async () => complete()); const old = h.owner;
    const changed = { ...old.binding, [field]: field === 'viewer' || field === 'protocol_version' ? 2 : 'different' };
    h.replace({ ...old, binding: changed }); await h.controller.start(old, windowScope); assert.equal(h.requests.length, 0, field);
  }
  for (const field of ['authorization', 'generation', 'token', 'protocolSchema'] as const) {
    const h = harness(async () => complete()); const old = h.owner;
    h.replace({ ...old, [field]: field === 'token' || field === 'protocolSchema' ? 'different' : {} });
    await h.controller.start(old, windowScope); assert.equal(h.requests.length, 0, field);
  }
  const h = harness(async () => complete()); h.replace(null); await h.controller.start(h.owner, windowScope); assert.equal(h.requests.length, 0);
});

test('foreign envelope viewer/match/version never aggregates or cancels another owner cursor', async () => {
  for (const patch of [{ viewer: 1 }, { match_id: 'a2' }, { protocol_version: 2 }]) {
    const h = harness(async () => result(incomplete().outcome, patch)); await h.start();
    assert.equal(h.controller.getSnapshot().phase, 'stale'); assert.equal(h.controller.getSnapshot().cancellation, 'unknown');
    assert.deepEqual(h.controller.getSnapshot().beginnings, []); assert.equal(h.requests.length, 1);
  }
});

test('generated-valid but mismatched token/object scope/window scope are rejected on continuation too', async () => {
  for (const patch of [{ token: 'foreign' }, { scope: { scope: 'object', object: 'foreign' } as ActionScope }]) {
    let n = 0; const h = harness(async () => ++n === 1 ? incomplete() : complete(page([pass], patch)));
    await h.start(); assert.equal(h.controller.getSnapshot().phase, 'uncertain'); assert.equal(h.requests.length, 2);
  }
  const h = harness(async () => complete()); await h.start({ scope: 'object', object: 'own' }); assert.equal(h.controller.getSnapshot().phase, 'uncertain');
});

test('page consistency rejects duplicate initiations, mixed contexts, context loss and oversized matching page', async () => {
  const variants = [page([pass, pass]), page([pass], { context: null }), page([], { context: null }),
    page([finish], { context: { window: 'mana', cost: { generic: '0', colored: [] }, source: { status: 'unavailable' } } }), page(Array(17).fill(finish))];
  for (const p of variants) {
    let n = 0; const h = harness(async () => ++n === 1 ? incomplete(1, page([pass])) : complete(p));
    await h.start(); assert.equal(h.controller.getSnapshot().phase, 'uncertain'); assert.deepEqual(h.controller.getSnapshot().beginnings, []);
  }
});

test('same cursor replay is uncertain and never blindly replayed or cancelled', async () => {
  let n = 0; const h = harness(async () => ++n === 1 ? incomplete() : incomplete(1, page([pass])));
  await h.start(); assert.equal(h.controller.getSnapshot().phase, 'uncertain'); assert.equal(h.controller.getSnapshot().cancellation, 'unknown');
  assert.equal(h.requests.length, 2);
});

test('stale, cancelled and all worker unavailable reasons remain distinct from complete empty menu', async () => {
  for (const status of ['stale', 'cancelled'] as const) { const h = harness(async () => result({ status })); await h.start(); assert.equal(h.controller.getSnapshot().phase, status); }
  for (const reason of [{ kind: 'core', reason: 'unsupported_window' }, { kind: 'quota' }, { kind: 'in_flight' }, { kind: 'total_work' }, { kind: 'response_limit' }, { kind: 'executor' }] as const) {
    const h = harness(async () => result({ status: 'unavailable', reason }), { retries: 0 }); await h.start();
    assert.equal(h.controller.getSnapshot().phase, 'unavailable'); assert.deepEqual(h.controller.getSnapshot().unavailable, reason);
  }
  const h = harness(async () => complete(page())); await h.start(); assert.equal(h.controller.getSnapshot().phase, 'complete');
});

test('429 and explicit worker quota retry only within the original charged call/work budget', async () => {
  let call = 0; const h = harness(async () => { call++; if (call === 1) return incomplete(); if (call === 2) throw 'rate-limited'; if (call === 3) return result({ status: 'unavailable', reason: { kind: 'quota' } }); return complete(); });
  await h.start(); assert.equal(h.controller.getSnapshot().phase, 'complete'); assert.equal(h.requests.length, 4);
  assert.equal(h.controller.getSnapshot().work, 1000); assert.deepEqual(h.requests.slice(1).map(r => 'cursor' in r ? r.cursor : ''), [cursor(1), cursor(1), cursor(1)]);
  const bounded = harness(async () => { throw 'rate-limited'; }, { calls: 2 }); await bounded.start();
  assert.equal(bounded.requests.length, 2); assert.equal(bounded.controller.getSnapshot().phase, 'limited');
});

test('401/403/503/protocol/request/body errors have no hidden retry or cursor replay', async () => {
  for (const failure of ['unauthorized', 'forbidden', 'unavailable', 'protocol', 'request-transport', 'body-transport'] as const) {
    let n = 0; const h = harness(async r => { if (r.operation === 'cancel') return result({ status: 'cancelled' }); if (++n === 1) return incomplete(); throw failure; });
    await h.start(); const snapshot = h.controller.getSnapshot(); assert.equal(snapshot.failure, failure);
    assert.equal(snapshot.phase, failure === 'unauthorized' || failure === 'forbidden' ? 'unavailable' : 'uncertain');
    assert.equal(h.requests.filter(r => r.operation === 'continue').length, 1);
    if (snapshot.phase === 'uncertain') { assert.equal(snapshot.cancellation, 'unknown'); assert.equal(h.requests.length, 2); }
  }
});

test('entry/byte/call/work limits retain incompleteness and release known own cursor with one bounded cancel', async () => {
  for (const limits of [{ entries: 1 }, { bytes: 1 }, { calls: 1 }, { work: 250 }]) {
    let n = 0; const h = harness(async r => r.operation === 'cancel' ? result({ status: 'cancelled' }) : incomplete(++n, page(n === 1 ? [pass] : [finish])), limits);
    await h.start(); const snapshot = h.controller.getSnapshot(); assert.equal(snapshot.phase, 'limited'); assert.equal(snapshot.cancellation, 'confirmed');
    assert.equal(h.requests.filter(r => r.operation === 'cancel').length, 1); assert.deepEqual(snapshot.beginnings, []);
  }
});

test('TTL is absolute across paging/quota waits and never resets on success', async () => {
  const h = harness(async r => r.operation === 'cancel' ? result({ status: 'cancelled' }) : incomplete(h.requests.length), { lifetimeMs: 250 });
  await h.start(); assert.equal(h.controller.getSnapshot().phase, 'limited'); assert.equal(h.controller.getSnapshot().failure, 'deadline');
  assert.equal(h.requests.filter(r => r.operation === 'continue').length, 2);
  assert.equal(h.requests.at(-1)?.operation, 'cancel');
});

test('token/handle request bounds and invalid client budgets block before query effects', async () => {
  for (const token of ['', 'bad token', 'x'.repeat(129)]) { const h = harness(async () => complete()); h.replace({ ...h.owner, token }); await h.start(); assert.equal(h.requests.length, 0); assert.equal(h.controller.getSnapshot().failure, 'request-limit'); }
  const h = harness(async () => complete()); await h.start({ scope: 'object', object: 'x'.repeat(129) }); assert.equal(h.requests.length, 0);
  for (const key of Object.keys(ACTION_QUERY_LIMITS) as (keyof ActionQueryLimits)[]) assert.throws(() => new ActionQueryController(h.effects, { [key]: ACTION_QUERY_LIMITS[key] + 1 }));
});

test('abort before learning cursor is unknown; a late own rotated cursor is cancelled without publication', async () => {
  const pending = deferred<ActionQueryResponse>();
  const h = harness(async r => r.operation === 'cancel' ? result({ status: 'cancelled' }) : pending.promise);
  const started = h.start(); await flush(); await h.controller.stop(); assert.equal(h.signals[0]?.aborted, true);
  assert.equal(h.controller.getSnapshot().phase, 'cancelled'); assert.equal(h.controller.getSnapshot().cancellation, 'unknown');
  pending.resolve(incomplete()); await started; await flush(); assert.equal(h.controller.getSnapshot().phase, 'cancelled');
  assert.equal(h.controller.getSnapshot().cancellation, 'confirmed'); assert.equal(h.requests.at(-1)?.operation, 'cancel');
});

test('no cursor learned after abort never claims server cancellation; deadline bounds abort-ignoring fetch', async () => {
  const pending = deferred<ActionQueryResponse>(); const h = harness(async () => pending.promise);
  const started = h.start(); await flush(); h.operations[0]!.expire(); await started;
  assert.equal(h.signals[0]?.aborted, true); assert.equal(h.controller.getSnapshot().phase, 'uncertain');
  assert.equal(h.controller.getSnapshot().cancellation, 'unknown'); assert.equal(h.requests.length, 1);
  pending.reject('body-transport'); await flush();
});

test('seat change cancels only captured old transport and cannot publish private old pages', async () => {
  const pending = deferred<ActionQueryResponse>(); const h = harness(async r => r.operation === 'cancel' ? result({ status: 'cancelled' }) : pending.promise);
  const started = h.start(); await flush(); h.replace({ ...context(), binding: { ...h.owner.binding, viewer: 1 } });
  h.controller.synchronize(); assert.equal(h.signals[0]?.aborted, true); assert.deepEqual(h.controller.getSnapshot().beginnings, []);
  pending.resolve(incomplete(1, page([pass]))); await started; await flush(); assert.equal(h.controller.getSnapshot().phase, 'stale');
  assert.equal(h.requests.at(-1)?.operation, 'cancel'); // captured old closure, never reauthorized as viewer 1
});

test('repeated start/stop StrictMode and late cleanup cannot stop replacement query', async () => {
  const pending = deferred<ActionQueryResponse>(); let call = 0;
  const h = harness(async r => { if (r.operation === 'cancel') return result({ status: 'cancelled' }); return ++call === 1 ? pending.promise : complete(); });
  const first = h.start(); await flush(); await h.start(); assert.equal(h.requests.length, 1);
  const old = h.controller.getSnapshot().generation; await h.controller.stop(); await h.controller.stop();
  await h.start(); const snapshot = h.controller.getSnapshot(); await h.controller.cancel(old);
  pending.resolve(incomplete()); assert.equal(await first, old); await flush(); assert.equal(h.controller.getSnapshot(), snapshot); assert.equal(snapshot.phase, 'complete');
});

test('scope replacement fences pending response and captures a new bounded generation', async () => {
  const pending = deferred<ActionQueryResponse>(); let call = 0;
  const h = harness(async r => r.operation === 'cancel' ? result({ status: 'cancelled' }) : ++call === 1 ? pending.promise : complete(page([], { scope: { scope: 'object', object: 'synthetic' } })));
  const old = h.start(); await flush(); await h.start({ scope: 'object', object: 'synthetic' }); const snapshot = h.controller.getSnapshot();
  pending.resolve(incomplete()); await old; await flush(); assert.equal(h.controller.getSnapshot(), snapshot); assert.equal(snapshot.generation, 2);
});

test('synchronous subscription stop at loading/incomplete/retry publication fences subsequent I/O', async () => {
  for (const trigger of ['loading', 'incomplete', 'unavailable'] as const) {
    const h = harness(async r => r.operation === 'cancel' ? result({ status: 'cancelled' }) : trigger === 'unavailable' ? result({ status: 'unavailable', reason: { kind: 'quota' } }) : incomplete());
    h.controller.subscribe(() => { const snapshot = h.controller.getSnapshot(); if ((trigger === 'unavailable' && snapshot.unavailable) || snapshot.phase === trigger) void h.controller.stop(); });
    await h.start(); await flush(); assert.equal(h.controller.getSnapshot().phase, 'cancelled');
    assert.equal(h.requests.filter(r => r.operation !== 'cancel').length, trigger === 'loading' ? 0 : 1);
  }
});

test('cancellation is bounded and stale/foreign/failing responses cannot claim confirmed cancellation', async () => {
  for (const disposition of ['stale', 'foreign', 'error', 'timeout'] as const) {
    const pending = deferred<ActionQueryResponse>();
    const h = harness(async r => { if (r.operation !== 'cancel') return incomplete(); if (disposition === 'error') throw 'body-transport'; if (disposition === 'timeout') return pending.promise;
      return result({ status: disposition === 'stale' ? 'stale' : 'cancelled' }, disposition === 'foreign' ? { viewer: 1 } : {}); });
    h.controller.subscribe(() => { if (h.controller.getSnapshot().phase === 'incomplete') void h.controller.stop(); });
    const started = h.start(); await flush(); if (disposition === 'timeout') h.operations.at(-1)!.expire(); await started; await flush();
    assert.equal(h.controller.getSnapshot().cancellation, disposition === 'stale' ? 'stale' : 'unknown'); assert.equal(h.requests.length, 2);
  }
});

test('queued replacement start is fenced by a later stop while cancellation is pending', async () => {
  const cancelled = deferred<ActionQueryResponse>();
  const waiting = deferred<void>();
  const h = harness(async r => r.operation === 'cancel' ? cancelled.promise : incomplete());
  h.effects.wait = () => waiting.promise;
  const first = h.start(); await flush();
  const second = h.start({ scope: 'object', object: 'synthetic' }); await flush();
  const stopped = h.controller.stop(); cancelled.resolve(result({ status: 'cancelled' })); waiting.resolve();
  await Promise.all([first, second, stopped]); assert.equal(h.requests.filter(r => r.operation === 'start').length, 1);
  assert.equal(h.controller.getSnapshot().phase, 'cancelled');
});

test('deadline after cursor rotation cancels only the newly learned late cursor', async () => {
  const pending = deferred<ActionQueryResponse>(); let n = 0;
  const h = harness(async r => r.operation === 'cancel' ? result({ status: 'cancelled' }) : ++n === 1 ? incomplete() : pending.promise);
  const started = h.start(); await flush(); h.operations.at(-1)!.expire(); await started;
  assert.equal(h.controller.getSnapshot().phase, 'uncertain'); assert.equal(h.requests.length, 2);
  pending.resolve(incomplete(2, page([pass]))); await flush();
  assert.deepEqual(h.requests.at(-1), { operation: 'cancel', cursor: cursor(2) });
  assert.equal(h.controller.getSnapshot().cancellation, 'confirmed'); assert.deepEqual(h.controller.getSnapshot().beginnings, []);
});

test('late foreign binding cursor is never cancelled; null read context revokes visible pages', async () => {
  const pending = deferred<ActionQueryResponse>(); const h = harness(async () => pending.promise);
  const started = h.start(); await flush(); h.replace(null); h.controller.synchronize();
  pending.resolve(result(incomplete().outcome, { viewer: 1 })); await started; await flush();
  assert.equal(h.requests.length, 1); assert.equal(h.controller.getSnapshot().phase, 'stale');
  const completed = harness(async () => complete()); await completed.start(); completed.replace(null); completed.controller.synchronize();
  assert.equal(completed.controller.getSnapshot().phase, 'stale'); assert.deepEqual(completed.controller.getSnapshot().beginnings, []);
});

test('canonical initiation equality rejects reordered wire objects and quota result bytes are bounded', async () => {
  let n = 0; const h = harness(async () => ++n === 1 ? incomplete(1, page([finish])) : complete(page([{ label: 'unavailable', initiation: { undo_mana: false, action: 'finish_mana' } }])));
  await h.start(); assert.equal(h.controller.getSnapshot().phase, 'uncertain');
  const limited = harness(async () => result({ status: 'unavailable', reason: { kind: 'quota' } }), { bytes: 1 });
  await limited.start(); assert.equal(limited.requests.length, 1); assert.equal(limited.controller.getSnapshot().phase, 'limited');
});

test('intent binding is captured before awaiting predecessor cancellation', async () => {
  const cancelled = deferred<ActionQueryResponse>(); const waiting = deferred<void>();
  const h = harness(async r => r.operation === 'cancel' ? cancelled.promise : incomplete()); h.effects.wait = () => waiting.promise;
  const first = h.start(); await flush();
  const mutable = { ...h.owner }; h.replace(mutable);
  const second = h.controller.start(mutable, { scope: 'object', object: 'synthetic' }); await flush();
  mutable.token = 'replacement-token'; cancelled.resolve(result({ status: 'cancelled' })); waiting.resolve();
  await Promise.all([first, second]); assert.equal(h.requests.filter(r => r.operation === 'start').length, 1);
});
