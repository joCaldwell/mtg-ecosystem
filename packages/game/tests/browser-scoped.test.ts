import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import opening from './current-opening.ts';
import { browserCommandSession } from '../web/src/session/effects/browser.ts';

// Synthetic transport over the retained permitted fixture; no native acceptance
// claim. Exercise the production adapter and SDK, including the SDK's own fetch
// options, rather than calling a cookie wrapper in isolation.
const catalog = JSON.parse(readFileSync(new URL('../../game-engine/tests/fixtures/expected-match-ir.json', import.meta.url), 'utf8'));
const deck = { commander: catalog.definitions[0].oracle_id, cards: [{ oracle_id: catalog.definitions[0].oracle_id, count: 99 }] };
const identity = { protocol_version: 2, match_id: opening.resume.match_id,
  engine_build_id: opening.resume.engine_build_id, server_build_id: opening.resume.server_build_id,
  catalog_sha256: opening.resume.catalog_sha256 };
const origin = 'http://127.0.0.1:18741';
function file(match_id = identity.match_id, viewer = 0) {
  return new File([JSON.stringify({ ...identity, match_id, viewer, bootstrap_version: 1, server_origin: origin, capability: 'a'.repeat(64) })], 'synthetic-seat.json');
}
function install(fetcher: typeof fetch) {
  const oldWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const oldLocks = Object.getOwnPropertyDescriptor(navigator, 'locks');
  const oldFetch = globalThis.fetch;
  Object.defineProperty(globalThis, 'window', { configurable: true, value: { location: { origin } } });
  Object.defineProperty(navigator, 'locks', { configurable: true, value: {
    async request(name: string, _options: unknown, callback: (lock: { name: string }) => Promise<void>) { await callback({ name }); },
  } });
  globalThis.fetch = fetcher;
  return () => {
    globalThis.fetch = oldFetch;
    if (oldWindow) Object.defineProperty(globalThis, 'window', oldWindow); else Reflect.deleteProperty(globalThis, 'window');
    if (oldLocks) Object.defineProperty(navigator, 'locks', oldLocks); else Reflect.deleteProperty(navigator, 'locks');
  };
}

test('account-scoped config and SDK reads use host transport if supplied; standalone omits cookies', async () => {
  for (const mode of ['hosted', 'hosted-injected', 'standalone'] as const) {
    const scoped = mode !== 'standalone', injected = mode === 'hosted-injected';
    const prefix = scoped ? `/api/games/${identity.match_id}/native` : '';
    const seen: string[] = [];
    let session: ReturnType<typeof browserCommandSession>;
    const requestFetch: typeof fetch = async (input, init) => {
      const url = new URL(String(input));
      assert.equal(url.origin, origin);
      assert.ok(url.pathname.startsWith(`${prefix}/`));
      seen.push(url.pathname);
      assert.equal(init?.credentials, scoped ? 'same-origin' : 'omit');
      const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status, headers: { 'content-type': 'application/json' } });
      if (url.pathname.endsWith('/client-config.json')) {
        assert.equal(new Headers(init?.headers).get('Authorization'), null);
        return json({ client_config_version: 1, server_origin: origin });
      }
      assert.equal(new Headers(init?.headers).get('Authorization'), `Bearer ${'a'.repeat(64)}`);
      if (url.pathname.endsWith('/manifest')) return json({ ...identity, profile: opening.profile, decks: [deck, deck, deck, deck], starting_player: 0, catalog });
      if (url.pathname.endsWith('/presentation')) return json({}, 404);
      if (url.pathname.endsWith('/resume')) return json(opening.resume);
      if (url.pathname.endsWith('/events')) {
        queueMicrotask(() => session.stop());
        return json({ protocol_version: 2, next: opening.resume.next, entries: [], has_more: false });
      }
      throw new Error('Unexpected SDK route');
    };
    const restore = install(injected ? async () => assert.fail('Hosted request bypassed its transport') : requestFetch);
    session = browserCommandSession('4'.repeat(64), scoped ? { endpoint: prefix, ...(injected ? { requestFetch } : {}) } : undefined);
    try {
      await session.join(file());
      assert.deepEqual(seen.map(path => path.slice(prefix.length)), ['/client-config.json', `/v1/matches/${identity.match_id}/manifest`, `/v1/matches/${identity.match_id}/presentation`, `/v1/matches/${identity.match_id}/resume`, `/v1/matches/${identity.match_id}/events`]);
    } finally { session.stop(); restore(); }
  }
});

test('scoped bootstrap must bind the app-selected match and human seat before any request', async () => {
  let requests = 0;
  const restore = install(async () => { requests++; throw new Error('Unexpected request'); });
  try {
    for (const imported of [file('f'.repeat(32)), file(identity.match_id, 1)]) {
      const session = browserCommandSession('4'.repeat(64), { endpoint: `/api/games/${identity.match_id}/native` });
      await session.join(imported);
      assert.equal(session.getSnapshot().connection, 'invalid-file');
      session.stop();
    }
    assert.equal(requests, 0);
  } finally { restore(); }
});

test('actual SDK read 429 supplies an authorization-local Retry-After cooldown without changing credentials', async () => {
  for (const scoped of [true, false]) {
    const prefix = scoped ? `/api/games/${identity.match_id}/native` : '';
    const delays: number[] = [], seen: string[] = [];
    const originalTimeout = globalThis.setTimeout;
    let polls = 0, session: ReturnType<typeof browserCommandSession>;
    // Record the real adapter/controller delay, accelerating only this isolated
    // unit-test timer. Controller tests separately hold the wait unresolved.
    globalThis.setTimeout = ((callback: (...args: any[]) => void, ms?: number, ...args: any[]) => {
      if (ms && ms > 30_000) delays.push(ms);
      return originalTimeout(callback, ms && ms > 30_000 ? 0 : ms, ...args);
    }) as typeof setTimeout;
    const restore = install(async (input, init) => {
      const url = new URL(String(input)); seen.push(url.pathname);
      assert.equal(init?.credentials, scoped ? 'same-origin' : 'omit');
      const json = (value: unknown, status = 200, headers = {}) => new Response(JSON.stringify(value), { status, headers: { 'content-type': 'application/json', ...headers } });
      if (url.pathname.endsWith('/client-config.json')) return json({ client_config_version: 1, server_origin: origin });
      if (url.pathname.endsWith('/manifest')) return json({ ...identity, profile: opening.profile, decks: [deck, deck, deck, deck], starting_player: 0, catalog });
      if (url.pathname.endsWith('/presentation')) return json({}, 404);
      if (url.pathname.endsWith('/resume')) return json(opening.resume);
      if (url.pathname.endsWith('/events')) {
        if (++polls === 1) return json({}, 429, { 'Retry-After': '35' });
        queueMicrotask(() => session.stop());
        return json({ protocol_version: 2, next: opening.resume.next, entries: [], has_more: false });
      }
      throw new Error('Unexpected SDK route');
    });
    session = browserCommandSession('4'.repeat(64), scoped ? { endpoint: prefix } : undefined);
    try {
      await session.join(file());
      assert.equal(delays.length, 1);
      assert.ok(delays[0]! > 34_000 && delays[0]! <= 35_000);
      assert.equal(seen.filter(path => path.endsWith('/manifest')).length, 2);
      assert.equal(seen.filter(path => path.endsWith('/resume')).length, 2);
    } finally { session.stop(); restore(); globalThis.setTimeout = originalTimeout; }
  }
});
