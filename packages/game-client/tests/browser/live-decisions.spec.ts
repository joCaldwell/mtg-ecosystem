import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import opening from '../current-opening.ts';
import type { PlayerRequest, SeatResume } from '@multiverse/game-server-sdk';

// Synthetic host requests over an unchanged permitted capture. Tests establish
// App/SDK/journal composition, never native rules or source-backed reachability.
test.use({ trace: 'off', screenshot: 'off', video: 'off' });
const catalog = JSON.parse(readFileSync(new URL('../../../game-engine/tests/fixtures/expected-match-ir.json', import.meta.url), 'utf8'));
const deck = { commander: catalog.definitions[0].oracle_id, cards: [{ oracle_id: catalog.definitions[0].oracle_id, count: 99 }] };
const identity = { protocol_version: 1, match_id: opening.resume.match_id, engine_build_id: opening.resume.engine_build_id, server_build_id: opening.resume.server_build_id, catalog_sha256: opening.resume.catalog_sha256 };
async function host(page: Page, mode: 'discard' | 'payment' | 'land' | 'blocked') {
  const read = structuredClone(opening.resume) as SeatResume;
  if (read.state.status !== 'available') throw Error('fixture');
  const hand = read.state.observation.view.objects.filter(object => object.zone.zone === 'hand' && object.owner === 0);
  const token = 'synthetic-current-decision';
  const request: PlayerRequest = mode === 'discard' ? { request: 'discard', player: 0, token, candidates: [hand[0]!.handle], count: 1 }
    : mode === 'payment' ? { request: 'spend_mana', player: 0, token, cost: { generic: '9007199254740993', colored: ['U'] }, available: { U: '1', C: '9007199254740993' } }
      : mode === 'land' ? { request: 'priority', player: 0, token } : { request: 'announce_x', player: 0, token };
  read.state.observation.decision = request;
  if (mode === 'payment') read.state.observation.decision_context = { source: { kind: 'spell', oracle_id: 'synthetic-source', text: '' } };
  const commands: unknown[] = [];
  await page.route('**/client-config.json', route => route.fulfill({ json: { client_config_version: 1, server_origin: 'http://127.0.0.1:18741' } }));
  await page.route('**/v1/**', async route => {
    const incoming = route.request(), path = new URL(incoming.url()).pathname;
    if (path.endsWith('/manifest')) { await route.fulfill({ json: { ...identity, profile: opening.profile, decks: [deck, deck, deck, deck], starting_player: 0, catalog } }); return; }
    if (path.endsWith('/presentation')) { await route.fulfill({ status: 404, json: {} }); return; }
    if (path.endsWith('/resume')) { await route.fulfill({ json: read }); return; }
    if (path.endsWith('/events')) { await route.fulfill({ json: { protocol_version: 1, entries: [], next: read.next, has_more: false } }); return; }
    if (path.endsWith('/actions/query')) {
      await route.fulfill({ json: { protocol_version: 1, match_id: identity.match_id, viewer: 0, outcome: { status: 'complete', page: { token, scope: { scope: 'window' }, context: { window: 'priority' }, beginnings: [{ initiation: { action: 'play_land', object: hand[0]!.handle }, label: { available: { text: 'Play land' } } }] } } } }); return;
    }
    if (path.endsWith('/commands') && incoming.method() === 'POST') {
      const body = incoming.postData()!, command = JSON.parse(body);
      const saved = await page.evaluate(async () => {
        const db = await new Promise<IDBDatabase>((resolve, reject) => { const open = indexedDB.open('multiverse-command-journal', 1); open.onsuccess = () => resolve(open.result); open.onerror = () => reject(Error('journal')); });
        try { return await new Promise<string>((resolve, reject) => { const get = db.transaction('commands').objectStore('commands').getAll(); get.onsuccess = () => resolve(get.result[0]?.exactBody ?? ''); get.onerror = () => reject(Error('journal')); }); }
        finally { db.close(); }
      });
      expect(saved === body).toBe(true); expect(command.expected_revision).toBe(read.next); expect(command.command.token).toBe(token);
      commands.push(command.command.response);
      await route.fulfill({ json: { status: 'accepted', command_id: command.command_id } }); return;
    }
    if (/\/commands\/[a-f0-9]{32}$/.test(path)) { await route.fulfill({ json: { command_id: path.split('/').at(-1), status: { status: 'pending' } } }); return; }
    await route.fulfill({ status: 404, json: {} });
  });
  await page.goto('/join');
  await page.getByLabel('Single-seat bootstrap file').setInputFiles({ name: 'synthetic-seat.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify({ ...identity, bootstrap_version: 1, server_origin: 'http://127.0.0.1:18741', viewer: 0, capability: 'a'.repeat(64) })) });
  await expect(page.getByRole('heading', { name: 'Connected · Seat 1', exact: true })).toBeVisible();
  await page.getByRole('link', { name: 'Open live table' }).click();
  return { commands, handle: hand[0]!.handle };
}

test('live supplied discard submits its original context and journals exact bytes before sending', async ({ page }) => {
  const proof = await host(page, 'discard');
  const form = page.getByRole('group', { name: 'Choose cards to discard', exact: true });
  await form.getByRole('checkbox').check();
  await page.getByRole('button', { name: 'Confirm choice', exact: true }).click();
  await expect.poll(() => proof.commands.length).toBe(1);
  expect(proof.commands[0]).toEqual({ response: 'discard', objects: [proof.handle] });
});

test('live source-bound mana payment preserves exact quantities beyond Number precision', async ({ page }) => {
  const proof = await host(page, 'payment');
  const form = page.getByRole('group', { name: 'Choose mana to spend', exact: true });
  await form.getByRole('textbox', { name: 'Blue Available: 1', exact: true }).fill('1');
  await form.getByRole('textbox', { name: 'Colorless Available: 9007199254740993', exact: true }).fill('9007199254740993');
  await page.getByRole('button', { name: 'Confirm choice', exact: true }).click();
  await expect.poll(() => proof.commands.length).toBe(1);
  expect(proof.commands[0]).toEqual({ response: 'spend_mana', spending: { U: '1', C: '9007199254740993' } });
});

test('qualified producer action label enables explicit object initiation through the journal', async ({ page }) => {
  const proof = await host(page, 'land');
  const actions = page.getByRole('region', { name: 'Available actions', exact: true });
  await actions.getByRole('button', { name: 'Discover actions', exact: true }).click();
  const play = actions.getByRole('button', { name: /^Play land ·/ });
  await expect(play).toBeEnabled(); await play.click();
  await expect.poll(() => proof.commands.length).toBe(1);
  expect(proof.commands[0]).toEqual({ response: 'play_land', object: proof.handle });
});

test('missing producer meaning leaves the live choice blocked without any game command', async ({ page }) => {
  const proof = await host(page, 'blocked');
  await expect(page.getByRole('heading', { name: 'Decision context unavailable', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Confirm choice', exact: true })).toHaveCount(0);
  expect(proof.commands).toEqual([]);
});
