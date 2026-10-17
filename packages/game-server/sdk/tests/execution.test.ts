import test from 'node:test';
import assert from 'node:assert/strict';
import { GameClient, ProtocolError, TransportError } from '../src/client.ts';
import { execution } from '../src/validation.ts';

const commandId = '0123456789abcdef0123456789abcdef';
const capability = 'private-execution-seat-capability';
const json = value => new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } });
const client = fetch => new GameClient({ baseURL: 'https://engine.example', matchId: 'abc', capability, fetch });

test('execution statuses are Rust-owned exact variants, separate from host lifecycle', async () => {
  const statuses = ['pending', 'consumed_by_rules', 'discarded_by_departure', 'cancelled'].map(status => ({ status }));
  for (const reason of ['stale_revision', 'invalid_response', 'already_finished', 'resource_paused', 'cancelled']) statuses.push({ status: 'rejected', reason });
  for (const status of statuses) {
    const expected = { command_id: commandId, status };
    const calls = [];
    const sdk = client(async (url, init) => { calls.push({ url, init }); return json(expected); });
    assert.deepEqual(await sdk.execution(commandId), expected);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].url, `https://engine.example/v1/matches/abc/commands/${commandId}`);
    assert.equal(calls[0].init.method, 'GET');
    assert.equal(calls[0].init.body, undefined);
    assert.equal(calls[0].init.headers.Authorization, `Bearer ${capability}`);
    assert(!calls[0].url.includes(capability));
  }
  for (const status of [
    { status: 'waiting' }, { status: 'running' }, { status: 'applied' }, { status: 'rejected' },
    { status: 'pending', reason: 'cancelled' }, { status: 'rejected', reason: 'invented' },
    { status: 'consumed_by_rules', snapshot: {} },
  ]) assert.throws(() => execution({ command_id: commandId, status }), ProtocolError);
  assert.throws(() => execution({ command_id: commandId, status: { status: 'pending' }, viewer: 1 }), ProtocolError);
});

test('cancel POST has no body and preserves an already accepted immutable submission outcome', async () => {
  for (const expected of [
    { status: 'rejected', command_id: commandId, reason: 'cancelled' },
    { status: 'accepted', command_id: commandId },
  ]) {
    const calls = [];
    const sdk = client(async (url, init) => { calls.push({ url, init }); return json(expected); });
    assert.deepEqual(await sdk.cancel(commandId), expected);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].url, `https://engine.example/v1/matches/abc/commands/${commandId}/cancel`);
    assert.equal(calls[0].init.method, 'POST');
    assert.equal(calls[0].init.body, undefined);
    assert.equal(calls[0].init.headers['Content-Type'], undefined);
    assert.equal(calls[0].init.headers.Authorization, `Bearer ${capability}`);
    assert(!('undone' in expected), 'SDK does not manufacture cancellation or reversal facts');
  }
});

test('execution and cancellation bind safe path identifiers and exact returned command ID', async () => {
  let calls = 0;
  const sdk = client(async () => { calls++; return json({}); });
  for (const id of ['', 'a', 'a'.repeat(31), 'a'.repeat(33), 'A'.repeat(32), '../'.repeat(11), 'a'.repeat(31) + '/', 'a'.repeat(31) + '\n']) {
    await assert.rejects(sdk.execution(id), ProtocolError);
    await assert.rejects(sdk.cancel(id), ProtocolError);
  }
  assert.equal(calls, 0);
  await assert.rejects(client(async () => json({ command_id: 'f'.repeat(32), status: { status: 'pending' } })).execution(commandId), /different command identifier/);
  await assert.rejects(client(async () => json({ command_id: 'f'.repeat(32), status: 'accepted' })).cancel(commandId), /different command identifier/);
});

test('execution/cancel retain abort, redaction and no automatic retry semantics', async () => {
  let calls = 0;
  const sdk = client(async () => { calls++; throw new Error(capability); });
  for (const method of ['execution', 'cancel']) {
    const controller = new AbortController(); controller.abort();
    await assert.rejects(sdk[method](commandId, { signal: controller.signal }), { name: 'AbortError' });
  }
  assert.equal(calls, 0);
  for (const method of ['execution', 'cancel']) {
    const before = calls;
    await assert.rejects(sdk[method](commandId), error => error instanceof TransportError && !String(error).includes(capability));
    assert.equal(calls, before + 1);
  }
});

test('new execution route retains strict raw JSON and bounded response decoding', async () => {
  const duplicate = `{"command_id":"${commandId}","status":{"status":"pending","\\u0073tatus":"pending"}}`;
  const sdk = client(async () => new Response(duplicate, { headers: { 'content-type': 'application/json' } }));
  await assert.rejects(sdk.execution(commandId), /Duplicate/);
  const bounded = new GameClient({ baseURL: 'https://engine.example', matchId: 'abc', capability, maximumResponseBytes: 8, fetch: async () => json({ command_id: commandId, status: { status: 'pending' } }) });
  await assert.rejects(bounded.execution(commandId), /exceeds limit/);
});

test('constructor identifiers and capabilities reject terminal-newline anchor aliases', () => {
  for (const matchId of ['abc\n', 'abc\r', 'a'.repeat(64) + '\n']) {
    assert.throws(() => new GameClient({ baseURL: 'https://engine.example', matchId, capability }), /match identifier/);
  }
  for (const secret of [capability + '\n', capability + '\r']) {
    assert.throws(() => new GameClient({ baseURL: 'https://engine.example', matchId: 'abc', capability: secret }), /capability/);
  }
});
