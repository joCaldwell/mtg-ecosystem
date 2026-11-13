import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import { chromium, firefox } from 'playwright';

const sdk = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const build = JSON.parse(fs.readFileSync(path.join(sdk, 'dist/build.json')));
for (const file of build.files) assert.equal(sha(fs.readFileSync(path.join(sdk, 'dist', file.path))), file.sha256, 'build hash');
const csp = "default-src 'none'; script-src 'self'; connect-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'";
const files = new Map([
  ['/', ['browser/index.html', 'text/html; charset=utf-8']],
  ['/proof.mjs', ['browser/proof.mjs', 'text/javascript; charset=utf-8']],
  ['/browser.js', ['dist/browser.js', 'text/javascript; charset=utf-8']],
]);
let origin;
let requests = 0;
let pendingResponses = 0;
const server = createServer((req, res) => {
  res.setHeader('Content-Security-Policy', csp);
  res.setHeader('Cache-Control', 'no-store');
  res.setHeader('X-Content-Type-Options', 'nosniff');
  if (req.headers.host !== new URL(origin).host || (req.headers.origin && req.headers.origin !== origin)) {
    res.writeHead(403).end(); return;
  }
  const url = new URL(req.url, origin);
  if (req.method === 'GET' && files.has(url.pathname) && !url.search) {
    const [file, type] = files.get(url.pathname);
    res.setHeader('Content-Type', type); res.end(fs.readFileSync(path.join(sdk, file))); return;
  }
  if (req.method === 'GET' && url.pathname === '/v1/matches/a/events' && req.headers.authorization === 'Bearer synthetic-browser-test-capability') {
    requests++;
    if (url.searchParams.get('wait_ms') === '20000') {
      pendingResponses++; // Browser abort must close this connection; no timer/retry loop.
      res.on('close', () => { pendingResponses--; }); return;
    }
    res.setHeader('Content-Type', 'application/json');
    res.end(JSON.stringify({ protocol_version: 1, entries: [], next: url.searchParams.get('after'), has_more: false })); return;
  }
  res.writeHead(404).end();
});
server.listen(0, '127.0.0.1'); await once(server, 'listening');
origin = `http://127.0.0.1:${server.address().port}`;
const results = [];
try {
  for (const [name, type] of [['chromium', chromium], ['firefox', firefox]]) {
    const browser = await type.launch({ headless: true });
    try {
      const context = await browser.newContext({ serviceWorkers: 'block' });
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', () => errors.push('uncaught browser error'));
      await page.goto(origin);
      await page.locator('#result[data-status="passed"], #result[data-status="failed"]').waitFor({ timeout: 30_000 });
      const result = JSON.parse(await page.locator('#result').textContent());
      assert.equal(result.status, 'passed', JSON.stringify(result));
      assert.deepEqual(errors, []);
      results.push({ browser: name, version: browser.version(), ...result });
      await context.close();
    } finally { await browser.close(); }
  }
  assert.equal(requests, 6, 'two polls and one aborted poll per browser; no retries');
  assert.equal(pendingResponses, 0, 'aborted HTTP responses closed');
  const sourcePaths = ['browser/index.html', 'browser/proof.mjs', 'scripts/test-browser.mjs', 'package-lock.json'];
  const report = { format_version: 1, status: 'browser-fixture-proof-passed',
    scope: 'production ESM/CSP/fixture transport; not real Rust hosting, recovery, legality or human play',
    node: process.version, playwright: '1.65.0', schema_sha256: build.schema_sha256,
    build_sha256: sha(fs.readFileSync(path.join(sdk, 'dist/build.json'))), csp,
    sources: sourcePaths.map(file => ({ path: file, sha256: sha(fs.readFileSync(path.join(sdk, file))) })),
    http_requests: requests, results,
  };
  const output = path.join(sdk, 'generated', `browser-proof-${Date.now()}.json`);
  fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ status: report.status, report: output, browsers: results.map(r => ({ browser: r.browser, version: r.version, checks: r.checks.length })) }));
} finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
