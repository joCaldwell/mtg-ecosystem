import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import opening from '../current-opening.ts';
import type { ActionScope, SeatResume } from '@multiverse/game-server-sdk';

// Production App + real built SDK, synthetic host envelopes. No native legality
// or full-game claim. Disable authenticated capture even for synthetic credentials.
test.use({ trace: 'off', screenshot: 'off', video: 'off' });
const catalog = JSON.parse(readFileSync(new URL('../../../game-engine/tests/fixtures/expected-match-ir.json', import.meta.url), 'utf8'));
const deck = { commander: catalog.definitions[0].oracle_id, cards: [{ oracle_id: catalog.definitions[0].oracle_id, count: 99 }] };
const identity = { protocol_version: 1, match_id: opening.resume.match_id, engine_build_id: opening.resume.engine_build_id, server_build_id: opening.resume.server_build_id, catalog_sha256: opening.resume.catalog_sha256 };
const cursor = 'f'.repeat(64);
const pass = { initiation: { action: 'pass' }, label: 'unavailable' };
const land = { initiation: { action: 'play_land', object: 'synthetic-land' }, label: 'unavailable' };
async function host(page: Page, mode: 'complete' | 'cancel' | 'stale') {
  const read = structuredClone(opening.resume) as SeatResume;
  if (read.state.status !== 'available') throw new Error('fixture');
  read.state.observation.decision = { request: 'priority', token: 'v0-d0', player: 0 };
  let calls = 0, submits = 0;
  const requests: string[] = [];
  let scope: ActionScope = { scope: 'window' };
  await page.route('**/client-config.json', route => route.fulfill({ json: { client_config_version: 1, server_origin: 'http://127.0.0.1:18741' } }));
  await page.route('**/v1/**', async route => {
    const request = route.request(), url = new URL(request.url());
    if (url.pathname.endsWith('/manifest')) { await route.fulfill({ json: { ...identity, profile: opening.profile, decks: [deck, deck, deck, deck], starting_player: 0, catalog } }); return; }
    if (url.pathname.endsWith('/presentation')) { await route.fulfill({ status: 404, json: {} }); return; }
    if (url.pathname.endsWith('/resume')) { await route.fulfill({ json: read }); return; }
    if (url.pathname.endsWith('/events')) { await route.fulfill({ json: { protocol_version: 1, entries: [], next: '1', has_more: false } }); return; }
    if (url.pathname.endsWith('/actions/query')) {
      const request = route.request().postDataJSON(); requests.push(request.operation); calls++;
      if (request.operation === 'start') scope = request.scope;
      const page = { token: 'v0-d0', scope, context: { window: 'priority' }, beginnings: calls === 1 ? [pass, land] : [] };
      const outcome = request.operation === 'cancel' ? { status: 'cancelled' }
        : mode === 'stale' ? { status: 'stale' }
          : mode === 'cancel' || calls === 1 ? { status: 'incomplete', cursor, required_minimum: 1, page }
            : { status: 'complete', page };
      await route.fulfill({ json: { protocol_version: 1, match_id: identity.match_id, viewer: 0, outcome } }); return;
    }
    if (url.pathname.endsWith('/commands') && request.method() === 'POST') {
      const body = request.postData()!; const command = JSON.parse(body);
      // Observe the journal BEFORE admitting the HTTP submission.
      const saved = await page.evaluate(async () => {
        const db = await new Promise<IDBDatabase>((resolve, reject) => { const open = indexedDB.open('multiverse-command-journal', 1); open.onsuccess = () => resolve(open.result); open.onerror = () => reject(new Error('open')); });
        try { return await new Promise<string>((resolve, reject) => { const read = db.transaction('commands').objectStore('commands').getAll(); read.onsuccess = () => resolve(read.result[0]?.exactBody ?? ''); read.onerror = () => reject(new Error('read')); }); }
        finally { db.close(); }
      });
      expect(saved === body).toBe(true); expect(command.expected_revision).toBe('1'); expect(command.command).toEqual({ kind: 'decision', token: 'v0-d0', response: { response: 'pass' } });
      submits++; await route.fulfill({ json: { status: 'accepted', command_id: command.command_id } }); return;
    }
    if (/\/commands\/[a-f0-9]{32}$/.test(url.pathname)) { await route.fulfill({ json: { command_id: url.pathname.split('/').at(-1), status: { status: 'pending' } } }); return; }
    await route.fulfill({ status: 404, json: {} });
  });
  await page.goto('/join');
  await page.getByLabel('Single-seat bootstrap file').setInputFiles({ name: 'synthetic-seat.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify({ ...identity, bootstrap_version: 1, server_origin: 'http://127.0.0.1:18741', viewer: 0, capability: 'a'.repeat(64) })) });
  await expect(page.getByRole('heading', { name: 'Connected · Seat 1', exact: true })).toBeVisible();
  await page.getByRole('link', { name: 'Open live table' }).click();
  return { requests, submits: () => submits };
}

test('bounded production discovery preserves keyboard focus and journals original pass bytes before submission', async ({ page }) => {
  const errors: string[] = []; page.on('pageerror', () => errors.push('page-error'));
  const proof = await host(page, 'complete');
  const actions = page.getByRole('region', { name: 'Available actions', exact: true });
  const discover = actions.getByRole('button', { name: 'Discover actions', exact: true });
  await discover.focus(); await page.keyboard.press('Enter');
  await expect(actions.getByText(/^Discovery complete/)).toBeVisible();
  await expect(discover).toBeFocused();
  await expect(actions.getByRole('button', { name: 'Play land — card context unavailable' })).toBeDisabled();
  const pass = actions.getByRole('button', { name: 'Pass priority', exact: true });
  await expect(pass).toBeEnabled(); await pass.focus(); await page.keyboard.press('Enter');
  await expect(pass).toHaveCount(0); await expect.poll(proof.submits).toBe(1);
  expect(proof.requests).toEqual(['start', 'continue']); expect(errors).toEqual([]);
  expect(await page.evaluate(() => ({ local: localStorage.length, session: sessionStorage.length }))).toEqual({ local: 0, session: 0 });
});

test('explicit stop cancels own query scratch and never submits partial actions', async ({ page }) => {
  const proof = await host(page, 'cancel');
  const actions = page.getByRole('region', { name: 'Available actions', exact: true });
  await actions.getByRole('button', { name: 'Discover actions', exact: true }).click();
  await expect(actions.getByText(/^Partial results/)).toBeVisible();
  await expect(actions.getByRole('button', { name: 'Pass priority', exact: true })).toBeDisabled();
  await actions.getByRole('button', { name: 'Stop discovery', exact: true }).click();
  await expect(actions.getByText(/^Discovery stopped/)).toBeVisible();
  await expect.poll(() => proof.requests.includes('cancel')).toBe(true); expect(proof.submits()).toBe(0);
});

test('stale result offers an explicit new discovery without an empty-complete claim', async ({ page }) => {
  const proof = await host(page, 'stale');
  const actions = page.getByRole('region', { name: 'Available actions', exact: true });
  await actions.getByRole('button', { name: 'Discover actions', exact: true }).click();
  await expect(actions.getByText('The decision changed. Discover actions again.')).toBeVisible();
  await expect(actions.getByText('No beginnings were supplied for this scope.')).toHaveCount(0);
  await expect(actions.getByRole('button', { name: 'Discover actions', exact: true })).toBeEnabled(); expect(proof.submits()).toBe(0);
});

// Object scope never offers global pass, even from a generated-valid but
// semantically mismatched synthetic producer response.
test('object scope retains exact selected handle and blocks global pass', async ({ page }) => {
  const proof = await host(page, 'complete');
  const actions = page.getByRole('region', { name: 'Available actions', exact: true });
  await actions.getByLabel('Discovery scope', { exact: true }).selectOption({ index: 1 });
  await actions.getByRole('button', { name: 'Discover actions', exact: true }).click();
  await expect(actions.getByText(/^Discovery complete/)).toBeVisible();
  await expect(actions.getByRole('button', { name: 'Pass priority', exact: true })).toBeDisabled();
  expect(proof.submits()).toBe(0);
});
