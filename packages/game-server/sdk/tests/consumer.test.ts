import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { CONSUMER_LIMITS, foundationsPolicy, parseBootstrap, runFoundationsConsumer } from '../src/examples/foundations-consumer.ts';

const catalog = JSON.parse(fs.readFileSync(new URL('../../../game-engine/tests/fixtures/expected-match-ir.json', import.meta.url), 'utf8'));
const oracle = name => catalog.definitions.find(card => card.name === name).oracle_id;
const deck = { commander: oracle('Isamaru, Hound of Konda'), cards: ['Isamaru, Hound of Konda', 'Yoked Ox', 'Samite Healer', 'Sacred Nectar', 'Healing Hands', 'Healing Salve', 'Plains'].map(name => ({ oracle_id: oracle(name), count: name === 'Plains' ? 94 : 1 })) };
const publicManifest = () => ({ protocol_version: 1, match_id: 'abc', engine_build_id: 'b'.repeat(64), catalog_sha256: 'fc4bd3390e63ea352e129ac370fa848dbee11cf0700e7e6781d449fd49244a7f', profile: 'commander-foundations-v1', decks: Array.from({ length: 4 }, () => structuredClone(deck)), starting_player: 0, catalog: structuredClone(catalog) });
const bootstrap = viewer => ({ baseURL: 'http://127.0.0.1:3000', matchId: 'abc', capability: 'c'.repeat(64), viewer });
const observation = (viewer, decision) => ({ observation_version: 15, decision, view: { viewer, active: 0, turn_owner: 0, turn: { number: 1, step: 'precombat_main', lands_played: 0 }, live_turn_order: [0, 1, 2, 3], commanders: [], priority: 0, players: [], objects: [], stack: [], combat: [] } });
const view = (viewer, token, request = 'priority', extra = {}) => ({ kind: 'view_replaced', observation: observation(viewer, { request, player: viewer, token, ...extra }) });
const lifecycle = status => ({ kind: 'lifecycle', status: { status } });
const page = (cursor, publication, more = false) => ({ protocol_version: 1, entries: [{ cursor: String(cursor), publication }], next: String(cursor), has_more: more });
const outcome = () => ({ kind: 'outcome', result: { outcome: 'draw' } });
const json = value => new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } });

function transport(pages, submit = body => json({ status: 'accepted', command_id: body.command_id }), manifest = publicManifest) {
  const bodies = [], polls = [];
  let manifests = 0;
  return {
    bodies, polls,
    fetch: async (address, init) => {
      const url = new URL(address);
      if (url.pathname.endsWith('/manifest')) return json(manifest(manifests++));
      if (url.pathname.endsWith('/events')) {
        polls.push(url.searchParams.get('after'));
        const next = pages.shift(); assert(next, 'fixture must supply every actual poll');
        return json(typeof next === 'function' ? next() : next);
      }
      assert(url.pathname.endsWith('/commands'));
      bodies.push(init.body);
      return submit(JSON.parse(init.body), bodies.length);
    },
  };
}

test('finite public policy keeps, passes, declares empty combat and uses only first offered discard handles', () => {
  const choose = (request, extra = {}) => foundationsPolicy(observation(2, { request, player: 2, token: 'private-token', ...extra }), 2);
  assert.deepEqual(choose('mulligan', { can_mulligan: true, mulligans_taken: 0, bottom_if_mulligan: 0 }), { response: 'mulligan', take: false });
  assert.deepEqual(choose('priority'), { response: 'pass' });
  assert.deepEqual(choose('attackers', { candidates: ['a'], defenders: [0] }), { response: 'attackers', attacks: [] });
  assert.deepEqual(choose('blockers', { candidates: ['b'], attackers: ['a'] }), { response: 'blockers', blocks: [] });
  assert.deepEqual(choose('commander_return'), { response: 'commander_return', to_command: true });
  assert.deepEqual(choose('discard', { candidates: ['private-a', 'private-b', 'private-c'], count: 2 }), { response: 'discard', objects: ['private-a', 'private-b'] });
  assert.equal(foundationsPolicy(observation(2, null), 2), null);
  assert.throws(() => choose('discard', { candidates: ['a'], count: 2 }), /invalid_candidates/);
  assert.throws(() => choose('discard', { candidates: ['a', 'a'], count: 1 }), /invalid_candidates/);
  assert.throws(() => choose('bottom_opening', { count: 1, candidates: ['a'] }), /unexpected_request/);
  assert.throws(() => foundationsPolicy(observation(1, null), 2), /foreign_observation/);
});

test('drains pages before answering, preserves own view across lifecycle pages and never gives one token another ID', async () => {
  const t = transport([
    page(1, view(2, 'private-first-token'), true), page(2, lifecycle('waiting')),
    page(3, lifecycle('running')), page(4, view(2, 'private-second-token')), page(5, outcome()),
  ]);
  const report = await runFoundationsConsumer(bootstrap(2), { fetch: t.fetch });
  assert.equal(report.status, 'completed');
  assert.deepEqual(report.result, { outcome: 'draw' });
  assert.equal(report.counts.accepted_inputs, 2);
  assert.equal(t.bodies.length, 2);
  const [a, b] = t.bodies.map(JSON.parse);
  assert.equal(a.expected_revision, '2'); assert.equal(b.expected_revision, '4');
  assert.equal(a.command.token, 'private-first-token'); assert.equal(b.command.token, 'private-second-token');
  assert.equal(a.command_id, '00000000000000020000000000000001');
  assert.equal(b.command_id, '00000000000000020000000000000002');
  assert.deepEqual(t.polls, ['0', '1', '2', '3', '4']);
  assert.equal(report.cursor, '5');
  for (const secret of [bootstrap(2).capability, 'private-first-token', 'private-second-token']) assert(!JSON.stringify(report).includes(secret));
});

test('unknown transport and transient HTTP retries retain exact envelope bytes and command ID', async () => {
  const t = transport([page(1, view(0, 'private-token')), page(2, outcome())], (body, attempt) => {
    if (attempt === 1) throw new Error(bootstrap(0).capability);
    if (attempt === 2) return new Response('', { status: 429 });
    if (attempt === 3) return new Response('', { status: 503 });
    return json({ status: 'accepted', command_id: body.command_id });
  });
  const report = await runFoundationsConsumer(bootstrap(0), { fetch: t.fetch, sleep: async () => {} });
  assert.equal(report.status, 'completed');
  assert.equal(report.counts.submission_attempts, 4);
  assert.equal(report.counts.accepted_inputs, 1); assert.equal(report.counts.retries, 3);
  assert.equal(new Set(t.bodies).size, 1);
  assert(!JSON.stringify(report).includes(bootstrap(0).capability));
});

test('stale or unexpected choices fail explicitly without inventing a new command or a rules result', async () => {
  const stale = transport([page(1, view(0, 'private-token'))], body => json({ status: 'rejected', command_id: body.command_id, reason: 'stale_revision' }));
  const a = await runFoundationsConsumer(bootstrap(0), { fetch: stale.fetch });
  assert.equal(a.status, 'stopped'); assert.equal(a.reason, 'stale_revision');
  assert.equal(stale.bodies.length, 1); assert(!('result' in a));
  const unexpected = transport([page(1, view(0, 'private-token', 'bottom_opening', { count: 1, candidates: ['private-card'] }))]);
  const b = await runFoundationsConsumer(bootstrap(0), { fetch: unexpected.fetch });
  assert.equal(b.reason, 'unexpected_request'); assert.equal(unexpected.bodies.length, 0);
  const foreign = transport([page(1, view(1, 'other-player-token'))]);
  const c = await runFoundationsConsumer(bootstrap(0), { fetch: foreign.fetch });
  assert.equal(c.reason, 'foreign_observation'); assert.equal(foreign.bodies.length, 0);
});

test('host pause/fault and bounded resource stops never become Magic outcomes', async () => {
  for (const [status, reason] of [['resource_paused', 'host_paused'], ['faulted', 'host_faulted']]) {
    const t = transport([page(1, lifecycle(status))]);
    const r = await runFoundationsConsumer(bootstrap(0), { fetch: t.fetch });
    assert.equal(r.status, 'stopped'); assert.equal(r.reason, reason); assert(!('result' in r));
  }
  const limited = transport([page(1, view(0, 'first')), page(2, view(0, 'second'))]);
  const r = await runFoundationsConsumer(bootstrap(0), { fetch: limited.fetch, limits: { acceptedInputs: 1 } });
  assert.equal(r.reason, 'input_limit'); assert.equal(limited.bodies.length, 1);
  const retry = transport([], () => { throw new Error('unused'); });
  retry.fetch = async () => new Response('', { status: 503 });
  const exhausted = await runFoundationsConsumer(bootstrap(0), { fetch: retry.fetch, sleep: async () => {}, limits: { attempts: 2 } });
  assert.equal(exhausted.reason, 'retry_limit'); assert(!('result' in exhausted));
  let time = 0;
  const deadline = transport([() => { time = 2; return page(1, outcome()); }]);
  const expired = await runFoundationsConsumer(bootstrap(0), { fetch: deadline.fetch, now: () => time, limits: { durationMs: 1 } });
  assert.equal(expired.reason, 'deadline'); assert(!('result' in expired));
  assert.equal(CONSUMER_LIMITS.acceptedInputs * 4, 20_000);
  const over = await runFoundationsConsumer(bootstrap(0), { limits: { acceptedInputs: 5001 }, fetch: async () => { throw new Error('must not fetch'); } });
  assert.equal(over.reason, 'bootstrap_invalid');
});

test('only explicit typed Outcome completes and public manifest identity is rechecked', async () => {
  const t = transport([page(1, lifecycle('finished')), page(2, outcome())]);
  assert.equal((await runFoundationsConsumer(bootstrap(0), { fetch: t.fetch })).status, 'completed');
  const changed = transport([page(1, outcome())], undefined, index => ({ ...publicManifest(), engine_build_id: (index ? 'd' : 'b').repeat(64) }));
  const r = await runFoundationsConsumer(bootstrap(0), { fetch: changed.fetch });
  assert.equal(r.reason, 'manifest_identity'); assert(!('result' in r));
  const badHash = transport([], undefined, () => ({ ...publicManifest(), catalog_sha256: '0'.repeat(64) }));
  assert.equal((await runFoundationsConsumer(bootstrap(0), { fetch: badHash.fetch })).reason, 'manifest_identity');
});

test('private bootstrap is closed and validation errors never echo bearer material', () => {
  assert.deepEqual(parseBootstrap(bootstrap(3)), bootstrap(3));
  for (const value of [{ ...bootstrap(0), seed: [1] }, { ...bootstrap(0), viewer: 4 }, { ...bootstrap(0), capability: 'secret\n' }, { ...bootstrap(0), matchId: bootstrap(0).capability }]) {
    assert.throws(() => parseBootstrap(value), error => !String(error).includes(bootstrap(0).capability));
  }
});

test('foundations profile pin stays the default and attachment consumers opt in explicitly', async () => {
  const attachments = () => ({ ...publicManifest(), profile: 'commander-attachments-v1' });
  const wrong = transport([], undefined, attachments);
  assert.equal((await runFoundationsConsumer(bootstrap(0), { fetch: wrong.fetch })).reason, 'manifest_identity');
  assert.deepEqual(wrong.polls, []);
  const explicit = transport([page(1, outcome())], undefined, attachments);
  assert.equal((await runFoundationsConsumer(bootstrap(0), {
    fetch: explicit.fetch, expectedProfile: 'commander-attachments-v1',
  })).status, 'completed');
});

test('CLI malformed private stdin emits only a scrubbed failure report', { timeout: 10_000 }, async () => {
  const script = fileURLToPath(new URL('../src/examples/foundations-consumer.mjs', import.meta.url));
  const secret = 'c'.repeat(64);
  for (const input of [`{"capability":"${secret}",`, JSON.stringify({ ...bootstrap(0), seed: 'not-allowed' })]) {
    const result = await new Promise((resolve, reject) => {
      const child = spawn(process.execPath, [script], { stdio: ['pipe', 'pipe', 'pipe'] });
      let stdout = '', stderr = '';
      child.stdout.on('data', chunk => { stdout += chunk; }); child.stderr.on('data', chunk => { stderr += chunk; });
      child.on('error', reject); child.on('exit', code => resolve({ code, stdout, stderr }));
      child.stdin.end(input);
    });
    assert.equal(result.code, 2); assert.equal(result.stderr, '');
    assert(!result.stdout.includes(secret));
    assert.deepEqual(JSON.parse(result.stdout), { consumer_version: 1, status: 'stopped', reason: 'bootstrap_invalid' });
  }
});

test('four real Node processes receive only stdin capabilities and use the actual HTTP SDK', { timeout: 30_000 }, async () => {
  // Synthetic transport fixture only: this proves process/HTTP/privacy wiring,
  // not a rules-defined game or actual Rust hosting acceptance.
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'mtg-consumer-'));
  const secrets = ['c', 'd', 'e', 'f'].map(char => char.repeat(64));
  const submitted = [false, false, false, false];
  const bodies = [], children = [];
  const server = http.createServer(async (request, response) => {
    const seat = secrets.findIndex(secret => request.headers.authorization === `Bearer ${secret}`);
    if (seat < 0) { response.writeHead(401).end(); return; }
    const url = new URL(request.url, 'http://127.0.0.1');
    let result;
    if (url.pathname.endsWith('/manifest')) result = publicManifest();
    else if (url.pathname.endsWith('/events')) {
      const after = url.searchParams.get('after');
      if (after === '0') {
        result = page(1, view(seat, `private-token-seat-${seat}`));
        result.entries[0].publication.observation.view.objects.push({ attack_ready: false, controller: seat, handle: `private-hand-seat-${seat}`, marked_damage: '0', oracle_id: oracle('Plains'), owner: seat, tapped: false, zone: { zone: 'hand', owner: seat } });
      }
      else { assert(submitted[seat]); result = page(2, outcome()); }
    } else {
      let body = ''; for await (const chunk of request) body += chunk;
      const input = JSON.parse(body); bodies.push({ seat, input }); submitted[seat] = true;
      result = { status: 'accepted', command_id: input.command_id };
    }
    response.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify(result));
  });
  try {
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const baseURL = `http://127.0.0.1:${server.address().port}`;
    const script = fileURLToPath(new URL('../src/examples/foundations-consumer.mjs', import.meta.url));
    const reports = await Promise.all(secrets.map((capability, viewer) => new Promise((resolve, reject) => {
      const child = spawn(process.execPath, [script], { stdio: ['pipe', 'pipe', 'pipe'] }); children.push(child);
      assert(!child.spawnargs.includes(capability));
      let stdout = '', stderr = '';
      child.stdout.on('data', chunk => { stdout += chunk; }); child.stderr.on('data', chunk => { stderr += chunk; });
      child.on('error', reject);
      child.on('exit', code => {
        try {
          assert.equal(code, 0, stderr); assert.equal(stderr, '');
          for (const secret of secrets) assert(!stdout.includes(secret));
          assert(!stdout.includes('private-token-seat')); assert(!stdout.includes('private-hand-seat')); assert(!stdout.includes('response'));
          const report = JSON.parse(stdout);
          assert.deepEqual(JSON.parse(fs.readFileSync(path.join(directory, `${viewer}.json`), 'utf8')), report);
          assert.equal(fs.statSync(path.join(directory, `${viewer}.json`)).mode & 0o777, 0o600);
          resolve(report);
        } catch (error) { reject(error); }
      });
      child.stdin.end(JSON.stringify({ baseURL, matchId: 'abc', capability, viewer, outputPath: path.join(directory, `${viewer}.json`) }));
    })));
    assert.equal(bodies.length, 4); assert.equal(new Set(bodies.map(b => b.input.command_id)).size, 4);
    for (const { seat, input } of bodies) {
      assert.equal(input.command.token, `private-token-seat-${seat}`);
      assert.equal(input.expected_revision, '1');
      assert.deepEqual(input.command.response, { response: 'pass' });
    }
    for (const secret of secrets) assert(!JSON.stringify(bodies).includes(secret));
    for (let viewer = 0; viewer < 4; viewer++) {
      assert.equal(reports[viewer].viewer, viewer); assert.equal(reports[viewer].status, 'completed');
      assert.equal(reports[viewer].counts.accepted_inputs, 1);
    }
  } finally {
    for (const child of children) if (child.exitCode === null) child.kill('SIGKILL');
    server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
