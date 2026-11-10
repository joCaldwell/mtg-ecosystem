#!/usr/bin/env node
// The card panel's Copies control, with real input against a dedicated serve.ts
// fixture: bursts settle into one logged write, writes never race, the floor and
// the copy limit hold, and typing behaves as it always did.
//
//   node serve.ts --port 8871 --seed <seed.db> --dist <dist>
//   node check-copies.mjs [--origin http://127.0.0.1:8871]
import assert from 'node:assert/strict';
import { chromium } from 'playwright-core';
import { BROWSER_ARGS, VIEWPORTS, newContext, watchPage, scrollUntilVisible, settle } from './lib/browser.mjs';

const opt = name => { const i = process.argv.indexOf('--' + name); return i < 0 ? undefined : process.argv[i + 1]; };
const origin = opt('origin') ?? 'http://127.0.0.1:8871';
assert(['8870', '8871'].includes(new URL(origin).port), 'Use a reserved, dedicated fixture server');
const browser = await chromium.launch({ args: BROWSER_ARGS });
const headers = { Cookie: 'checks_user=1', Origin: origin, 'Content-Type': 'application/json' };
const api = async (path, init) => {
  const response = await fetch(origin + path, { headers, ...init });
  assert.equal(response.status, 200, path); return response.json();
};
const deckId = 4, name = 'Mountain', sibling = 'Plains';
const state = async () => (await api(`/api/decks/${deckId}`)).state;
const copies = async (card = name) => (await state()).cards.find(item => item.name === card).quantity;
const SETTLE = 500;

async function open(width, { deck = deckId, mutate } = {}) {
  const vp = VIEWPORTS[width], context = await newContext(browser, { origin, vp, user: 1, frozenNow: '2026-10-20T17:00:00Z', notes: { blocked: new Set(), imageFailures: new Set() } });
  const page = await context.newPage(), watch = watchPage(page); page.setDefaultTimeout(12000);
  if (mutate) await page.route(`**/api/decks/${deck}`, async route => {
    const response = await route.fetch(), body = await response.json();
    mutate(body.state); await route.fulfill({ response, json: body });
  });
  await page.goto(origin + `/deck/${deck}`); await settle(page, watch);
  return { context, page, watch };
}
const row = (page, card) => page.locator('[data-deck-list] button[data-oracle-id]').filter({ hasText: card }).first();
const controls = (page, card = name) => ({
  input: page.getByRole('textbox', { name: `Copies of ${card}`, exact: true }),
  up: page.getByRole('button', { name: `Add a copy of ${card}`, exact: true }),
  down: page.getByRole('button', { name: `Remove a copy of ${card}`, exact: true }),
});
/** Every PATCH of a quantity this page sends, in order, with completion. */
function watchWrites(page) {
  const writes = [];
  page.on('request', request => {
    if (request.method() === 'PATCH' && request.url().includes('/cards/')) writes.push({ body: request.postDataJSON(), done: false });
  });
  page.on('requestfinished', request => {
    if (request.method() === 'PATCH') { const write = writes.find(item => !item.done); if (write) write.done = true; }
  });
  return writes;
}

try {
  const start = await copies();
  assert.equal(start, 5, 'Seed has five Mountains');
  assert.equal((await state()).cards.find(item => item.name === name).copy_limit, null, 'A basic has no limit');
  assert.equal((await state()).cards.find(item => !/Basic/.test(item.type_line)).copy_limit, 1, 'A nonbasic is singleton');

  const { context, page, watch } = await open(1440);
  await row(page, name).click(); await settle(page, watch);
  const { input, up, down } = controls(page), writes = watchWrites(page);
  assert.equal(await input.inputValue(), '5');
  assert.equal(await up.getAttribute('title'), 'Add a copy of Mountain');

  // A burst shows at once, then settles into one write and one log entry.
  const log0 = (await state()).log.length, revision0 = (await state()).deck.revision;
  await up.click(); await up.click(); await up.click();
  assert.equal(await input.inputValue(), '8', 'The running number shows immediately');
  assert.equal(writes.length, 0, 'Nothing is written mid-burst');
  await page.waitForTimeout(SETTLE + 300); await settle(page, watch);
  assert.deepEqual(writes.map(write => write.body), [{ quantity: 8 }], 'One write');
  let after = await state();
  assert.equal(after.cards.find(item => item.name === name).quantity, 8);
  assert.equal(after.log.length, log0 + 1, 'One decision');
  assert.equal(after.deck.revision, revision0 + 1, 'One revision');
  // That one entry undoes the whole burst.
  await api(`/api/decks/${deckId}/log/${after.log[0].id}/undo`, { method: 'POST' }); await settle(page, watch);
  assert.equal(await copies(), 5, 'Undo restores the number before the burst');
  await input.waitFor(); await page.waitForFunction(() => document.querySelector('[aria-label="Copies of Mountain"]').value === '5');

  // There and back writes nothing.
  writes.length = 0;
  await up.click(); await up.click(); await down.click(); await down.click();
  await page.waitForTimeout(SETTLE + 200);
  assert.equal(writes.length, 0, 'A net of zero is not a write');

  // A slow write: a second burst during it waits, and the newest number wins.
  await page.route('**/cards/*', async route => { if (route.request().method() === 'PATCH') await new Promise(done => setTimeout(done, 900)); await route.continue(); });
  await up.click();
  await page.waitForTimeout(SETTLE + 100);
  assert.equal(writes.length, 1, 'First write is in flight');
  await up.click(); await up.click();
  assert.equal(await input.inputValue(), '8');
  await page.waitForTimeout(SETTLE + 100);
  assert.equal(writes.length, 1, 'The second write waits for the first');
  await page.waitForFunction(() => document.querySelector('[aria-label="Copies of Mountain"]').value === '8');
  await page.waitForTimeout(2000); await page.unroute('**/cards/*'); await settle(page, watch);
  assert.deepEqual(writes.map(write => write.body.quantity), [6, 8], 'In order');
  assert.equal(await copies(), 8, 'No update was lost');
  assert.equal(await input.inputValue(), '8');

  // Typing is what it was: Enter commits, Escape reverts, junk reverts.
  writes.length = 0;
  await input.click(); await page.keyboard.type('7'); await page.keyboard.press('Enter'); await settle(page, watch);
  assert.deepEqual(writes.map(write => write.body), [{ quantity: 7 }]);
  await input.click(); await page.keyboard.type('9'); await page.keyboard.press('Escape');
  assert.equal(await input.inputValue(), '7', 'Escape reverts');
  await page.keyboard.press('Tab'); await page.keyboard.press('Shift+Tab'); await settle(page, watch);
  await page.keyboard.type('abc'); await page.getByRole('heading', { name, exact: true }).click(); await settle(page, watch);
  assert.equal(await input.inputValue(), '7', 'Junk reverts');
  assert.equal(writes.length, 1, 'Neither reverted edit wrote');

  // A typed number then an arrow is one edit.
  writes.length = 0;
  await input.click(); await page.keyboard.type('3'); await up.click();
  assert.equal(await input.inputValue(), '4');
  await page.waitForTimeout(SETTLE + 300); await settle(page, watch);
  assert.deepEqual(writes.map(write => write.body), [{ quantity: 4 }]);
  // Tabbing off a typed number through the arrows still saves it.
  writes.length = 0;
  await input.click(); await page.keyboard.type('6'); await page.keyboard.press('Tab');
  assert.equal(await down.evaluate(el => el === document.activeElement), true, 'Tab reaches down, then up');
  await page.keyboard.press('Tab');
  assert.equal(await up.evaluate(el => el === document.activeElement), true);
  await page.waitForTimeout(SETTLE + 300); await settle(page, watch);
  assert.deepEqual(writes.map(write => write.body), [{ quantity: 6 }]);
  // Keys press the arrows too.
  await page.keyboard.press('Enter'); await page.keyboard.press(' ');
  assert.equal(await input.inputValue(), '8');
  await page.waitForTimeout(SETTLE + 300); await settle(page, watch);

  // The floor: down is off at 1, keeps focus, and never removes the card.
  await input.click(); await page.keyboard.type('2'); await page.keyboard.press('Enter'); await settle(page, watch);
  await down.focus(); await page.keyboard.press('Enter');
  assert.equal(await input.inputValue(), '1');
  assert.equal(await down.getAttribute('aria-disabled'), 'true');
  assert.equal(await down.evaluate(el => el === document.activeElement), true, 'Focus stays on down at the floor');
  await page.keyboard.press('Enter'); await down.click({ force: true });
  await page.waitForTimeout(SETTLE + 300); await settle(page, watch);
  assert.equal(await input.inputValue(), '1');
  assert.equal(await copies(), 1, 'Still in the deck at one copy');
  assert.equal(await up.getAttribute('aria-disabled'), null, 'A basic has no ceiling');

  // Moving to another card mid-burst saves the burst to the right card.
  await input.click(); await page.keyboard.type('5'); await page.keyboard.press('Enter'); await settle(page, watch);
  const plains = await copies(sibling);
  await up.click(); await up.click();
  await row(page, sibling).click(); await settle(page, watch);
  await page.waitForTimeout(300);
  assert.equal(await copies(), 7, 'The burst saved on leaving');
  assert.equal(await copies(sibling), plains, 'The next card is untouched');
  assert.equal(await controls(page, sibling).input.inputValue(), String(plains));
  assert.deepEqual(watch.errors, []);
  await context.close();
  await api(`/api/decks/${deckId}/cards/${(await state()).cards.find(item => item.name === name).oracle_id}`, { method: 'PATCH', body: JSON.stringify({ quantity: start }) });

  // The ceiling, from a printed limit.
  {
    const { context, page, watch } = await open(1440, { mutate: s => { s.cards.find(item => item.name === name).copy_limit = 6; } });
    await row(page, name).click(); await settle(page, watch);
    const { input, up, down } = controls(page);
    assert.equal(await up.getAttribute('aria-disabled'), null);
    await up.click();
    assert.equal(await input.inputValue(), '6');
    assert.equal(await up.getAttribute('aria-disabled'), 'true', 'Up is off at the limit');
    await up.click({ force: true });
    assert.equal(await input.inputValue(), '6');
    await down.click();
    await page.waitForTimeout(SETTLE + 200);
    assert.equal(await copies(), start);
    await context.close();
  }

  // Archived decks keep the plain row.
  {
    const { context, page, watch } = await open(1440, { deck: 1, mutate: s => { const card = s.cards.find(item => !item.maybe); card.quantity = 3; s.cards = [card, ...s.cards.filter(item => item !== card)]; } });
    const card = (await api('/api/decks/1')).state.cards.find(item => !item.maybe);
    await page.locator(`[data-deck-list] [data-oracle-id="${card.oracle_id}"]`).first().click(); await settle(page, watch);
    await page.getByText('Copies', { exact: true }).waitFor();
    assert.equal(await page.getByRole('button', { name: /copy of/ }).count(), 0, 'No arrows when read-only');
    await context.close();
  }

  // Every copy rule, with real cards. Only "A deck can have…" sets a limit: the
  // search effects on Squadron Hawk and Battalion Foot Soldier are not one.
  {
    const rulesDeck = 5, cards = { unlimited: 'Relentless Rats', capped: 'Seven Dwarves', searchesThree: 'Squadron Hawk', searchesAny: 'Battalion Foot Soldier' };
    const found = {};
    for (const [key, card] of Object.entries(cards)) {
      const results = await api('/api/search?filter=off&q=' + encodeURIComponent(`!"${card}"`));
      found[key] = results.find(item => item.name === card)?.oracle_id;
      assert(found[key], `${card} resolves in the card database`);
      await api(`/api/decks/${rulesDeck}/cards`, { method: 'POST', body: JSON.stringify({ oracle_id: found[key] }) });
    }
    const rulesState = async () => (await api(`/api/decks/${rulesDeck}`)).state;
    const held = async id => (await rulesState()).cards.find(item => item.oracle_id === id);
    assert.equal((await held(found.unlimited)).copy_limit, null, 'Any number reaches the panel as null');
    assert.equal((await held(found.capped)).copy_limit, 7, 'A printed cap reaches the panel');
    assert.equal((await held(found.searchesThree)).copy_limit, 1);
    assert.equal((await held(found.searchesAny)).copy_limit, 1);

    const { context, page, watch } = await open(1440, { deck: rulesDeck });
    const select = async id => {
      const row = page.locator(`[data-deck-list] [data-oracle-id="${id}"]`).first();
      await scrollUntilVisible(page, row, VIEWPORTS[1440]); await row.click(); await settle(page, watch);
    };
    // Any number: up from 1, past any cap, and down stops at 1.
    await select(found.unlimited);
    let { input, up, down } = controls(page, cards.unlimited);
    assert.equal(await input.inputValue(), '1');
    assert.equal(await down.getAttribute('aria-disabled'), 'true', 'Down is off at 1');
    await up.click(); await up.click();
    assert.equal(await input.inputValue(), '3');
    await page.waitForTimeout(SETTLE + 300); await settle(page, watch);
    assert.equal((await held(found.unlimited)).quantity, 3, 'Raised from 1');
    await input.click(); await page.keyboard.type('12'); await page.keyboard.press('Enter'); await settle(page, watch);
    await up.click();
    assert.equal(await input.inputValue(), '13', 'No ceiling');
    await page.waitForTimeout(SETTLE + 300); await settle(page, watch);
    await input.click(); await page.keyboard.type('2'); await page.keyboard.press('Enter'); await settle(page, watch);
    await down.click(); await down.click({ force: true });
    await page.waitForTimeout(SETTLE + 300); await settle(page, watch);
    assert.equal(await input.inputValue(), '1');
    assert.equal((await held(found.unlimited)).quantity, 1, 'Down stops at 1 and keeps the card');

    // A printed cap: up from 1, then off at seven.
    await select(found.capped);
    ({ input, up, down } = controls(page, cards.capped));
    assert.equal(await input.inputValue(), '1');
    for (let i = 0; i < 6; i++) await up.click();
    assert.equal(await input.inputValue(), '7');
    assert.equal(await up.getAttribute('aria-disabled'), 'true', 'Up is off at the printed cap');
    await up.click({ force: true });
    assert.equal(await input.inputValue(), '7');
    await page.waitForTimeout(SETTLE + 300); await settle(page, watch);
    assert.equal((await held(found.capped)).quantity, 7);

    // A singleton has no control at 1, even one whose text mentions other copies.
    for (const key of ['searchesThree', 'searchesAny']) {
      await select(found[key]);
      await page.getByRole('heading', { name: cards[key], exact: true }).waitFor();
      assert.equal(await page.getByRole('button', { name: /copy of/ }).count(), 0, `${cards[key]} has no Copies at 1`);
    }
    // A singleton somehow above 1 can be lowered, not raised, and the control leaves at 1.
    await api(`/api/decks/${rulesDeck}/cards/${found.searchesThree}`, { method: 'PATCH', body: JSON.stringify({ quantity: 2 }) });
    await settle(page, watch);
    await select(found.searchesThree);
    ({ input, up, down } = controls(page, cards.searchesThree));
    await input.waitFor();
    assert.equal(await input.inputValue(), '2');
    assert.equal(await up.getAttribute('aria-disabled'), 'true', 'Up is off above the limit');
    await down.click();
    await page.waitForTimeout(SETTLE + 300); await settle(page, watch);
    assert.equal((await held(found.searchesThree)).quantity, 1);
    assert.equal(await page.getByRole('button', { name: /copy of/ }).count(), 0, 'The control leaves at 1');
    assert.deepEqual(watch.errors, []);
    await context.close();
    for (const id of Object.values(found)) await api(`/api/decks/${rulesDeck}/cards/${id}`, { method: 'DELETE' });
  }

  // Touch widths: each arrow is a whole tap target, and the control one tall.
  {
    const { context, page, watch } = await open(768);
    await row(page, name).tap(); await settle(page, watch);
    const { input, up, down } = controls(page);
    for (const control of [up, down]) {
      const box = await control.boundingBox();
      assert(box.width >= 44 && box.height >= 44, `Arrow is a tap target: ${JSON.stringify(box)}`);
    }
    const height = await input.evaluate(el => el.parentElement.getBoundingClientRect().height);
    assert.equal(height, 44, 'One tap tall');
    await up.tap(); await up.tap();
    assert.equal(await input.inputValue(), '7');
    await down.tap(); await down.tap();
    await page.waitForTimeout(SETTLE + 200);
    assert.equal(await copies(), start);
    await context.close();
  }
  console.log('check-copies passed');
} finally { await browser.close(); }
