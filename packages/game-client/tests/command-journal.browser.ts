import assert from 'node:assert/strict';
import test from 'node:test';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import type { AddressInfo } from 'node:net';
import { chromium, firefox, expect, type Page } from '@playwright/test';
import ts from 'typescript';

// Real IndexedDB, Web Locks, browser refresh and SDK fetch boundary. Synthetic
// command HTTP responses are explicitly NOT native-host/rules evidence. No
// capabilities, full observations, diagnostics, trace or HAR are persisted.
for (const [name, launcher] of Object.entries({ chromium, firefox })) {
  test(`command journal actual ${name} storage/refresh/ownership`, { timeout: 45_000 }, async t => {
    const bodies: string[] = [];
    let loseNextResponse = false;
    const server = createServer(async (request, response) => {
      const path = request.url!;
      if (path === '/') { response.setHeader('content-type', 'text/html'); response.end('<!doctype html><title>Journal harness</title>'); return; }
      if (path === '/v1/matches/a1/commands') {
        let body = ''; for await (const part of request) body += part.toString(); bodies.push(body);
        if (loseNextResponse) {
          loseNextResponse = false; response.writeHead(200, { 'content-type': 'application/json', 'content-length': '1000' });
          response.flushHeaders(); response.write('{"status":'); setTimeout(() => response.destroy(), 20); return;
        }
        response.setHeader('content-type', 'application/json'); response.end(JSON.stringify({ status: 'accepted', command_id: JSON.parse(body).command_id })); return;
      }
      let source: string;
      if (path === '/sdk.js') source = await readFile(new URL('../../game-server/sdk/dist/browser.js', import.meta.url), 'utf8');
      else if (/^\/src\/session\/(?:effects\/)?[a-z-]+\.ts$/.test(path)) {
        source = await readFile(new URL(`../${path.slice(1)}`, import.meta.url), 'utf8');
        source = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2023, module: ts.ModuleKind.ESNext, verbatimModuleSyntax: true } }).outputText;
      } else { response.statusCode = 404; response.end(); return; }
      response.setHeader('content-type', 'text/javascript'); response.end(source);
    });
    await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
    const origin = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
    const browser = await launcher.launch();
    const context = await browser.newContext();
    const setup = async (page: Page) => {
      await page.goto(origin);
      await page.evaluate(`(async () => {
        const { IndexedDBCommandJournal } = await import('/src/session/effects/command-journal.ts');
        const { CommandLifecycle } = await import('/src/session/command.ts');
        const { acquireSeat } = await import('/src/session/effects/ownership.ts');
        const { GameClient, prepareCommand, parsePreparedCommand } = await import('/sdk.js');
        const binding = { origin: location.origin, match: 'a1', seat: '0', engineBuild: '1'.repeat(64), serverBuild: '2'.repeat(64), catalog: '3'.repeat(64), protocol: '4'.repeat(64) };
        const codec = { prepare: prepareCommand, parse: parsePreparedCommand };
        const record = id => ({ version: 1, binding: { ...binding }, ...prepareCommand({ protocol_version: 1, command_id: id ?? 'a'.repeat(32), expected_revision: '1', command: { kind: 'decision', token: 'v0-d0', response: { response: 'mulligan', take: false } } }) });
        globalThis.kit = { IndexedDBCommandJournal, CommandLifecycle, acquireSeat, GameClient, codec, binding, record, journal: name => new IndexedDBCommandJournal(codec, { name }) };
      })()`);
    };
    const page = await context.newPage();
    try {
      console.log(`command journal ${name} ${browser.version()}`);
      await setup(page);
      await t.test('persist-before-send crash, reload and exact whitespace at actual SDK HTTP boundary', async () => {
        const exact = await page.evaluate(`(async () => {
          const record = kit.record(); record.exactBody = ' \\n' + record.exactBody + '  ';
          await kit.journal('crash').persist(record); return record.exactBody;
        })()`);
        assert.equal(bodies.length, 0);
        await page.reload(); await setup(page);
        const recovered = await page.evaluate<{ exactBody: string; phase: string; count: number }>(`(async () => {
          const journal = kit.journal('crash'); const records = await journal.unresolved(kit.binding);
          const sdk = new kit.GameClient({ baseURL: location.origin, matchId: 'a1', capability: 'synthetic-test-only' });
          let snapshot;
          const lifecycle = new kit.CommandLifecycle({ binding: kit.binding, signal: new AbortController().signal, active: () => true, context: () => null,
            publish: value => { snapshot = value; }, refresh() {}, effects: { journal, codec: kit.codec, protocol: kit.binding.protocol, protocolVersion: 1,
              newId() { throw Error('no replacement'); }, transport: { execution: async () => null, submitExact: (body, signal) => sdk.submitExact(body, { signal }) } } });
          await lifecycle.recover(); await lifecycle.reconcile();
          return { exactBody: records[0].exactBody, phase: snapshot.phase, count: (await journal.unresolved(kit.binding)).length };
        })()`);
        assert.equal(recovered.exactBody, exact); assert.equal(bodies[0], exact); assert.equal(recovered.phase, 'unresolved'); assert.equal(recovered.count, 1);
      });
      await t.test('lost committed HTTP response and accepted/pending reload retain journal until authoritative consumption', async () => {
        loseNextResponse = true;
        const result = await page.evaluate(`(async () => {
          const journal = kit.journal('response-loss'), record = kit.record('d'.repeat(32)); await journal.persist(record);
          const sdk = new kit.GameClient({ baseURL: location.origin, matchId: 'a1', capability: 'synthetic-test-only' });
          const failed = await sdk.submitExact(record.exactBody).then(() => false, () => true);
          return { failed, body: record.exactBody };
        })()`);
        assert.equal((result as { failed: boolean }).failed, true);
        assert.equal(bodies.at(-1), (result as { body: string }).body);
        await page.reload(); await setup(page);
        const pending = await page.evaluate(`(async () => {
          const journal = kit.journal('response-loss'); let phase, status = 'pending';
          const id = (await journal.unresolved(kit.binding))[0].envelope.command_id;
          const lifecycle = new kit.CommandLifecycle({ binding: kit.binding, signal: new AbortController().signal, active: () => true, context: () => null,
            publish: value => { phase = value.phase; }, refresh() {}, effects: { journal, codec: kit.codec, protocol: kit.binding.protocol, protocolVersion: 1,
              newId() { throw Error('replacement'); }, transport: { execution: async () => ({ command_id: id, status: { status } }), submitExact() { throw Error('pending must not replay'); } } } });
          await lifecycle.recover(); await lifecycle.reconcile();
          const retained = (await journal.unresolved(kit.binding)).length;
          status = 'consumed_by_rules'; await lifecycle.reconcile();
          return { retained, remaining: (await journal.unresolved(kit.binding)).length, phase };
        })()`);
        assert.deepEqual(pending, { retained: 1, remaining: 0, phase: 'awaiting-refresh' });
      });
      await t.test('atomic one unresolved slot across builds, seats and competing pages', async () => {
        const other = await context.newPage(); await setup(other);
        const outcomes = await Promise.all([
          page.evaluate(`kit.journal('atomic').persist(kit.record()).then(() => 'ok', error => error.kind)`),
          other.evaluate(`kit.journal('atomic').persist({ ...kit.record('b'.repeat(32)), binding: { ...kit.binding, engineBuild: 'f'.repeat(64) } }).then(() => 'ok', error => error.kind)`),
        ]);
        assert.deepEqual(outcomes.sort(), ['conflict', 'ok']);
        const result = await page.evaluate(`(async () => {
          const journal = kit.journal('atomic');
          const records = await journal.unresolved({ ...kit.binding, engineBuild: '9'.repeat(64), catalog: '8'.repeat(64), protocol: '7'.repeat(64) });
          const otherSeat = { ...kit.record(), binding: { ...kit.binding, seat: '1' } };
          await journal.persist(otherSeat);
          return { count: records.length, other: (await journal.unresolved(otherSeat.binding)).length, absent: (await journal.unresolved({ ...kit.binding, match: 'b2' })).length };
        })()`);
        assert.deepEqual(result, { count: 1, other: 1, absent: 0 }); await other.close();
      });
      await t.test('corrupt records remain present and strictly block recovery', async () => {
        const result = await page.evaluate(`(async () => {
          const journal = kit.journal('corrupt'); await journal.persist(kit.record());
          await new Promise((resolve, reject) => {
            const request = indexedDB.open('corrupt', 1); request.onsuccess = () => {
              const db = request.result, tx = db.transaction('commands', 'readwrite');
              tx.objectStore('commands').put({ version: 1, binding: kit.binding, exactBody: kit.record().exactBody, capability: 'forbidden-extra' }, JSON.stringify([kit.binding.origin, 'a1', '0']));
              tx.oncomplete = () => { db.close(); resolve(); }; tx.onerror = reject;
            };
          });
          const kind = await journal.unresolved(kit.binding).then(() => 'unexpected', error => error.kind);
          const again = await journal.unresolved(kit.binding).then(() => 'unexpected', error => error.kind);
          return [kind, again];
        })()`);
        assert.deepEqual(result, ['corrupt', 'corrupt']);
      });
      await t.test('bounded database growth preserves all 64 unresolved slots', async () => {
        const result = await page.evaluate(`(async () => {
          const journal = kit.journal('bounded');
          for (let i = 0; i < 64; i++) await journal.persist({ ...kit.record(), binding: { ...kit.binding, match: (i + 1).toString(16) } });
          const kind = await journal.persist({ ...kit.record(), binding: { ...kit.binding, match: 'ffff' } }).then(() => 'unexpected', error => error.kind);
          const retained = await journal.unresolved({ ...kit.binding, match: '1' });
          return { kind, retained: retained.length };
        })()`);
        assert.deepEqual(result, { kind: 'full', retained: 1 });
      });
      await t.test('Web Locks block second tab and release on close without takeover', async () => {
        const owner = await context.newPage(); await setup(owner);
        const acquire = `kit.acquireSeat({ server_origin: location.origin, match_id: 'a1', viewer: 0 }, new AbortController().signal).then(release => { globalThis.release = release; return !!release; })`;
        assert.equal(await owner.evaluate(acquire), true);
        assert.equal(await page.evaluate(acquire), false);
        await owner.close();
        await expect.poll(async () => page.evaluate(async () => (await navigator.locks.query()).held?.length ?? 0)).toBe(0);
        assert.equal(await page.evaluate(acquire), true);
        await page.evaluate('release()');
      });
      await t.test('wrong record cannot resolve another exact body; terminal resolution permits next slot', async () => {
        const result = await page.evaluate(`(async () => {
          const journal = kit.journal('resolution'), record = kit.record(); await journal.persist(record);
          const conflict = await journal.resolve(kit.record('b'.repeat(32)), 'consumed').then(() => 'unexpected', error => error.kind);
          await journal.resolve(record, 'consumed'); await journal.persist(kit.record('c'.repeat(32)));
          return { conflict, id: (await journal.unresolved(kit.binding))[0].envelope.command_id };
        })()`);
        assert.deepEqual(result, { conflict: 'conflict', id: 'c'.repeat(32) });
      });
    } finally {
      await context.close(); await browser.close(); server.closeAllConnections(); await new Promise<void>(resolve => server.close(() => resolve()));
    }
  });
}
