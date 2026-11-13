import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { parseSeatBootstrap, type SeatResume } from '@multiverse/game-server-sdk/browser';

const paths = (process.env.MTG_CLIENT_SEAT_FILES ?? '').split(',').filter(Boolean);
if (paths.length !== 4) throw new Error('Supply four private single-seat bootstrap paths in MTG_CLIENT_SEAT_FILES');
const seats = paths.map(path => ({ path, bootstrap: parseSeatBootstrap(readFileSync(path, 'utf8')) }));
if (new Set(seats.map(seat => seat.bootstrap.viewer)).size !== 4) throw new Error('Four distinct seats required');
const baseURL = process.env.MTG_CLIENT_LIVE_ORIGIN!;

async function join(page: Page, index = 0) {
  await page.goto('/join');
  await page.getByLabel('Single-seat bootstrap file').setInputFiles(seats[index]!.path);
  await expect(page.getByRole('heading', { name: `Connected · Seat ${seats[index]!.bootstrap.viewer + 1}`, exact: true })).toBeVisible();
}
async function storage(page: Page) {
  return page.evaluate(async () => ({ local: localStorage.length, session: sessionStorage.length, databases: (await indexedDB.databases()).length, caches: (await caches.keys()).length, workers: (await navigator.serviceWorker.getRegistrations()).length }));
}
const emptyStorage = { local: 0, session: 0, databases: 0, caches: 0, workers: 0 };

// These are real native reads. Assertions and reports contain only bounded counts
// and public identities. No bearer/header/body/hand dump or trace is retained.
test('four authenticated seats: native identity, permitted hands, CSP, empty storage and GET-only traffic', async ({ browser }, info) => {
  const contexts: BrowserContext[] = [];
  const evidence: { viewer: number; cursor: string; ownHand: number; methods: string[]; requests: number }[] = [];
  try {
    for (const [index, seat] of seats.entries()) {
      const context = await browser.newContext({ baseURL, serviceWorkers: 'block' }); contexts.push(context);
      const page = await context.newPage();
      const errors: string[] = [], methods: string[] = []; let requests = 0;
      page.on('pageerror', () => errors.push('page-error'));
      page.on('console', message => { if (message.type() === 'error') errors.push('console-error'); });
      await page.addInitScript(() => { (window as unknown as { cspViolations: number }).cspViolations = 0; document.addEventListener('securitypolicyviolation', () => { (window as unknown as { cspViolations: number }).cspViolations++; }); });
      page.on('request', request => {
        requests++; methods.push(request.method());
        // Compare without printing credential-bearing strings on assertion failure.
        expect(seats.every(seat => !request.url().includes(seat.bootstrap.capability))).toBe(true);
        expect(new URL(request.url()).origin === new URL(baseURL).origin).toBe(true);
      });
      const resumeResponse = page.waitForResponse(response => response.url().endsWith('/resume') && response.status() === 200);
      const response = await page.goto('/join');
      const csp = response!.headers()['content-security-policy']!;
      expect(csp.includes("script-src 'self'")).toBe(true); expect(csp.includes('unsafe-eval')).toBe(false); expect(csp.includes('unsafe-inline')).toBe(false);
      await page.getByLabel('Single-seat bootstrap file').setInputFiles(seat.path);
      const wire = await resumeResponse;
      expect(wire.headers()['cache-control']).toBe('no-store');
      const read = await wire.json() as SeatResume;
      for (const key of ['protocol_version', 'match_id', 'viewer', 'engine_build_id', 'server_build_id', 'catalog_sha256'] as const) expect(read[key] === seat.bootstrap[key]).toBe(true);
      expect(read.state.status).toBe('available');
      if (read.state.status !== 'available') throw new Error('Expected source-qualified live view');
      expect(read.state.observation.view.viewer).toBe(seat.bootstrap.viewer);
      const hand = read.state.observation.view.objects.filter(object => object.zone.zone === 'hand');
      expect(hand.every(object => object.owner === seat.bootstrap.viewer)).toBe(true);
      expect(hand.length).toBeGreaterThan(0);
      await expect(page.getByRole('heading', { name: `Connected · Seat ${seat.bootstrap.viewer + 1}`, exact: true })).toBeVisible();
      await page.getByRole('link', { name: 'Open live table' }).click();
      await expect(page.getByRole('heading', { name: 'Commander table', exact: true })).toBeVisible();
      await expect(page.getByRole('region', { name: 'Your hand', exact: true }).getByRole('button')).toHaveCount(hand.length);
      await expect(page.getByText('Read-only table. Choices cannot be submitted.')).toBeVisible();
      const input = page.getByRole('button', { name: 'Jump to decision' });
      await input.focus(); await page.keyboard.press('Enter');
      await expect(page.getByRole('region', { name: 'Opening hand', exact: true })).toBeFocused();
      expect(await storage(page)).toEqual(emptyStorage);
      expect(await page.evaluate(() => (window as unknown as { cspViolations: number }).cspViolations)).toBe(0);
      expect(errors).toEqual([]);
      expect(methods.every(method => method === 'GET')).toBe(true);
      expect(await page.evaluate(() => location.search === '' && location.hash === '')).toBe(true);
      evidence.push({ viewer: seat.bootstrap.viewer, cursor: read.next, ownHand: hand.length, methods: [...new Set(methods)], requests });
      expect(index).toBeGreaterThanOrEqual(0);
    }
    await info.attach('read-proof', { body: JSON.stringify({ browser: browser.version(), seats: evidence }), contentType: 'application/json' });
  } finally { for (const context of contexts) await context.close(); }
});

test('same-profile duplicate tab is blocked, release allows explicit import, refresh forgets capability', async ({ context, page }) => {
  await join(page);
  const second = await context.newPage();
  let seatReads = 0;
  second.on('request', request => { if (request.url().includes('/v1/')) seatReads++; });
  await second.goto('/join'); await second.getByLabel('Single-seat bootstrap file').setInputFiles(seats[0]!.path);
  await expect(second.getByRole('heading', { name: 'Seat open in another tab', exact: true })).toBeVisible();
  expect(seatReads).toBe(0);
  await page.getByRole('button', { name: 'Release seat and clear view' }).click();
  await second.getByLabel('Single-seat bootstrap file').setInputFiles(seats[0]!.path);
  await expect(second.getByRole('heading', { name: 'Connected · Seat 1', exact: true })).toBeVisible();
  await second.reload();
  await expect(second.getByRole('heading', { name: 'Not connected', exact: true })).toBeVisible();
  expect(await storage(second)).toEqual(emptyStorage);
  await join(page);
  await second.close();
});

test('closing owner releases lock; missing Web Locks stops before authenticated requests', async ({ context, page }) => {
  await join(page);
  await page.close();
  const next = await context.newPage(); await join(next); await next.close();
  const unsupported = await context.newPage();
  await unsupported.addInitScript(() => Object.defineProperty(navigator, 'locks', { value: undefined, configurable: true }));
  let reads = 0; unsupported.on('request', request => { if (request.url().includes('/v1/')) reads++; });
  await unsupported.goto('/join'); await unsupported.getByLabel('Single-seat bootstrap file').setInputFiles(seats[0]!.path);
  await expect(unsupported.getByRole('heading', { name: 'Browser ownership unavailable', exact: true })).toBeVisible();
  expect(reads).toBe(0);
});

test('missing metadata remains usable; mismatched metadata digest fails closed (injected HTTP faults)', async ({ page }) => {
  await page.route('**/v1/matches/*/presentation', route => route.fulfill({ status: 404, contentType: 'application/json', body: '{}' }));
  await join(page); await page.getByRole('link', { name: 'Open live table' }).click();
  await expect(page.getByText('Presentation metadata unavailable. Observed identity and characteristics remain available.')).toBeVisible();
  const card = page.getByRole('region', { name: 'Command zone', exact: true }).getByRole('button').first();
  await card.focus(); await page.keyboard.press('Enter');
  await expect(page.getByRole('complementary', { name: 'Card inspector' }).getByText('Power', { exact: true })).toBeVisible();
  await page.keyboard.press('Escape'); await expect(card).toBeFocused();
  await page.getByRole('button', { name: 'Release seat and clear view' }).click();
  await page.unrouteAll();
  await page.route('**/presentation/*.json', route => route.fulfill({ status: 200, contentType: 'application/json', body: '{}' }));
  await page.goto('/join'); await page.getByLabel('Single-seat bootstrap file').setInputFiles(seats[0]!.path);
  await expect(page.getByRole('heading', { name: 'Incompatible table or presentation', exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Open live table' })).toHaveCount(0);
});

test('wrong configured origin rejects without bearer request; malformed seat file remains private', async ({ page }) => {
  let reads = 0; page.on('request', request => { if (request.url().includes('/v1/')) reads++; });
  await page.goto('/join');
  const value = { ...seats[0]!.bootstrap, server_origin: 'http://127.0.0.1:1' };
  await page.getByLabel('Single-seat bootstrap file').setInputFiles({ name: 'seat.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(value)) });
  await expect(page.getByRole('heading', { name: 'Unable to import this seat file', exact: true })).toBeVisible();
  expect(reads).toBe(0);
  await page.getByLabel('Single-seat bootstrap file').setInputFiles({ name: 'seat.json', mimeType: 'application/json', buffer: Buffer.from('{broken') });
  await expect(page.getByRole('heading', { name: 'Unable to import this seat file', exact: true })).toBeVisible();
  expect(reads).toBe(0); expect(await storage(page)).toEqual(emptyStorage);
});

test('late old-seat resume cannot replace newly imported seat (injected delayed response)', async ({ page }) => {
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  let first = true; let arrived!: () => void; const arrival = new Promise<void>(resolve => { arrived = resolve; });
  await page.route('**/v1/matches/*/resume', async route => {
    if (!first) { await route.continue(); return; }
    first = false;
    const response = await route.fetch(); arrived(); await gate;
    await route.fulfill({ response }).catch(() => {});
  });
  await page.goto('/join'); await page.getByLabel('Single-seat bootstrap file').setInputFiles(seats[0]!.path); await arrival;
  await page.getByLabel('Single-seat bootstrap file').setInputFiles(seats[1]!.path);
  await expect(page.getByRole('heading', { name: 'Connected · Seat 2', exact: true })).toBeVisible();
  release();
  await page.getByRole('link', { name: 'Open live table' }).click();
  await expect(page.getByText(/Live permitted view · seat 2/)).toBeVisible();
  await expect(page.getByText(/Live permitted view · seat 1/)).toHaveCount(0);
});

for (const [status, label] of [[401, 'Seat authorization rejected'], [429, 'Rate limit or possible contention'], [503, 'Host temporarily unavailable']] as const) {
  test(`distinct HTTP ${status} handling and bounded retries (injected host failure)`, async ({ page }) => {
    let reads = 0;
    await page.route('**/v1/matches/*/resume', route => { reads++; return route.fulfill({ status, contentType: 'application/json', body: '{}' }); });
    await page.goto('/join'); await page.getByLabel('Single-seat bootstrap file').setInputFiles(seats[0]!.path);
    await expect(page.getByRole('heading', { name: label, exact: true })).toBeVisible({ timeout: 20_000 });
    expect(reads).toBe(status === 401 ? 1 : 5);
  });
}
