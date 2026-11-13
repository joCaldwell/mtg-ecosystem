import test from 'node:test';
import assert from 'node:assert/strict';
import { GameClient, ProtocolError, parseSeatBootstrap } from '../src/client.ts';
import { resume } from '../src/validation.ts';

const fixture = () => ({ protocol_version: 1, match_id: 'abc', viewer: 0, engine_build_id: 'e'.repeat(64), server_build_id: 'f'.repeat(64),
  catalog_sha256: 'd'.repeat(64), next: '18446744073709551615', state: { status: 'pending' }, lifecycle: null, outcome: null });
const bootstrap = () => ({ ...Object.fromEntries(Object.entries(fixture()).filter(([key]) => !['next', 'state', 'lifecycle', 'outcome'].includes(key))),
  bootstrap_version: 1, server_origin: 'http://127.0.0.1:18741', capability: 'a'.repeat(64) });
const client = (fetch, maximumResponseBytes) => new GameClient({ baseURL: 'http://127.0.0.1:18741', matchId: 'abc', capability: 'a'.repeat(64), fetch,
  ...(maximumResponseBytes ? { maximumResponseBytes } : {}) });
const json = value => new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } });

test('resume keeps the permitted view through lifecycle changes and rejects inconsistent viewer/state shapes', () => {
  // Minimal structural fixture, with no card facts or claimed rules legality.
  const observation = { observation_version: 16, decision: null, view: {
    active: 0, priority: 0, viewer: 0, turn_owner: 0, live_turn_order: [0, 1, 2, 3],
    turn: { number: 1, lands_played: 0, step: 'precombat_main' },
    players: [], combat: [], commanders: [], objects: [], stack: [],
  } };
  const value = { ...fixture(), state: { status: 'available', observation }, lifecycle: { status: 'running' } };
  assert.deepEqual(resume(value), value);
  assert.throws(() => resume({ ...value, viewer: 1 }), /different viewer/);
  assert.throws(() => resume({ ...value, state: { status: 'pending', observation } }), ProtocolError);
  assert.throws(() => resume({ ...value, state: { status: 'available' } }), ProtocolError);
});

test('resume reads once, correlates match, and preserves pending/lifecycle/outcome independently', async () => {
  let calls = 0;
  const sdk = client(async (url, init) => {
    calls++;
    assert.equal(url, 'http://127.0.0.1:18741/v1/matches/abc/resume');
    assert.equal(init.method, 'GET'); assert.equal(init.body, undefined);
    assert.equal(init.headers.Authorization, `Bearer ${'a'.repeat(64)}`);
    return json(fixture());
  });
  assert.deepEqual(await sdk.resume(), fixture()); assert.equal(calls, 1);
  for (const value of [{ ...fixture(), match_id: 'def' }, { ...fixture(), protocol_version: 2 }, { ...fixture(), hidden_state: {} }]) {
    await assert.rejects(client(async () => json(value)).resume(), ProtocolError);
  }
  const terminal = { ...fixture(), lifecycle: { status: 'finished' }, outcome: { outcome: 'win', player: 0 } };
  assert.deepEqual(resume(terminal), terminal);
});

test('resume has an inclusive 128KiB wire bound respecting smaller caller limits and abort', async () => {
  const text = JSON.stringify(fixture());
  const atLimit = ' '.repeat(131_072 - text.length) + text;
  const raw = text => new Response(text, { headers: { 'content-type': 'application/json' } });
  assert.deepEqual(await client(async () => raw(atLimit)).resume(), fixture());
  await assert.rejects(client(async () => raw(' ' + atLimit)).resume(), /exceeds limit/);
  await assert.rejects(client(async () => raw(text), text.length - 1).resume(), /exceeds limit/);
  let cancelled = false;
  const body = new ReadableStream({ cancel() { cancelled = true; } });
  await assert.rejects(client(async () => new Response(body, { headers: { 'content-type': 'application/json', 'content-length': '131073' } })).resume(), /exceeds limit/);
  assert(cancelled);
  const controller = new AbortController(); controller.abort();
  await assert.rejects(client(async () => { throw new Error('must not send'); }).resume({ signal: controller.signal }), error => error.name === 'AbortError');
});

test('bootstrap validates Rust shape, bounded raw JSON and canonical numeric loopback origins without exposing secrets', () => {
  const source = bootstrap();
  assert.deepEqual(parseSeatBootstrap(JSON.stringify(source)), source);
  for (const server_origin of ['http://127.0.0.1:80', 'http://[::1]:18741', 'http://127.0.0.1:65535']) {
    assert.equal(parseSeatBootstrap(JSON.stringify({ ...source, server_origin })).server_origin, server_origin);
  }
  for (const value of [{ ...source, capability: source.capability + '\n' }, { ...source, viewer: 4 }, { ...source, operator: 'secret' },
    { ...source, bootstrap_version: 2 }, { ...source, protocol_version: 2 },
    ...['http://127.0.0.1:65536', 'http://127.0.0.1:0', 'http://127.0.0.1:018741', 'http://localhost:18741',
      'https://127.0.0.1:18741', 'http://127.0.0.1:18741\n', 'http://127.0.0.1:18741/'].map(server_origin => ({ ...source, server_origin })),
  ]) assert.throws(() => parseSeatBootstrap(JSON.stringify(value)), error => error instanceof ProtocolError && !String(error).includes(source.capability));
  const text = JSON.stringify(source);
  assert.deepEqual(parseSeatBootstrap(' '.repeat(4096 - text.length) + text), source);
  assert.throws(() => parseSeatBootstrap(' '.repeat(4097 - text.length) + text), /exceeds limit/);
  assert.throws(() => parseSeatBootstrap(text.replace('"viewer":0', '"viewer":0,"viewer":0')), /Duplicate/);
  assert.throws(() => parseSeatBootstrap(text.replace('"viewer":0', '"viewer":0e0')), /Noncanonical/);
});
