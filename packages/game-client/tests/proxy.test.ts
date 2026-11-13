import assert from 'node:assert/strict';
import test from 'node:test';
import { upstreamOrigin, localRequestAllowed, readRouteAllowed } from '../scripts/proxy-policy.ts';

test('proxy destination is a canonical explicit numeric loopback, including port 80', () => {
  for (const value of ['http://127.0.0.1:80', 'http://127.0.0.1:18741', 'http://[::1]:65535']) assert.equal(upstreamOrigin(value), value);
  for (const value of ['http://localhost:18741', 'http://127.0.0.2:10', 'http://127.0.0.1:0', 'http://127.0.0.1:65536', 'http://127.0.0.1:080', 'http://127.0.0.1:1/', 'http://127.0.0.1:1\n', 'http://evil.test', 'http://u@127.0.0.1:2']) assert.throws(() => upstreamOrigin(value));
});
test('local Host/Origin/Fetch Metadata are checked before upstream rewriting', () => {
  assert.ok(localRequestAllowed('127.0.0.1:4179', undefined, 'same-origin', 4179));
  assert.ok(localRequestAllowed('127.0.0.1:4179', 'http://127.0.0.1:4179', 'none', 4179));
  for (const host of ['localhost:4179', 'evil.test:4179', '127.0.0.1:9999', undefined]) assert.equal(localRequestAllowed(host, undefined, undefined, 4179), false);
  for (const origin of ['null', 'http://evil.test', 'http://127.0.0.1:5173']) assert.equal(localRequestAllowed('127.0.0.1:4179', origin, undefined, 4179), false);
  assert.equal(localRequestAllowed('127.0.0.1:4179', undefined, 'cross-site', 4179), false);
});
test('proxy forwards only exact read routes, never commands, private files or arbitrary paths', () => {
  for (const path of ['/v1/matches/a1/manifest', '/v1/matches/a1/resume', '/v1/matches/a1/presentation', '/v1/matches/a1/events?after=1&limit=32&wait_ms=20000', `/presentation/${'a'.repeat(64)}.json`]) assert.ok(readRouteAllowed('GET', path), path);
  for (const path of ['/v1/matches/a1/commands', '/v1/matches/a1/resume?viewer=1', '/v1/matches/a1/%72esume', '/presentation/../secret', '/generated/seats.json', '/v1/matches/a1/manifest#x']) assert.equal(readRouteAllowed('GET', path), false, path);
  assert.equal(readRouteAllowed('POST', '/v1/matches/a1/resume'), false);
});
