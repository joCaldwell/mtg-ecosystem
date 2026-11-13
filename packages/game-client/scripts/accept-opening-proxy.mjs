// Real restricted Vite preview -> native host proof, using the production App.
// Explicit owned ports/paths only; no Cargo, trace, HAR, deployment or other host.
import fs from 'node:fs';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { request } from 'node:http';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import { chromium, firefox, expect } from '@playwright/test';
const [binary, provisioner, bundle, presentation, output, nativePortText, proxyPortText] = process.argv.slice(2);
assert(output && proxyPortText && !fs.existsSync(output), 'Supply retained binary/provisioner/bundle/presentation, NEW output, explicit native and proxy ports');
for (const p of [binary, provisioner, bundle, presentation, output]) assert(path.isAbsolute(p));
const ports = [nativePortText, proxyPortText].map(Number);
assert(ports.every(p => Number.isInteger(p) && p > 1024 && p < 65536) && ports[0] !== ports[1]);
const nativeOrigin = `http://127.0.0.1:${ports[0]}`, proxyOrigin = `http://127.0.0.1:${ports[1]}`;
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
fs.mkdirSync(output, { mode: 0o700 });
const privateDir = path.join(output, 'private'), registry = path.join(output, 'registry');
assert(spawnSync(provisioner, [registry, privateDir, nativeOrigin], { encoding: 'utf8' }).status === 0, 'Provisioning failed; private output omitted');
const bundleHash = sha(fs.readFileSync(path.join(bundle, 'client-assets.json'))), presentationHash = sha(fs.readFileSync(path.join(presentation, 'artifacts.json')));
const hostLog = fs.openSync(path.join(output, 'host.log'), 'wx', 0o600), proxyLog = fs.openSync(path.join(output, 'proxy.log'), 'wx', 0o600);
const host = spawn(binary, ['serve-client', registry, `127.0.0.1:${ports[0]}`, bundle, bundleHash, presentation, presentationHash], { stdio: ['ignore', hostLog, hostLog] });
const proxy = spawn(process.execPath, ['node_modules/vite/bin/vite.js', 'preview', '--port', proxyPortText], { cwd: path.resolve('packages/game-client'), env: { ...process.env, MTG_CLIENT_UPSTREAM: nativeOrigin }, stdio: ['ignore', proxyLog, proxyLog] });
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
async function stop(child) { if (child.exitCode !== null) return; child.kill('SIGINT'); const timeout = setTimeout(() => child.kill('SIGKILL'), 8000); try { await once(child, 'exit'); } finally { clearTimeout(timeout); } }
async function raw(origin, target, method = 'GET', headers = {}, body = '') {
  return new Promise((resolve, reject) => { const req = request(origin + target, { method, headers }, res => { res.resume(); res.on('end', () => resolve({ status: res.statusCode, cors: res.headers['access-control-allow-origin'] })); }); req.on('error', reject); req.end(body); });
}
const checks = [], browsers = [];
try {
  for (const origin of [nativeOrigin, proxyOrigin]) for (let n = 0;; n++) { try { if ((await raw(origin, '/join')).status === 200) break; } catch {} assert(n < 300 && host.exitCode === null && proxy.exitCode === null, 'Owned host readiness failed'); await pause(50); }
  const id = 'a'.repeat(32);
  for (const [target, method, headers, expected] of [
    ['/join', 'GET', { host: 'evil.test' }, 403], ['/join', 'GET', { origin: 'http://evil.test' }, 403], ['/join', 'GET', { 'sec-fetch-site': 'cross-site' }, 403],
    ['/v1/matches/a1/commands', 'POST', { origin: 'http://evil.test' }, 403],
    ['/v1/matches/a1/commands', 'POST', {}, 401], [`/v1/matches/a1/commands/${id}`, 'GET', {}, 401],
    ['/v1/matches/a1/commands', 'GET', {}, 404], [`/v1/matches/a1/commands/${id}`, 'POST', {}, 404],
    [`/v1/matches/a1/commands/${id}/cancel`, 'POST', {}, 404], ['/v1/matches/a1/actions/query', 'POST', {}, 404],
    ['/v1/matches/a1/commands?viewer=0', 'POST', {}, 404], [`/v1/matches/a1/commands/${id}?viewer=0`, 'GET', {}, 404],
    ['/v1/matches/a1/%63ommands', 'POST', {}, 404], ['/v1/matches/a1/resume?viewer=0', 'GET', {}, 404],
    ['/generated/private.json', 'GET', {}, 404], ['/private/seat-0.json', 'GET', {}, 404], ['/client-assets.json', 'GET', {}, 404],
  ]) { const r = await raw(proxyOrigin, target, method, headers); assert(r.status === expected && r.cors === undefined, `Restricted proxy failed: ${method} ${target}`); checks.push({ target, method, status: r.status }); }
  const seat0 = JSON.parse(fs.readFileSync(path.join(privateDir, 'seat-0.json')));
  for (const origin of [nativeOrigin, proxyOrigin]) {
    const auth = { authorization: `Bearer ${seat0.capability}`, 'content-type': 'application/json' };
    assert((await raw(origin, '/v1/matches/a1/commands', 'POST', auth, 'x'.repeat(65537))).status === 413, 'Native authenticated command body bound failed');
    assert((await raw(origin, '/v1/matches/a1/commands', 'POST', auth, '{}')).status === 400, 'Native authenticated command schema bound failed');
  }
  const auth = { authorization: `Bearer ${seat0.capability}`, 'content-type': 'application/json' };
  for (const [target, method, body, status] of [
    [`/v1/matches/a1/commands/${id}/cancel?x=1`, 'POST', '', 400],
    [`/v1/matches/a1/commands/${id}/cancel`, 'POST', '{}', 400],
    ['/v1/matches/a1/commands/invalid/cancel', 'POST', '', 400],
    ['/v1/matches/a1/commands/invalid', 'GET', '', 400],
    [`/v1/matches/a1/commands/${id}`, 'GET', '', 404],
  ]) { const result = await raw(nativeOrigin, target, method, auth, body); assert.equal(result.status, status, `Native execution/cancel restriction: ${method} ${target}`); }
  for (const [index, launcher] of [chromium, firefox].entries()) {
    const browser = await launcher.launch();
    try {
      const context = await browser.newContext(); const page = await context.newPage();
      let posted = 0, checked = 0;
      page.on('request', req => { assert(new URL(req.url()).origin === proxyOrigin, 'App bypassed configured proxy'); if (req.method() === 'POST') { assert(req.url().endsWith('/commands')); posted++; } if (/\/commands\/[a-f0-9]{32}$/.test(req.url())) checked++; });
      await page.goto(`${proxyOrigin}/join`); await page.getByLabel('Single-seat bootstrap file').setInputFiles(path.join(privateDir, `seat-${index}.json`));
      await expect(page.getByRole('heading', { name: `Connected · Seat ${index + 1}`, exact: true })).toBeVisible();
      await page.getByRole('link', { name: 'Open live table', exact: true }).click();
      await page.getByRole('radio', { name: 'Keep hand', exact: true }).check();
      await page.getByRole('button', { name: 'Confirm opening choice', exact: true }).click();
      await expect(page.getByText('The table used your choice. Review the next decision.', { exact: true })).toBeVisible({ timeout: 15000 });
      assert(posted === 1 && checked >= 1, 'Proxy did not carry exact submission and authoritative status');
      browsers.push({ browser: index === 0 ? 'chromium' : 'firefox', version: browser.version(), posts: posted, executionReads: checked });
      await context.close();
    } finally { await browser.close(); }
  }
  fs.writeFileSync(path.join(output, 'report.json'), JSON.stringify({ status: 'passed', scope: 'Production App via restricted Vite preview and real native command/execution endpoints. No query or cancellation proxy route.', binary_sha256: sha(fs.readFileSync(binary)), bundle_sha256: bundleHash, presentation_sha256: presentationHash, harness_sha256: sha(fs.readFileSync(new URL(import.meta.url))), checks, authenticatedBodySchemaChecks: 4, nativeExecutionCancelChecks: 5, browsers }, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ status: 'passed', routeChecks: checks.length, authenticatedBodySchemaChecks: 4, nativeExecutionCancelChecks: 5, browsers: browsers.length }));
} finally { await stop(proxy); await stop(host); fs.closeSync(proxyLog); fs.closeSync(hostLog); }
