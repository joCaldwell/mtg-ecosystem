import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import { chromium, firefox } from 'playwright';
import { parseSeatBootstrap } from '../dist/browser.js';
import { ROUTES } from '../generated/routes.ts';

const sdk = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const nativeCommit = process.env.SDK_NATIVE_COMMIT;
assert(/^[0-9a-f]{40}$/.test(nativeCommit ?? ''), 'Set SDK_NATIVE_COMMIT to the frozen native source');
const nativeInfoBytes = fs.readFileSync(process.env.SDK_NATIVE_INFO);
const nativeInfo = JSON.parse(nativeInfoBytes);
const binaryHash = sha(fs.readFileSync(process.env.SDK_NATIVE_BIN));
const upstream = new URL(process.env.SDK_BROWSER_HOST_ORIGIN);
assert(upstream.protocol === 'http:' && ['127.0.0.1', '[::1]'].includes(upstream.hostname) && upstream.href === `${upstream.origin}/`, 'Explicit numeric loopback upstream required');
const seats = process.argv.slice(2).map(file => {
  const stat = fs.statSync(file);
  assert(stat.isFile() && stat.size <= 4096 && (stat.mode & 0o077) === 0, 'Use a bounded private single-seat file');
  const value = parseSeatBootstrap(fs.readFileSync(file, 'utf8'));
  assert.equal(new URL(value.server_origin).origin, upstream.origin, 'Bootstrap upstream mismatch');
  assert.equal(value.engine_build_id, nativeInfo.engine_build_id, 'Engine build mismatch');
  assert.equal(value.server_build_id, nativeInfo.server_build_id, 'Server build mismatch');
  return { file, viewer: value.viewer, match: value.match_id };
});
assert(seats.length > 0 && seats.length <= 4, 'Pass one to four single-seat file paths');
const buildBytes = fs.readFileSync(path.join(sdk, 'dist/build.json'));
const build = JSON.parse(buildBytes);
for (const file of build.files) assert.equal(sha(fs.readFileSync(path.join(sdk, 'dist', file.path))), file.sha256, 'Build artifact mismatch');
const csp = "default-src 'none'; script-src 'self'; connect-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'";
const files = new Map([['/', ['browser/live.html', 'text/html']], ['/live.mjs', ['browser/live.mjs', 'text/javascript']], ['/browser.js', ['dist/browser.js', 'text/javascript']]]);
const permittedPaths = new Set(seats.flatMap(seat => ['get_resume', 'get_manifest', 'poll_events'].map(name => ROUTES[name].path.replace('{match_id}', seat.match))));
const proxyStatuses = [];
let origin;
const server = createServer(async (req, res) => {
  res.setHeader('Content-Security-Policy', csp); res.setHeader('Cache-Control', 'no-store'); res.setHeader('X-Content-Type-Options', 'nosniff');
  if (req.headers.host !== new URL(origin).host || (req.headers.origin && req.headers.origin !== origin) || req.method !== 'GET') { res.writeHead(403).end(); return; }
  const url = new URL(req.url, origin);
  if (files.has(url.pathname) && !url.search) {
    const [file, type] = files.get(url.pathname); res.setHeader('Content-Type', `${type}; charset=utf-8`); res.end(fs.readFileSync(path.join(sdk, file))); return;
  }
  if (url.pathname === '/configuration.json' && !url.search) { res.setHeader('Content-Type', 'application/json'); res.end(JSON.stringify({ upstream: upstream.origin })); return; }
  if (!permittedPaths.has(url.pathname) || !/^Bearer [0-9a-f]{64}$/.test(req.headers.authorization ?? '')) { res.writeHead(403).end(); return; }
  const controller = new AbortController();
  res.on('close', () => controller.abort());
  try {
    // This is an explicitly configured test proxy, not a general URL proxy or
    // production serving/auth policy. The bootstrap cannot choose a destination.
    const response = await fetch(`${upstream.origin}${url.pathname}${url.search}`, {
      headers: { Authorization: req.headers.authorization, Accept: 'application/json' },
      redirect: 'error', credentials: 'omit', cache: 'no-store', signal: controller.signal,
    });
    assert.equal(response.headers.get('cache-control'), 'no-store', 'Native API cache policy');
    proxyStatuses.push(response.status);
    res.writeHead(response.status, { 'Content-Type': response.headers.get('content-type') ?? 'application/octet-stream' });
    let bytes = 0;
    for await (const chunk of response.body) {
      bytes += chunk.byteLength;
      if (bytes > 8 * 1024 * 1024) throw new Error('Proxy response bound');
      if (!res.write(chunk)) await once(res, 'drain', { signal: controller.signal });
    }
    res.end();
  } catch { if (res.headersSent) res.destroy(); else res.writeHead(502).end(); }
});
server.listen(0, '127.0.0.1'); await once(server, 'listening');
origin = `http://127.0.0.1:${server.address().port}`;
const results = [];
try {
  for (const [name, type] of [['chromium', chromium], ['firefox', firefox]]) {
    const browser = await type.launch({ headless: true });
    try {
      for (const seat of seats) {
        const context = await browser.newContext({ serviceWorkers: 'block' });
        const page = await context.newPage();
        let browserError = false;
        page.on('pageerror', () => { browserError = true; });
        await page.goto(origin);
        await page.locator('#bootstrap').setInputFiles(seat.file);
        await page.locator('#result[data-status="passed"], #result[data-status="failed"]').waitFor({ timeout: 30_000 });
        assert.equal(await page.locator('#result').getAttribute('data-status'), 'passed', 'Real browser host proof failed (details deliberately redacted)');
        const result = JSON.parse(await page.locator('#result').textContent());
        assert.equal(result.viewer, seat.viewer); assert.equal(browserError, false);
        results.push({ browser: name, version: browser.version(), ...result });
        await context.close();
      }
    } finally { await browser.close(); }
  }
  assert.equal(proxyStatuses.length, seats.length * 2 * 4, 'Four explicit reads per seat/browser; no retries');
  assert(proxyStatuses.every(status => status === 200));
  const sources = ['browser/live.html', 'browser/live.mjs', 'scripts/test-browser-host.mjs', 'package-lock.json'].map(file => ({ path: file, sha256: sha(fs.readFileSync(path.join(sdk, file))) }));
  const report = { format_version: 1, status: 'real-browser-host-read-proof-passed',
    scope: 'authenticated manifest/resume/stream reads only; no gameplay submissions, crash recovery, semantic queries or natural game',
    native_commit: nativeCommit, native_binary_sha256: binaryHash, native_info_sha256: sha(nativeInfoBytes),
    engine_build_id: nativeInfo.engine_build_id, server_build_id: nativeInfo.server_build_id,
    schema_sha256: build.schema_sha256, build_sha256: sha(buildBytes), node: process.version, playwright: '1.65.0',
    csp, sources, requests: proxyStatuses.length, results };
  const output = path.join(sdk, 'generated', `browser-host-proof-${Date.now()}.json`);
  fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  console.log(JSON.stringify({ status: report.status, report: output, contexts: results.length, requests: report.requests }));
} finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
