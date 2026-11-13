import assert from 'node:assert/strict';
import test from 'node:test';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { readFileSync } from 'node:fs';
import opening from '../fixtures/foundations-opening/opening-seat0-http.json' with { type: 'json' };
import { browserReadSession } from '../src/session/effects/browser.ts';
import type { ReadSnapshot } from '../src/session/controller.ts';

// Synthetic HTTP envelopes around source fixtures; this is transport/session
// evidence, not engine admission or native-host qualification. The real browser
// adapter and built SDK perform all reads, decoding, classification and retries.
const catalog = JSON.parse(readFileSync(new URL('../../game-engine/tests/fixtures/expected-match-ir.json', import.meta.url), 'utf8'));
const deck = { commander: catalog.definitions[0].oracle_id, cards: [{ oracle_id: catalog.definitions[0].oracle_id, count: 99 }] };
for (const mode of ['recover', 'exhaust', 'malformed', 'schema'] as const) {
  test(`real adapter body recovery: ${mode}`, { timeout: 20_000 }, async () => {
    let origin = '', polls = 0, resumes = 0, ready = 0, released = 0;
    let onRelease!: () => void;
    const lockReleased = new Promise<void>(resolve => { onRelease = resolve; });
    const snapshots: ReadSnapshot[] = [];
    const methods: string[] = [];
    const identity = { protocol_version: 1, match_id: opening.resume.match_id,
      engine_build_id: opening.resume.engine_build_id, server_build_id: opening.resume.server_build_id,
      catalog_sha256: opening.resume.catalog_sha256 };
    const server = createServer((request, response) => {
      methods.push(request.method!);
      response.setHeader('content-type', 'application/json');
      const json = (value: unknown) => response.end(JSON.stringify(value));
      if (request.url === '/client-config.json') { json({ client_config_version: 1, server_origin: origin }); return; }
      if (request.url?.endsWith('/manifest')) { json({ ...identity, profile: opening.profile, decks: [deck, deck, deck, deck], starting_player: 0, catalog }); return; }
      if (request.url?.endsWith('/presentation')) { response.statusCode = 404; json({}); return; }
      if (request.url?.endsWith('/resume')) { resumes++; json(opening.resume); return; }
      if (request.url?.includes('/events?')) {
        polls++;
        if (mode === 'malformed') { response.end('{"protocol_version":'); return; }
        if (mode === 'schema') { json({ protocol_version: 1, unexpected: true }); return; }
        // Valid headers plus partial bytes reach fetch before the socket drops.
        response.setHeader('content-length', '1000');
        response.flushHeaders(); response.write('{"protocol_version":1,');
        setTimeout(() => response.destroy(), 20);
        return;
      }
      response.statusCode = 404; json({});
    });
    await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
    origin = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
    const previousWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
    const previousLocks = Object.getOwnPropertyDescriptor(navigator, 'locks');
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { location: { origin } } });
    Object.defineProperty(navigator, 'locks', { configurable: true, value: {
      async request(name: string, _options: unknown, callback: (lock: { name: string }) => Promise<void>) {
        try { await callback({ name }); } finally { released++; onRelease(); }
      },
    } });
    const session = browserReadSession();
    session.subscribe(() => {
      const snapshot = session.getSnapshot(); snapshots.push(snapshot);
      if (snapshot.connection === 'ready' && ++ready === 2 && mode === 'recover') queueMicrotask(session.stop);
    });
    try {
      await session.join(new File([JSON.stringify({ ...identity, bootstrap_version: 1, server_origin: origin, viewer: 0, capability: 'a'.repeat(64) })], 'synthetic-seat.json'));
      assert.ok(snapshots.some(snapshot => snapshot.connection === 'ready' && snapshot.read));
      const retries = snapshots.filter(snapshot => snapshot.connection === 'retrying');
      if (mode === 'recover' || mode === 'exhaust') {
        assert.deepEqual(retries.map(snapshot => snapshot.retry), mode === 'recover' ? [1] : [1, 2, 3, 4]);
        for (const snapshot of retries) {
          assert.equal(snapshot.failure, 'disconnected');
          assert.deepEqual(snapshot.read, opening.resume, 'interrupted body must preserve the permitted view');
        }
        assert.equal(polls, mode === 'recover' ? 1 : 5, 'retry budget is not renewed by a successful recovery resume');
        assert.equal(resumes, mode === 'recover' ? 2 : 5);
        if (mode === 'recover') assert.equal(ready, 2, 'recovery resumes from a validated committed boundary');
        else {
          assert.equal(session.getSnapshot().connection, 'disconnected');
          assert.deepEqual(session.getSnapshot().read, opening.resume);
        }
      } else {
        assert.equal(session.getSnapshot().connection, 'incompatible');
        assert.equal(session.getSnapshot().read, null);
        assert.equal(retries.length, 0); assert.equal(polls, 1); assert.equal(resumes, 1);
      }
      await lockReleased;
      assert.equal(released, 1);
      assert.ok(methods.every(method => method === 'GET'));
    } finally {
      session.stop();
      if (previousWindow) Object.defineProperty(globalThis, 'window', previousWindow); else Reflect.deleteProperty(globalThis, 'window');
      if (previousLocks) Object.defineProperty(navigator, 'locks', previousLocks); else Reflect.deleteProperty(navigator, 'locks');
      server.closeAllConnections();
      await new Promise<void>(resolve => server.close(() => resolve()));
    }
  });
}
