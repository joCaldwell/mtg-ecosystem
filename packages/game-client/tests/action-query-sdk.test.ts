import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { GameClient, HttpError, ProtocolError, TransportError } from '@multiverse/game-server-sdk';
import { ActionQueryController } from '../src/session/action-query.ts';
import type { ActionQueryContext, ActionQueryEffects, ActionQueryFailure } from '../src/session/action-query.ts';

// This exercises the ACTUAL built browser SDK with synthetic HTTP responses.
// It proves serialization/validation/composition, never native legality or App UI.
const hash = (bytes: Uint8Array) => createHash('sha256').update(bytes).digest('hex');
async function qualifiedSDK(): Promise<void> {
  const sdk = new URL('../../game-server/sdk/', import.meta.url);
  const pins = JSON.parse(await readFile(new URL('../sdk-qualification.json', import.meta.url), 'utf8'));
  const manifest = JSON.parse(await readFile(new URL('dist/build.json', sdk), 'utf8'));
  assert.equal(manifest.schema_sha256, pins.protocol_schema_sha256);
  assert.equal(hash(await readFile(new URL('generated/generation.json', sdk))), manifest.generation_sha256);
  const generation = JSON.parse(await readFile(new URL('generated/generation.json', sdk), 'utf8'));
  assert.equal(generation.bootstrap_schema_sha256, pins.bootstrap_schema_sha256);
  for (const file of manifest.files) assert.equal(hash(await readFile(new URL(`dist/${file.path}`, sdk))), file.sha256);
  for (const file of manifest.sources) assert.equal(hash(await readFile(new URL(file.path, sdk))), file.sha256);
}
const owner: ActionQueryContext = { authorization: {}, generation: {}, protocolSchema: 'a7416856c2879f2d0a69531d1ccf12b4ea5f2cbd5da6811a0cc8f175650870b3', token: 'v0-d0',
  binding: { protocol_version: 1, server_origin: 'http://127.0.0.1:18973', match_id: 'a1', viewer: 0,
    engine_build_id: 'a'.repeat(64), server_build_id: 'b'.repeat(64), catalog_sha256: 'c'.repeat(64) } };
function classify(error: unknown): ActionQueryFailure {
  if (error instanceof HttpError) return error.status === 401 ? 'unauthorized' : error.status === 403 ? 'forbidden' : error.status === 429 ? 'rate-limited' : 'unavailable';
  if (error instanceof ProtocolError) return 'protocol';
  // The real SDK intentionally sanitizes header/body transport into TransportError.
  // Pure effects test the finer distinctions separately; do not invent one here.
  if (error instanceof TransportError) return 'body-transport';
  return 'request-transport';
}
function setup(fetcher: typeof globalThis.fetch) {
  const sdk = new GameClient({ baseURL: owner.binding.server_origin, matchId: 'a1', capability: 'synthetic-test-capability', fetch: fetcher });
  let now = 0;
  const effects: ActionQueryEffects = {
    current: () => owner, authorize: context => { assert.equal(context, owner); return (request, signal) => sdk.queryActions(request, { signal }); },
    classify, now: () => now, wait: async ms => { now += ms; },
    operation: ms => { const abort = new AbortController(); let timer: ReturnType<typeof setTimeout>;
      const expired = new Promise<void>(resolve => { timer = setTimeout(resolve, ms); });
      return { signal: abort.signal, expired, abort: () => abort.abort(), dispose: () => clearTimeout(timer) }; },
  };
  return new ActionQueryController(effects);
}
const cursor = 'f'.repeat(64);
const page = { token: 'v0-d0', scope: { scope: 'window' }, context: { window: 'priority' }, beginnings: [] };
const response = (outcome: unknown, viewer = 0) => Response.json({ protocol_version: 1, match_id: 'a1', viewer, outcome });

test('qualified built SDK: initialization, single-use pagination, aggregate and exact one-shot request effects', async () => {
  await qualifiedSDK(); const sent: unknown[] = [];
  const controller = setup(async (url, init) => {
    assert.equal(url, 'http://127.0.0.1:18973/v1/matches/a1/actions/query');
    assert.equal(new URL(String(url)).search, ''); assert.equal(init?.method, 'POST');
    assert.equal(new Headers(init?.headers).get('authorization'), 'Bearer synthetic-test-capability');
    const request = JSON.parse(init!.body as string); sent.push(request);
    return request.operation === 'start' ? response({ status: 'incomplete', cursor, required_minimum: 1, page: { ...page, context: null } })
      : response({ status: 'complete', page: { ...page, beginnings: [{ initiation: { action: 'pass' }, label: 'unavailable' }] } });
  });
  await controller.start(owner, { scope: 'window' });
  assert.deepEqual(sent, [{ operation: 'start', token: 'v0-d0', scope: { scope: 'window' }, page_size: 16, work: 250 }, { operation: 'continue', cursor, work: 250 }]);
  assert.equal(controller.getSnapshot().phase, 'complete'); assert.equal(controller.getSnapshot().beginnings[0]?.label, 'unavailable');
  assert.doesNotMatch(JSON.stringify(controller.getSnapshot()), /synthetic-test-capability|ffffffff|v0-d0/);
});

test('qualified built SDK: matching malformed page is rejected by SDK, foreign viewer by session', async () => {
  for (const [mode, phase] of [['duplicate-json', 'uncertain'], ['foreign-viewer', 'stale'], ['wrong-token', 'uncertain']] as const) {
    let calls = 0;
    const controller = setup(async () => { calls++;
      if (mode === 'duplicate-json') return new Response('{"protocol_version":1,"protocol_version":1}', { headers: { 'content-type': 'application/json' } });
      return response({ status: 'complete', page: { ...page, token: mode === 'wrong-token' ? 'wrong' : page.token } }, mode === 'foreign-viewer' ? 1 : 0);
    });
    await controller.start(owner, { scope: 'window' }); assert.equal(controller.getSnapshot().phase, phase); assert.equal(calls, 1);
  }
});

test('qualified built SDK: actual request/body transport failures and HTTP distinctions never cause unsafe replay', async () => {
  for (const kind of ['request', 'body', '401', '403', '429', '503'] as const) {
    let calls = 0;
    const controller = setup(async () => { calls++;
      if (kind === 'request') throw new Error('synthetic request loss');
      if (kind === 'body') return new Response(new ReadableStream({ start(stream) { stream.enqueue(new TextEncoder().encode('{')); stream.error(new Error('synthetic interrupted body')); } }), { headers: { 'content-type': 'application/json' } });
      return new Response('', { status: Number(kind) });
    });
    await controller.start(owner, { scope: 'window' }); const snapshot = controller.getSnapshot();
    assert.equal(snapshot.phase, ['401', '403', '429'].includes(kind) ? 'unavailable' : 'uncertain');
    assert.equal(calls, kind === '429' ? 3 : 1); assert.equal(snapshot.calls, calls);
    assert.doesNotMatch(JSON.stringify(snapshot), /synthetic request|interrupted|capability/);
  }
});

test('qualified built SDK: subscriber cancellation sends exactly its own private continuation', async () => {
  const operations: string[] = [];
  const controller = setup(async (_url, init) => { const request = JSON.parse(init!.body as string); operations.push(request.operation);
    if (request.operation === 'cancel') { assert.equal(request.cursor, cursor); return response({ status: 'cancelled' }); }
    return response({ status: 'incomplete', cursor, required_minimum: 1, page });
  });
  let stopped: Promise<void> | null = null;
  controller.subscribe(() => { if (controller.getSnapshot().phase === 'incomplete') stopped = controller.stop(); });
  await controller.start(owner, { scope: 'window' }); await stopped;
  assert.deepEqual(operations, ['start', 'cancel']); assert.equal(controller.getSnapshot().phase, 'cancelled');
  assert.equal(controller.getSnapshot().cancellation, 'confirmed');
});
