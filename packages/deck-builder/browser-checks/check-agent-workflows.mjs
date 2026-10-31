// Run only against the disposable fixture server. Real browser events exercise
// focus ownership; a committed response is deliberately dropped to test retry.
import assert from 'node:assert/strict';
import { chromium } from 'playwright-core';
import { Database } from '../src/sqlite.ts';
import { CHECKS_ROOT } from './lib/paths.mjs';
import { join } from 'node:path';

const base = process.env.CHECKS_URL ?? 'http://127.0.0.1:8879';
assert.equal(new URL(base).hostname, '127.0.0.1');
const api = async (path, method = 'GET', body) => {
  const response = await fetch(base + path, { method, headers: { Cookie: 'checks_user=1', Origin: base, 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body) });
  assert.equal(response.ok, true, `${method} ${path}: ${response.status}`);
  return response.json();
};
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
await context.addCookies([{ name: 'checks_user', value: '1', url: base }]);
// Headless tabs do not model OS visibility consistently. Drive the browser's
// visibility events explicitly while keeping the real hooks/network active.
await context.addInitScript(() => {
  window.auditVisibility = 'visible';
  Object.defineProperty(document, 'visibilityState', { get: () => window.auditVisibility });
});
const until = async predicate => {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (await predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error('Expected state did not arrive');
};
const focusIs = async id => (await api('/api/focus')).focus?.deck_id === id;
const pages = [], errors = [];
const page = async () => {
  const p = await context.newPage(); pages.push(p); p.on('pageerror', e => errors.push(e.message)); return p;
};
let created;
try {
  const a = await page();
  await a.goto(base + '/deck/4');
  await a.getByRole('heading', { name: 'Hail Caesar', exact: true }).waitFor();
  await a.evaluate(() => window.dispatchEvent(new Event('focus')));
  await until(() => focusIs(4));
  const b = await page();
  await b.goto(base + '/decks');
  assert.equal(await focusIs(4), true, 'My decks must not clear another tab');
  await b.goto(base + '/deck/2');
  await b.getByRole('heading', { name: 'Najeela Warriors', exact: true }).waitFor();
  await b.evaluate(() => window.dispatchEvent(new Event('focus')));
  await until(() => focusIs(2));
  await a.getByRole('link', { name: /Current agent context, reported .*: Najeela Warriors/ }).waitFor();
  await a.evaluate(() => { window.dispatchEvent(new Event('mousemove')); document.dispatchEvent(new Event('visibilitychange')); });
  await a.waitForTimeout(150);
  assert.equal(await focusIs(2), true, 'hover and visibility do not steal');
  await b.evaluate(() => { window.auditVisibility = 'hidden'; document.dispatchEvent(new Event('visibilitychange')); window.dispatchEvent(new Event('pointerdown')); });
  await a.waitForTimeout(150);
  assert.equal(await focusIs(2), true, 'hidden owner retains read context');
  await a.evaluate(() => window.dispatchEvent(new Event('keydown')));
  await until(() => focusIs(4));
  await b.close();
  assert.equal(await focusIs(4), true, 'non-owner closing does not clear');

  const reads = [];
  a.on('request', request => { if (request.method() === 'GET' && request.url().includes('/api/')) reads.push(new URL(request.url()).pathname); });
  await a.waitForTimeout(250); reads.length = 0;
  await api('/api/decks/2/cards/' + (await api('/api/decks/2')).state.cards[0].oracle_id, 'PATCH', { tag_ids: [] });
  await a.waitForTimeout(250);
  assert.ok(!reads.includes('/api/decks/4'), 'another deck edit must not reload the open composite');
  reads.length = 0;
  await api('/api/memories', 'POST', { scope: 'deck', deck_id: 4, text: 'Browser audit preference' });
  await until(() => reads.some(path => path === '/api/memories'));
  assert.ok(!reads.includes('/api/decks/4'), 'memory events refresh their own queries');

  // Age only this disposable fixture, so stale and hard expiry need no wait.
  await a.evaluate(() => { window.auditVisibility = 'hidden'; document.dispatchEvent(new Event('visibilitychange')); });
  const db = new Database(join(CHECKS_ROOT, 'serve', new URL(base).port, 'runtime.db'));
  try {
    const raw = JSON.parse(db.prepare("SELECT value FROM user_settings WHERE user_id=1 AND key='focus'").get().value);
    db.prepare("UPDATE user_settings SET value=? WHERE user_id=1 AND key='focus'").run(JSON.stringify({ ...raw, at: raw.at - 4 * 60_000 }));
    assert.equal((await api('/api/focus')).focus.stale, true);
    const monitor = await page(); await monitor.goto(base + '/decks');
    await monitor.getByRole('link', { name: /Last-viewed agent context \(stale\), reported 4 min ago/ }).waitFor();
    db.prepare("UPDATE user_settings SET value=? WHERE user_id=1 AND key='focus'").run(JSON.stringify({ ...raw, at: raw.at - 31 * 60_000 }));
    await monitor.reload();
    await monitor.getByText('No deck in agent context', { exact: true }).waitFor();
    await monitor.close();
  } finally { db.close(); }
  await a.evaluate(() => { window.auditVisibility = 'visible'; document.dispatchEvent(new Event('visibilitychange')); });
  await until(() => focusIs(4));
  await a.goto(base + '/decks');
  await until(async () => (await api('/api/focus')).focus === null);

  let attempts = 0;
  const keys = [];
  await a.route('**/api/decks', async route => {
    if (route.request().method() !== 'POST') return route.continue();
    attempts++; keys.push(route.request().headers()['idempotency-key']);
    const response = await route.fetch();
    if (attempts === 1) { created = (await response.json()).state.deck.id; return route.abort('failed'); }
    return route.fulfill({ response });
  });
  await a.getByRole('button', { name: 'New deck', exact: true }).click();
  await a.getByRole('textbox', { name: 'Deck name', exact: true }).fill('Browser retry audit');
  await a.getByRole('button', { name: 'Create deck', exact: true }).click();
  await a.waitForURL(url => created !== undefined && url.pathname === '/deck/' + created);
  assert.equal(attempts, 2);
  assert.ok(keys[0]); assert.equal(keys[0], keys[1]);
  assert.equal((await api('/api/decks')).filter(deck => deck.name === 'Browser retry audit').length, 1);
  assert.deepEqual(errors, []);
  console.log('PASS: multi-tab focus, hidden owner, compare-and-clear, stale/expired sidebar, scoped deck/memory updates, and retry after a committed response was lost.');
} finally {
  await context.close(); await browser.close();
  if (created) await api('/api/decks/' + created, 'DELETE');
}
