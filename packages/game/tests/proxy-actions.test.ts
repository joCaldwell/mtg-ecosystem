import assert from 'node:assert/strict';
import test from 'node:test';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { spawn } from 'node:child_process';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { once } from 'node:events';
import { GameClient } from '@multiverse/game-server-sdk/browser';

// Real production Vite proxy + qualified SDK, synthetic HTTP upstream. No Rust
// host admission claim. Temporary failure files remain; credentials are synthetic.
test('production proxy forwards exact cancellation and rejects the removed discovery route after local boundary checks', { timeout: 15_000 }, async () => {
  const seen: string[] = [];
  const upstream = createServer(async (request, response) => {
    seen.push(`${request.method} ${request.url}`);
    assert.equal(request.headers.cookie, undefined); assert.equal(request.headers.referer, undefined);
    assert.equal(request.headers.authorization === `Bearer ${'a'.repeat(64)}`, true);
    assert.equal(request.headers.origin, `http://127.0.0.1:${(upstream.address() as AddressInfo).port}`);
    response.setHeader('content-type', 'application/json');
    response.end(JSON.stringify({ status: 'accepted', command_id: 'b'.repeat(32) }));
  });
  await new Promise<void>(resolve => upstream.listen(0, '127.0.0.1', resolve));
  const probe = createServer(); await new Promise<void>(resolve => probe.listen(0, '127.0.0.1', resolve));
  const port = (probe.address() as AddressInfo).port; await new Promise<void>(resolve => probe.close(() => resolve()));
  const output = await mkdtemp(join(tmpdir(), 'mtg-client-proxy-actions-'));
  await writeFile(join(output, 'index.html'), '<!doctype html><title>Proxy fixture</title>');
  const process = spawn(globalThis.process.execPath, ['node_modules/vite/bin/vite.js', 'preview', '--port', String(port), '--outDir', output], {
    cwd: fileURLToPath(new URL('../', import.meta.url)), stdio: ['ignore', 'pipe', 'pipe'],
    env: { ...globalThis.process.env, MTG_CLIENT_UPSTREAM: `http://127.0.0.1:${(upstream.address() as AddressInfo).port}` },
  });
  const logs: Buffer[] = [];
  process.stdout.on('data', chunk => logs.push(Buffer.from(chunk))); process.stderr.on('data', chunk => logs.push(Buffer.from(chunk)));
  try {
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('proxy startup deadline')), 5_000);
      process.stdout.on('data', () => { if (Buffer.concat(logs).toString().includes('Local:')) { clearTimeout(timer); resolve(); } });
      process.once('exit', () => { clearTimeout(timer); reject(new Error('proxy exited before ready')); });
    });
    const origin = `http://127.0.0.1:${port}`;
    const sdk = new GameClient({ baseURL: origin, matchId: 'a1', capability: 'a'.repeat(64) });
    assert.equal((await sdk.cancel('b'.repeat(32))).status, 'accepted');
    const count = seen.length;
    for (const [method, path] of [['POST', '/v1/matches/a1/actions/query'], ['GET', '/v1/matches/a1/actions/query'], ['POST', '/v1/matches/a1/actions/query?viewer=0'], ['POST', `/v1/matches/a1/commands/${'b'.repeat(32)}/cancel/`], ['POST', '/v1/operator']] as const) {
      assert.equal((await fetch(origin + path, { method })).status, 404);
    }
    assert.equal((await fetch(`${origin}/v1/matches/a1/commands`, { method: 'POST', headers: { origin: 'https://untrusted.example' } })).status, 403);
    assert.equal(seen.length, count); assert.equal(count, 1);
    assert.deepEqual(seen, [`POST /v1/matches/a1/commands/${'b'.repeat(32)}/cancel`]);
  } finally {
    if (process.exitCode === null && process.signalCode === null) { process.kill('SIGTERM'); await once(process, 'exit'); }
    await writeFile(join(output, 'proxy.log'), Buffer.concat(logs));
    upstream.closeAllConnections(); await new Promise<void>(resolve => upstream.close(() => resolve()));
  }
});
