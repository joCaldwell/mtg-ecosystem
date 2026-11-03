import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { GameClient, ProtocolError, HttpError, TransportError, counter } from '../src/client.ts';
import { command, stream } from '../src/validation.ts';

const commandId = '0123456789abcdef0123456789abcdef';
const capability = 'test-seat-capability-never-in-a-url';
const envelope = (response = { response: 'pass' }) => ({ protocol_version: 1, command_id: commandId, expected_revision: '0', command: { kind: 'decision', token: 'private-current-token', response } });
const page = (cursors = ['1'], after = '0') => ({ protocol_version: 1, entries: cursors.map(cursor => ({ cursor, publication: { kind: 'lifecycle', status: { status: 'running' } } })), next: cursors.at(-1) ?? after, has_more: false });
const json = value => new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } });
const client = fetch => new GameClient({ baseURL: 'https://engine.example', matchId: 'a'.repeat(32), capability, fetch });

const responses = [
  { response: 'legend_rule', keep: 'legend-handle' },
  { response: 'mulligan', take: false }, { response: 'bottom_opening', objects: ['h'] },
  { response: 'damage_allocation', allocations: [['d', '3']] }, { response: 'modes', mode: 0 },
  { response: 'damage_effect', effect: 'e', damage: 'd' }, { response: 'order_triggers', order: ['t'] },
  { response: 'entity_targets', entities: [{ entity: 'player', player: 1 }] },
  { response: 'stack_targets', entries: ['stack'] }, { response: 'activate_ability', object: 'h', ability: 0 },
  { response: 'player_targets', players: [1] }, { response: 'sacrifice', object: 'h' },
  { response: 'resolve_discard', objects: ['h'] }, { response: 'spend_mana', spending: { W: '1' } },
  { response: 'commander_return', to_command: true }, { response: 'attackers', attacks: [['h', 1]] },
  { response: 'blockers', blocks: [['b', 'a']] }, { response: 'damage', assignments: [['h', '90071992547409931234567890']] },
  { response: 'discard', objects: ['h'] }, { response: 'play_land', object: 'h' },
  { response: 'activate_mana', object: 'h' }, { response: 'cast', card: 'h' }, { response: 'pass' },
  { response: 'targets', objects: ['h', 'h', 'h'] }, { response: 'finish_mana', undo_mana: true },
];

test('Rust schemas cover every response variant and reject whole nested source shapes', () => {
  assert.equal(responses.length, 25);
  for (const response of responses) {
    const input = envelope(response); const before = JSON.stringify(input);
    assert.deepEqual(command(input), input);
    assert.equal(JSON.stringify(input), before, 'validation must not coerce or remove fields');
    assert.throws(() => command(envelope({ ...response, raw_receipt: {} })), ProtocolError);
  }
  for (const response of [
    { response: 'damage', assignments: [['h']] }, { response: 'damage', assignments: [['h', '3', 'extra']] },
    { response: 'damage', assignments: [['h', 3]] }, { response: 'cast', card: { card: 0, incarnation: 0 } },
    { response: 'spend_mana', spending: { Z: '1' } }, { response: 'modes', mode: 4_294_967_296 },
    { response: 'player_targets', players: [256] }, { response: 'raw_receipt', receipt: {} },
  ]) assert.throws(() => command(envelope(response)), ProtocolError);
  for (const quantity of ['01', '-0', '1\n', '\n1', '1\r', '1.0', '+1']) {
    assert.throws(() => command(envelope({ response: 'damage', assignments: [['h', quantity]] })), ProtocolError);
  }
  for (const revision of ['01', '-0', '1\n', '\n1', '1\r', '+1', '1.0', '18446744073709551616']) {
    assert.throws(() => counter(revision), ProtocolError);
    assert.throws(() => command({ ...envelope(), expected_revision: revision }), ProtocolError);
  }
  for (const revision of ['0', '9007199254740992', '18446744073709551615']) assert.equal(counter(revision), revision);
  assert.throws(() => command({ ...envelope(), protocol_version: 2 }), ProtocolError);
});

test('private capability is header-only, commands are never retried, and errors are redacted', async () => {
  const calls = [];
  const sdk = client(async (url, init) => { calls.push({ url, init }); return json({ status: 'accepted', command_id: commandId }); });
  const result = await sdk.submit(envelope());
  assert.equal(result.status, 'accepted'); assert.equal(calls.length, 1);
  const { url, init } = calls[0];
  assert.equal(url, `https://engine.example/v1/matches/${'a'.repeat(32)}/commands`);
  assert(!url.includes(capability)); assert.equal(init.headers.Authorization, `Bearer ${capability}`);
  assert.equal(init.credentials, 'omit'); assert.equal(init.redirect, 'error'); assert.equal(init.cache, 'no-store');
  assert.equal(init.referrerPolicy, 'no-referrer'); assert.equal(JSON.stringify(sdk), '{}');
  let failures = 0;
  const broken = client(async () => { failures++; throw new Error(capability); });
  await assert.rejects(broken.submit(envelope()), error => error instanceof TransportError && !String(error).includes(capability));
  assert.equal(failures, 1);
  await assert.rejects(client(async () => new Response(capability, { status: 503 })).submit(envelope()), error => error instanceof HttpError && error.status === 503 && !String(error).includes(capability));
  await assert.rejects(client(async () => json({ status: 'accepted', command_id: 'f'.repeat(32) })).submit(envelope()), ProtocolError);
});

test('long polling preserves exact cursor and rejects reordered, duplicate, missing or future pages', async () => {
  let seen;
  const max = '18446744073709551615';
  const sdk = client(async (url, init) => { seen = { url, init }; return json(page([], max)); });
  const actual = await sdk.poll({ after: max }); assert.equal(actual.next, max);
  const url = new URL(seen.url); assert.equal(url.searchParams.get('after'), max);
  assert.equal(url.searchParams.get('limit'), '32'); assert.equal(url.searchParams.get('wait_ms'), '20000');
  assert(!url.href.includes(capability)); assert.equal(seen.init.method, 'GET'); assert.equal(seen.init.body, undefined);
  for (const invalid of [page(['1', '1']), page(['2', '1']), { ...page(['1']), next: '2' }, { ...page([], '0'), next: '1' }, { ...page(['1']), protocol_version: 2 }, { ...page(['1']), raw_state: {} }]) {
    assert.throws(() => stream(invalid, '0'), ProtocolError);
  }
  await assert.rejects(sdk.poll({ after: '0', limit: 33 }), /bounds/);
  await assert.rejects(sdk.poll({ after: '0', waitMs: 20_001 }), /bounds/);
});

function observation() {
  return { observation_version: 15, decision: null, view: {
    active: 0, priority: 0, viewer: 0, turn_owner: 0, live_turn_order: [0, 1, 2, 3],
    turn: { number: 1, lands_played: 0, step: 'precombat_main' },
    players: [{ player: 0, life: '90071992547409931234567890', hand_count: 7, library_count: 92, mana: {}, status: 'live' }],
    combat: [], commanders: [], objects: [], stack: [],
  } };
}
test('outbound serialize schema requires explicit nullable handles and safe numbers, preserving exact life', () => {
  const value = page(['1']); value.entries[0].publication = { kind: 'view_replaced', observation: observation() };
  assert.equal(stream(value, '0').entries[0].publication.observation.view.players[0].life, '90071992547409931234567890');
  const request = { request: 'commander_return', token: 'token', player: 0, designation: 'commander', destination: { zone: 'hand', owner: 0 }, object: null };
  value.entries[0].publication.observation.decision = request;
  stream(value, '0');
  delete request.object;
  assert.throws(() => stream(value, '0'), ProtocolError);
  request.object = null;
  value.entries[0].publication.observation.view.turn.number = Number.MAX_SAFE_INTEGER + 1;
  assert.throws(() => stream(value, '0'), ProtocolError);
});

test('event iteration resumes only after consumer progress and abort never sends a command', async () => {
  const requested = []; const controller = new AbortController();
  const sdk = client(async url => {
    const after = new URL(url).searchParams.get('after'); requested.push(after);
    if (after === '0') return json(page(['1', '2']));
    throw new Error('stop test');
  });
  const events = sdk.events({ after: '0', signal: controller.signal });
  assert.equal((await events.next()).value.cursor, '1'); assert.deepEqual(requested, ['0']);
  assert.equal((await events.next()).value.cursor, '2'); assert.deepEqual(requested, ['0']);
  await assert.rejects(events.next(), TransportError); assert.deepEqual(requested, ['0', '2']);
  let calls = 0; controller.abort();
  await assert.rejects(client(async () => { calls++; return json(page()); }).poll({ after: '0', signal: controller.signal }), error => error.name === 'AbortError');
  assert.equal(calls, 0);
});

test('body, media type, safe origin and invalid command limits fail explicitly', async () => {
  for (const baseURL of ['http://example.com', 'ftp://example.com', 'https://user:secret@example.com', 'https://example.com/?cap=secret', 'https://example.com/#secret']) {
    assert.throws(() => new GameClient({ baseURL, matchId: 'a', capability }), /base URL/);
  }
  assert.throws(() => new GameClient({ baseURL: 'http://localhost', matchId: '../other', capability }), /identifier/);
  assert.throws(() => new GameClient({ baseURL: 'http://localhost', matchId: 'a', capability: 'token\r\nInjected: yes' }), /capability/);
  const oversized = new GameClient({ baseURL: 'http://127.0.0.1:3000', matchId: 'a', capability, maximumResponseBytes: 4, fetch: async () => json(page()) });
  await assert.rejects(oversized.poll({ after: '0' }), /body exceeds/);
  await assert.rejects(client(async () => new Response('{}', { headers: { 'content-type': 'text/html' } })).poll({ after: '0' }), /Expected JSON/);
  await assert.rejects(client(async () => new Response('{', { headers: { 'content-type': 'application/json' } })).poll({ after: '0' }), /Invalid JSON/);
  let calls = 0;
  const sdk = client(async () => { calls++; return json({}); });
  await assert.rejects(sdk.submit(envelope({ response: 'damage', assignments: [['h']] })), ProtocolError);
  await assert.rejects(sdk.submit(envelope({ response: 'targets', objects: ['x'.repeat(65_536)] })), /body exceeds/);
  assert.equal(calls, 0);
});

test('generated declarations and schema fingerprints remain exact and unconstrained types are absent', () => {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const manifest = JSON.parse(fs.readFileSync(path.join(root, 'generated/generation.json'), 'utf8'));
  assert.equal(manifest.generator, 'json-schema-to-typescript@16.0.0');
  assert.equal(createHash('sha256').update(fs.readFileSync(path.join(root, 'generated/protocol.schema.json'))).digest('hex'), manifest.schema_sha256);
  for (const file of manifest.files) {
    const source = fs.readFileSync(path.join(root, 'generated', file.path), 'utf8');
    assert.equal(createHash('sha256').update(source).digest('hex'), file.sha256);
    assert(!/\b(?:any|unknown)\s*(?:;|\[|\|)/.test(source));
  }
});


test('response streams are cancelled on early rejection and envelope identity is frozen before await', async () => {
  let cancelled = false;
  const source = new ReadableStream({ cancel() { cancelled = true; } });
  await assert.rejects(client(async () => new Response(source, { status: 503 })).poll({ after: '0' }), HttpError);
  assert(cancelled);
  const tooMany = client(async () => json(page(['1', '2'])));
  await assert.rejects(tooMany.poll({ after: '0', limit: 1 }), /page limit/);
  assert.throws(() => stream({ ...page([], '0'), has_more: true }, '0'), ProtocolError);
  const input = envelope();
  const sdk = client(async () => { input.command_id = 'f'.repeat(32); return json({ status: 'accepted', command_id: commandId }); });
  assert.equal((await sdk.submit(input)).status, 'accepted');
  const invalidSecret = 'secret-token-containing-invalid-base';
  assert.throws(() => new GameClient({ baseURL: invalidSecret, matchId: 'a', capability }), error => !JSON.stringify(error).includes(invalidSecret) && !String(error).includes(invalidSecret));
});


test('typed rules outcomes remain separate from lifecycle resource and fault statuses', () => {
  for (const result of [{ outcome: 'win', player: 2 }, { outcome: 'draw' }]) {
    const value = page(['1']); value.entries[0].publication = { kind: 'outcome', result };
    assert.deepEqual(stream(value, '0').entries[0].publication.result, result);
  }
  for (const result of [{ outcome: 'resource_paused' }, { outcome: 'faulted' }, { outcome: 'win' }, { outcome: 'draw', player: 2 }]) {
    const value = page(['1']); value.entries[0].publication = { kind: 'outcome', result };
    assert.throws(() => stream(value, '0'), ProtocolError);
  }
  const done = page(['1']); done.entries[0].publication = { kind: 'lifecycle', status: { status: 'finished' } };
  stream(done, '0');
});
