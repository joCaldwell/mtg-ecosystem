#!/usr/bin/env node
// Owner card-control choices, using only exact API-resolved card facts and a
// disposable fixture deck. Membership writes deliberately remain permissive.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { chromium } from 'playwright-core';
import { CHECKS_ROOT } from './lib/paths.mjs';
import { BROWSER_ARGS, VIEWPORTS, newContext, watchPage, settle, scrollUntilVisible, axe, shoot } from './lib/browser.mjs';
const option = name => { const i = process.argv.indexOf('--' + name); return i < 0 ? undefined : process.argv[i + 1]; };
const origin = option('origin') ?? 'http://127.0.0.1:8871';
assert.equal(new URL(origin).hostname, '127.0.0.1');
const out = option('out') ?? join(CHECKS_ROOT, 'manual', 'card-controls'); mkdirSync(out, { recursive: true });
const api = async (path, method = 'GET', body) => {
  const response = await fetch(origin + path, { method, headers: { Cookie: 'checks_user=1', Origin: origin, 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body) });
  assert.equal(response.status, 200, `${method} ${path}`); return response.json();
};
const cases = [
  ['land', 'Plains', false, false],
  ['legendary', 'Najeela, the Blade-Blossom', true, false],
  ['companion', 'Jegantha, the Wellspring', true, true],
  ['explicit-commander', 'Teferi, Temporal Archmage', true, false],
  ['both-controls', 'Cid, Timeless Artificer', true, false],
  ['copies-only', 'Nazgûl', false, false],
  ['back-face-legendary', 'Invasion of Ikoria // Zilortha, Apex of Ikoria', false, false],
];
const created = await api('/api/decks', 'POST', { name: 'Card controls study', category: 'theorycrafted' }), deck = created.state.deck.id;
const cards = [];
for (const [key, name, commander, companion] of cases) {
  const found = (await api('/api/search?filter=off&q=' + encodeURIComponent(`!"${name}"`))).find(card => card.name === name);
  assert(found, `${name} resolves exactly in the card database`);
  const response = await api(`/api/decks/${deck}/cards`, 'POST', { oracle_id: found.oracle_id });
  const card = response.state.cards.find(card => card.oracle_id === found.oracle_id);
  assert.equal(card.can_be_commander, commander, name); assert.equal(card.can_be_companion, companion, name);
  cards.push({ key, ...card });
}
const browser = await chromium.launch({ args: BROWSER_ARGS }), results = [];
try {
  for (const width of [1440, 768]) {
    const vp = VIEWPORTS[width], context = await newContext(browser, { origin, vp, user: 1, frozenNow: '2026-10-20T17:00:00Z', notes: { blocked: new Set(), imageFailures: new Set() } });
    const page = await context.newPage(), watch = watchPage(page);
    const panel = page.getByRole('complementary', { name: 'Deck panel' });
    const inspect = async card => {
      await page.goto(`${origin}/deck/${deck}`); await settle(page, watch);
      const hit = page.locator(`[data-deck-list] button[data-oracle-id="${card.oracle_id}"]`).first();
      await scrollUntilVisible(page, hit, vp); await hit.click(); await settle(page, watch);
      await panel.getByRole('heading', { name: card.name, exact: true }).waitFor();
    };
    for (const card of cards) {
      await inspect(card);
      const board = panel.getByRole('group', { name: 'Board', exact: true });
      const expected = ['Main deck', ...(card.can_be_commander ? ['Commander'] : []), ...(card.can_be_companion ? ['Companion'] : [])];
      assert.equal(await board.count(), expected.length > 1 ? 1 : 0, card.name);
      assert.equal(await panel.getByText('Board', { exact: true }).count(), 0, 'no redundant visible label');
      if (expected.length > 1) assert.deepEqual(await board.getByRole('button').allTextContents(), expected);
      const copies = panel.getByRole('textbox', { name: `Copies of ${card.name}`, exact: true });
      assert.equal(await copies.count(), card.copy_limit !== 1 ? 1 : 0, card.name);
      const sizes = width === 1440 ? [392, 340] : [null];
      for (const size of sizes) {
        if (size === 340) await page.getByRole('separator', { name: 'Resize the deck panel' }).press('Home');
        else if (size === 392) await page.getByRole('separator', { name: 'Resize the deck panel' }).dblclick();
        await settle(page, watch);
        const controls = panel.locator('[class*="_place_"]');
        if (await controls.count()) {
          await scrollUntilVisible(page, controls, vp);
          const geometry = await controls.evaluate(el => {
            const box = el.getBoundingClientRect();
            return { width: box.width, overflow: el.scrollWidth > el.clientWidth, boxes: [...el.children].map(child => { const b = child.getBoundingClientRect(); return { x: b.x, y: b.y, width: b.width, right: b.right, bottom: b.bottom }; }), right: box.right };
          });
          assert.equal(geometry.overflow, false, 'place controls never clip');
          assert(geometry.boxes.every(box => box.right <= geometry.right + 1));
          if (card.key === 'both-controls' && size === 392) assert(Math.abs(geometry.boxes[0].y - geometry.boxes[1].y) <= 1, 'Board and Copies share the default-width row');
          if (await copies.count()) {
            const stepperWidth = await copies.evaluate(el => el.parentElement.getBoundingClientRect().width);
            assert(stepperWidth < 160, 'Copies is sized to digits and arrows');
          }
        }
        if (card.key === 'both-controls' || card.key === 'back-face-legendary') await shoot(page, join(out, `${card.key}-${width}-${size ?? 'sheet'}.png`), { full: false });
      }
      results.push({ width, card: card.name, options: expected, copies: card.copy_limit !== 1 });
    }
    // A wrong role is still accepted through PATCH and remains offered until
    // the owner repairs it. No new membership eligibility restriction.
    const land = cards[0];
    const invalid = (await api(`/api/decks/${deck}/cards/${land.oracle_id}`, 'PATCH', { role: 'commander' })).state;
    assert.equal(invalid.cards.find(card => card.oracle_id === land.oracle_id).can_be_commander, false);
    assert(invalid.checks.some(finding => finding.key === `commander_ineligible:${land.oracle_id}`), 'the same eligibility predicate supplies the check');
    await inspect(land);
    const board = panel.getByRole('group', { name: 'Board', exact: true });
    assert.deepEqual(await board.getByRole('button').allTextContents(), ['Main deck', 'Commander']);
    assert.equal(await board.getByRole('button', { name: 'Commander', exact: true }).getAttribute('aria-pressed'), 'true');
    await scrollUntilVisible(page, board, vp); await board.getByRole('button', { name: 'Main deck', exact: true }).click(); await settle(page, watch);
    assert.equal(await board.count(), 0);
    const repaired = (await api(`/api/decks/${deck}`)).state;
    assert.equal(repaired.cards.find(card => card.oracle_id === land.oracle_id).role, 'card');
    assert.equal((await axe(page)).length, 0); assert.deepEqual(watch.errors, []);
    await context.close(); console.log(`Card controls ${width} passed`);
  }
  const tag = await api(`/api/decks/${deck}/tags`, 'POST', { name: 'Owned' });
  const tagId = tag.state.tags.find(item => item.name === 'Owned').id;
  await api(`/api/decks/${deck}/cards/${cards[0].oracle_id}`, 'PATCH', { tag_ids: [tagId] });
  await api(`/api/decks/${deck}`, 'PATCH', { category: 'archived' });
  const context = await newContext(browser, { origin, vp: VIEWPORTS[1440], user: 1, frozenNow: '2026-10-20T17:00:00Z', notes: { blocked: new Set(), imageFailures: new Set() } });
  const page = await context.newPage(), watch = watchPage(page);
  for (const card of cards) {
    await page.goto(`${origin}/deck/${deck}`); await settle(page, watch);
    const hit = page.locator(`[data-deck-list] button[data-oracle-id="${card.oracle_id}"]`).first();
    await scrollUntilVisible(page, hit, VIEWPORTS[1440]); await hit.click(); await settle(page, watch);
    const panel = page.getByRole('complementary', { name: 'Deck panel' });
    assert.equal(await panel.getByRole('group', { name: 'Board', exact: true }).count(), card.can_be_commander || card.can_be_companion ? 1 : 0);
    assert.equal(await panel.getByRole('textbox', { name: /Copies of/ }).count(), 0, 'archived copies stay read-only');
    assert.equal(await panel.getByRole('button', { name: /copy of/ }).count(), 0);
  }
  const filter = page.getByRole('button', { name: /Filter by tags$/ });
  await filter.click();
  await page.getByRole('option', { name: 'Owned', exact: true }).click();
  await page.keyboard.press('Escape'); await settle(page, watch);
  const filtered = page.locator('[data-deck-list] button[data-oracle-id]');
  assert.equal(await filtered.count(), 1, 'archived decks retain tag filtering');
  assert.equal(await filtered.getAttribute('data-oracle-id'), cards[0].oracle_id);
  assert.equal(await page.getByRole('button', { name: 'Create tags', exact: true }).count(), 0);
  await context.close();
  writeFileSync(join(out, 'qa.json'), JSON.stringify({ results, invalid_role_can_return_to_main: true, archived_eligibility_preserved: true, cards }, null, 2) + '\n');
  console.log('check-card-controls passed');
} finally {
  await browser.close();
  await api(`/api/decks/${deck}`, 'PATCH', { category: 'theorycrafted' });
  await api(`/api/decks/${deck}`, 'DELETE');
}
