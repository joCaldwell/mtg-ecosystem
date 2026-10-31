#!/usr/bin/env node
// The Select contract, driven with real keys and pointers against a dedicated
// serve.ts fixture: every migrated control still does what it did, the keyboard
// matches a listbox, and open lists are never clipped.
//
//   node serve.ts --port 8871 --seed <seed.db> --dist <dist>
//   node check-selects.mjs [--origin http://127.0.0.1:8871] [--out <dir>]
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { chromium } from 'playwright-core';
import { UX_EVAL_ROOT } from './lib/paths.mjs';
import { BROWSER_ARGS, VIEWPORTS, newContext, watchPage, settle, scrollUntilVisible, shoot } from './lib/browser.mjs';

const opt = name => { const i = process.argv.indexOf('--' + name); return i < 0 ? undefined : process.argv[i + 1]; };
const origin = opt('origin') ?? 'http://127.0.0.1:8871';
assert.equal(new URL(origin).hostname, '127.0.0.1');
assert(['8870', '8871'].includes(new URL(origin).port), 'Use a reserved, dedicated fixture server');
const out = opt('out') ?? join(UX_EVAL_ROOT, 'select-candidate');
mkdirSync(out, { recursive: true });
const browser = await chromium.launch({ args: BROWSER_ARGS });
const results = [];
const api = async path => {
  const response = await fetch(origin + path, { headers: { Cookie: 'uxeval_user=1' } });
  assert.equal(response.status, 200); return response.json();
};
const deckId = 2;
const before = (await api(`/api/decks/${deckId}`)).state, settings = await api('/api/settings'), decks = await api('/api/decks');

async function open(width) {
  const vp = VIEWPORTS[width], context = await newContext(browser, { origin, vp, user: 1, frozenNow: '2026-10-20T17:00:00Z', notes: { blocked: new Set(), imageFailures: new Set() } });
  const page = await context.newPage(), watch = watchPage(page); page.setDefaultTimeout(12000);
  return { vp, context, page, watch };
}
const value = control => control.getAttribute('data-value');
const focused = control => control.evaluate(el => el === document.activeElement);
const expanded = async control => (await control.getAttribute('aria-expanded')) === 'true';
const activeText = page => page.evaluate(() => {
  const id = document.activeElement?.getAttribute('aria-activedescendant');
  return id ? document.getElementById(id)?.textContent.slice(1) : null;
});

/** Opens a control and walks to `wanted` with physical keys, then Enter. */
async function choose(session, control, wanted) {
  const { page, vp, watch } = session;
  await settle(page, watch);
  await scrollUntilVisible(page, control, vp);
  await control.focus();
  await control.press('Enter');
  const options = await page.getByRole('option').evaluateAll(els => els.map(el => ({ value: el.dataset.value, disabled: el.getAttribute('aria-disabled') === 'true' })));
  const enabled = options.filter(option => !option.disabled), index = enabled.findIndex(option => option.value === String(wanted));
  assert(index >= 0, `Expected option ${wanted}: ${JSON.stringify(options)}`);
  await page.keyboard.press('Home');
  for (let i = 0; i < index; i++) await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await settle(page, watch);
  assert.equal(await value(control), String(wanted));
  assert.equal(await expanded(control), false);
}

/** Every list opened at this width stays inside the viewport and over its page. */
async function assertUnclipped(page, name) {
  const list = page.getByRole('listbox').last();
  await list.waitFor();
  const facts = await list.evaluate(el => {
    const box = el.closest('[class*="popover"]') ?? el, r = box.getBoundingClientRect();
    const top = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
    return { left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: innerWidth, height: innerHeight, onTop: box.contains(top), scrollable: el.scrollHeight > el.clientHeight };
  });
  assert(facts.left >= 0 && facts.top >= 0 && facts.right <= facts.width && facts.bottom <= facts.height, `${name}: list stays in the viewport ${JSON.stringify(facts)}`);
  assert(facts.onTop, `${name}: list paints above its surroundings`);
  return facts;
}

try {
  // ---- the keyboard contract, at /design where every state exists ----
  {
    const session = await open(1440), { page, watch } = session;
    await page.goto(origin + '/design'); await settle(page, watch);
    await page.getByRole('tab', { name: 'Forms', exact: true }).click().catch(() => {});
    await settle(page, watch);
    const combo = name => page.getByRole('combobox', { name, exact: true });
    const example = combo('Select example');
    await example.waitFor();
    await page.mouse.click(1, 1);
    await page.evaluate(() => document.activeElement?.blur?.());

    // Closed: Down, Up, Enter and Space open; focus never leaves the trigger.
    for (const key of ['ArrowDown', 'ArrowUp', 'Enter', ' ']) {
      await example.focus(); await page.keyboard.press(key);
      assert.equal(await expanded(example), true, `${key} opens`);
      assert.equal(await focused(example), true, `${key} keeps focus`);
      assert.equal(await activeText(page), 'Brawl', `${key} starts on the selection`);
      await page.keyboard.press('Escape');
      assert.equal(await expanded(example), false, 'Escape closes');
      assert.equal(await focused(example), true, 'Escape returns focus');
      assert.equal(await value(example), 'b', 'Escape changes nothing');
    }
    // Home and End from closed, then within the list.
    await page.keyboard.press('Home'); assert.equal(await activeText(page), 'Commander');
    await page.keyboard.press('Escape'); await page.keyboard.press('End'); assert.equal(await activeText(page), 'Standard');
    await page.keyboard.press('ArrowDown'); assert.equal(await activeText(page), 'Standard', 'Down stops at the end');
    await page.keyboard.press('Home'); await page.keyboard.press('ArrowUp'); assert.equal(await activeText(page), 'Commander', 'Up stops at the start');
    await page.keyboard.press('Escape');
    // Space commits, type-ahead moves and repeats, a pause starts afresh.
    await page.keyboard.press('Enter'); await page.keyboard.press('ArrowDown'); await page.keyboard.press(' ');
    assert.equal(await value(example), 'c', 'Space commits');
    assert.equal(await focused(example), true, 'A commit keeps focus');
    await page.keyboard.press('b'); assert.equal(await activeText(page), 'Brawl', 'A letter opens onto its match');
    await page.keyboard.press('Enter'); assert.equal(await value(example), 'b');
    // Tab commits what is highlighted and moves on, like a native select.
    await page.keyboard.press('ArrowDown'); await page.keyboard.press('ArrowDown'); await page.keyboard.press('Tab');
    assert.equal(await value(example), 'c', 'Tab keeps the highlighted option');
    assert.equal(await expanded(example), false, 'Tab closes');
    assert.equal(await combo('Required format').evaluate(el => el === document.activeElement), true, 'Tab reaches the next control');
    // A disabled control is skipped, a disabled option is stepped over.
    const required = combo('Required format');
    assert.equal(await required.getAttribute('aria-invalid'), 'true');
    assert.match(await required.textContent(), /Choose a format…/);
    await required.press('Enter'); await page.keyboard.press('ArrowDown'); await page.keyboard.press('Enter');
    assert.equal(await value(required), 'brawl'); assert.equal(await required.getAttribute('aria-invalid'), null);
    await page.keyboard.press('Tab');
    const skipping = combo('Select with a disabled option');
    assert.equal(await skipping.evaluate(el => el === document.activeElement), true, 'Tab skips a disabled Select');
    assert.equal(await combo('Disabled select example').isDisabled(), true);
    await skipping.press('Enter'); await page.keyboard.press('ArrowDown');
    assert.equal(await activeText(page), 'Also available', 'Down skips a disabled option');
    await page.keyboard.press('Escape');
    // A long list scrolls the active option into view; type-ahead reaches it.
    const long = combo('Long select example');
    await long.press('Enter');
    const scrollable = await assertUnclipped(page, 'long list'); assert.equal(scrollable.scrollable, true);
    await page.keyboard.press('End');
    assert.equal(await activeText(page), 'Option 24');
    assert.equal(await page.evaluate(() => {
      const option = document.getElementById(document.activeElement.getAttribute('aria-activedescendant')), list = option.parentElement;
      const a = option.getBoundingClientRect(), b = list.getBoundingClientRect(); return a.top >= b.top && a.bottom <= b.bottom + 1;
    }), true, 'The active option is scrolled into view');
    await page.keyboard.press('Escape');
    // The chevron turns over and back, and is one size on every kind of trigger.
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await example.click();
    assert.equal(await example.locator('svg').evaluate(el => getComputedStyle(el).transitionDuration), '0.2s');
    await page.waitForTimeout(260);
    assert.match(await example.locator('svg').evaluate(el => getComputedStyle(el).transform), /^matrix\(-1/);
    await page.keyboard.press('Escape'); await page.waitForTimeout(260);
    assert.match(await example.locator('svg').evaluate(el => getComputedStyle(el).transform), /^(none|matrix\(1)/);
    await session.context.close();
    results.push({ keyboardContract: true });
  }

  // ---- reduced motion: no rotation animation ----
  {
    const session = await open(1440), { page, watch } = session;
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto(origin + '/decks'); await settle(page, watch);
    assert.equal(await page.getByRole('combobox', { name: 'Lifecycle', exact: true }).locator('svg').evaluate(el => getComputedStyle(el).transitionDuration), '0s');
    await session.context.close();
    results.push({ reducedMotion: true });
  }

  // ---- every chevron in the app is one size and one distance from its edge ----
  {
    const session = await open(1440), { page, watch } = session;
    await page.goto(origin + `/deck/${deckId}`); await settle(page, watch);
    const chevrons = await page.locator('button[aria-expanded] svg[class*="chevron"]').evaluateAll(els => els.map(el => {
      const button = el.closest('button'), face = button.querySelector('span[class*="pillFace"]') ?? button, r = el.getBoundingClientRect(), b = face.getBoundingClientRect();
      return { name: button.getAttribute('aria-label') ?? button.textContent, width: r.width, height: r.height, inset: Math.round(b.right - r.right), centred: Math.abs((r.top + r.bottom) / 2 - (b.top + b.bottom) / 2) <= 1 };
    }).filter(item => item.width <= 16));
    assert(chevrons.length >= 5, 'Header, tag filter, group and sort chevrons were found');
    for (const c of chevrons) {
      assert.equal(c.width, 14, `${c.name} chevron width`); assert.equal(c.height, 14, `${c.name} chevron height`);
      assert.equal(c.centred, true, `${c.name} chevron centred`);
    }
    assert.equal(new Set(chevrons.map(c => c.inset)).size, 1, `One inset: ${JSON.stringify(chevrons)}`);
    results.push({ chevrons });
    await session.context.close();
  }

  // ---- every migrated control, at each width ----
  for (const width of [1440, 768]) {
    const session = await open(width), { page, watch, vp } = session;
    const combo = (name, scope = page) => scope.getByRole('combobox', { name, exact: true });
    const capture = async name => { await settle(page, watch); await shoot(page, join(out, `${name}-${width}.png`), { full: false }); assert.deepEqual(watch.errors, [], name); };

    // Lifecycle filter: a value changes the roster, and the list stays open only when asked.
    await page.goto(origin + '/decks'); await settle(page, watch);
    for (const category of ['archived', 'paper', 'theorycrafted', 'all']) {
      await choose(session, combo('Lifecycle'), category);
      const ids = await page.locator('main ul a[href^="/deck/"]').evaluateAll(els => els.map(el => Number(el.getAttribute('href').split('/')[2])));
      assert.deepEqual([...new Set(ids)].sort((a, b) => a - b), decks.filter(deck => category === 'all' || deck.category === category).map(deck => deck.id).sort((a, b) => a - b), `Lifecycle ${category}`);
    }
    // Inside a dialog: Escape closes the list and leaves the dialog open.
    await page.getByRole('button', { name: 'New deck', exact: true }).click();
    let dialog = page.getByRole('dialog', { name: 'New deck', exact: true });
    const lifecycle = combo('Lifecycle', dialog);
    await lifecycle.focus(); await lifecycle.press('Enter'); await assertUnclipped(page, 'new deck lifecycle');
    await page.keyboard.press('Escape');
    assert.equal(await dialog.isVisible(), true, 'Escape leaves the dialog open');
    assert.equal(await expanded(lifecycle), false); assert.equal(await focused(lifecycle), true);
    await choose(session, lifecycle, 'paper');
    await page.keyboard.press('Escape'); await settle(page, watch);
    assert.equal(await dialog.count(), 0, 'A second Escape closes the dialog');

    await page.goto(origin + '/memories'); await settle(page, watch);
    const composer = page.locator('textarea').first();
    await composer.fill('Unsaved scope draft');
    await choose(session, combo('Scope'), 'global');
    assert.equal(await composer.inputValue(), 'Unsaved scope draft');
    await choose(session, combo('Scope'), 'all');

    await page.goto(origin + '/settings'); await settle(page, watch);
    await choose(session, combo('Connection method'), 'codex');
    await page.getByLabel('Codex configuration', { exact: true }).waitFor();
    await choose(session, combo('Connection method'), 'claude-code');
    await choose(session, combo('Currency'), 'EUR');
    assert.equal(await value(combo('Store')), 'lowest');
    await choose(session, combo('Store'), 'cardmarket');
    await page.getByRole('button', { name: 'Save price source', exact: true }).click(); await settle(page, watch);
    assert.deepEqual((await api('/api/settings')).price_source, { currency: 'EUR', store: 'cardmarket' });
    await choose(session, combo('Currency'), settings.price_source.currency);
    await choose(session, combo('Store'), settings.price_source.store);
    await page.getByRole('button', { name: 'Save price source', exact: true }).click(); await settle(page, watch);
    assert.deepEqual((await api('/api/settings')).price_source, settings.price_source);

    // The deck: toolbar choices, and the card panel's writes.
    await page.goto(origin + `/deck/${deckId}`); await settle(page, watch);
    // The visible word beside a control is its label: it opens and closes it.
    const groupLabel = page.locator('label').filter({ hasText: 'Group' }).getByText('Group', { exact: true });
    await groupLabel.click(); assert.equal(await expanded(combo('Group the list')), true, 'The label opens it');
    await groupLabel.click(); assert.equal(await expanded(combo('Group the list')), false, 'The label closes it');
    // Scrolling the page under an open list carries the list with its trigger.
    if (width < 1100) {
      const trigger = combo('Group the list');
      await trigger.click();
      const box = await trigger.boundingBox();
      await page.mouse.move(vp.width / 2, vp.height - 40);
      await page.mouse.wheel(0, 120); await settle(page, watch);
      const moved = await trigger.boundingBox(), listTop = await page.getByRole('listbox').evaluate(el => el.closest('[class*="popover"]').getBoundingClientRect().top);
      assert(moved.y < box.y, 'The page scrolled');
      assert(Math.abs(listTop - (moved.y + moved.height)) <= 6, `List follows its trigger: ${listTop} ${moved.y + moved.height}`);
      await page.keyboard.press('Escape');
      await page.mouse.wheel(0, -120); await settle(page, watch);
    }
    await choose(session, combo('Group the list'), 'type');
    await choose(session, combo('Order the cards in each group'), 'price');
    await capture('toolbar-changed');
    await choose(session, combo('Group the list'), 'slot');
    await choose(session, combo('Order the cards in each group'), 'curve');

    const card = before.cards.find(item => !item.maybe && item.role === 'card');
    const row = page.locator(`[data-deck-list] [data-oracle-id="${card.oracle_id}"]`).first();
    await scrollUntilVisible(page, row, vp); await row.click(); await settle(page, watch);
    const primary = combo('Primary slot');
    const other = before.slots.find(slot => slot.id !== card.slot_id).id;
    await choose(session, primary, other);
    assert.equal((await api(`/api/decks/${deckId}`)).state.cards.find(item => item.oracle_id === card.oracle_id).slot_id, other);
    await choose(session, primary, card.slot_id ?? '');
    assert.equal((await api(`/api/decks/${deckId}`)).state.cards.find(item => item.oracle_id === card.oracle_id).slot_id, card.slot_id);

    // Board only offers eligible roles now. Use a DB-resolved Companion
    // rather than putting an arbitrary card outside the game through the UI.
    const roleName = 'Jegantha, the Wellspring';
    const roleCard = (await api('/api/search?filter=off&q=' + encodeURIComponent(`!"${roleName}"`))).find(item => item.name === roleName);
    assert(roleCard, 'Companion fixture resolves exactly');
    assert(!before.cards.some(item => item.oracle_id === roleCard.oracle_id), 'temporary Companion is absent before this check');
    const roleResponse = await session.context.request.post(`${origin}/api/decks/${deckId}/cards`, { headers: { Origin: origin }, data: { oracle_id: roleCard.oracle_id } });
    assert(roleResponse.ok());
    await page.goto(origin + `/deck/${deckId}`); await settle(page, watch);
    const roleRow = page.locator(`[data-deck-list] button[data-oracle-id="${roleCard.oracle_id}"]`).first();
    await scrollUntilVisible(page, roleRow, vp); await roleRow.click(); await settle(page, watch);
    // Board is not a select: a pressed button writes the role and shows it.
    const board = page.getByRole('group', { name: 'Board', exact: true });
    await scrollUntilVisible(page, board, vp);
    const boardBox = await board.boundingBox(), labels = await board.getByRole('button').evaluateAll(els => els.map(el => ({ text: el.textContent, fits: el.scrollWidth <= el.clientWidth, height: el.getBoundingClientRect().height, pressed: el.getAttribute('aria-pressed') })));
    assert.deepEqual(labels.map(label => label.text), ['Main deck', 'Commander', 'Companion']);
    assert(labels.every(label => label.fits), `Board labels fit: ${JSON.stringify(labels)}`);
    if (width < 1100) assert(labels.every(label => label.height >= 44), 'Board is one tap tall');
    assert.equal(labels[0].pressed, 'true');
    await board.getByRole('button', { name: 'Companion', exact: true }).click(); await settle(page, watch);
    assert.equal((await api(`/api/decks/${deckId}`)).state.cards.find(item => item.oracle_id === roleCard.oracle_id).role, 'companion');
    assert.equal(await board.getByRole('button', { name: 'Companion', exact: true }).getAttribute('aria-pressed'), 'true');
    await capture('board-companion');
    await board.getByRole('button', { name: 'Main deck', exact: true }).click(); await settle(page, watch);
    assert.equal((await api(`/api/decks/${deckId}`)).state.cards.find(item => item.oracle_id === roleCard.oracle_id).role, 'card');
    results.push({ width, boardBox, boardLabels: labels });
    if (width < 1100) { await page.keyboard.press('Escape'); await settle(page, watch); await page.keyboard.press('Escape'); await settle(page, watch); }

    // Near the bottom edge a list flips above its trigger, and the panel does not move.
    if (width >= 1100) {
      await page.setViewportSize({ width, height: 620 }); await settle(page, watch);
      await scrollUntilVisible(page, primary, { ...vp, height: 620 });
      const panelTop = await page.locator('[data-deck-panel]').evaluate(el => el.getBoundingClientRect().top);
      await primary.focus(); await primary.press('Enter');
      const facts = await assertUnclipped(page, 'near the edge');
      const trigger = await primary.boundingBox();
      assert(facts.bottom <= trigger.y + 1 || facts.top >= trigger.y + trigger.height - 1 || facts.bottom <= 620, 'List sits beside its trigger');
      assert.equal(await page.locator('[data-deck-panel]').evaluate(el => el.getBoundingClientRect().top), panelTop, 'Opening shifts nothing');
      await page.keyboard.press('Escape');
      await page.setViewportSize({ width, height: vp.height }); await settle(page, watch);
    }

    // The power editor: a dialog of several, none changed by opening and leaving.
    if (width >= 1100) {
      await page.getByRole('button', { name: /^Back to/ }).click(); await settle(page, watch);
      await page.getByRole('navigation', { name: 'Deck panel views' }).getByRole('button', { name: 'Power', exact: true }).click();
      await page.getByRole('button', { name: 'Assess deck', exact: true }).click(); await settle(page, watch);
      dialog = page.getByRole('dialog', { name: 'Assess the current deck', exact: true });
      const controls = await dialog.getByRole('combobox').all();
      assert(controls.length >= 4);
      for (const control of controls) {
        await control.focus(); await control.press('End'); await assertUnclipped(page, 'power editor');
        await page.keyboard.press('Enter'); await settle(page, watch);
      }
      await capture('power-editor');
      await dialog.getByRole('button', { name: 'Cancel', exact: true }).click(); await settle(page, watch);

      await page.getByRole('button', { name: 'More', exact: true }).click(); await page.getByRole('menuitem', { name: 'Compare', exact: true }).click();
      dialog = page.getByRole('dialog', { name: 'Compare decks', exact: true });
      const compare = combo('Compare with', dialog);
      assert.equal(await focused(compare), true, 'data-autofocus reaches the trigger');
      const other = decks.find(deck => deck.id !== deckId);
      await choose(session, compare, other.id); await capture('compare');
      await dialog.getByRole('button', { name: 'Close', exact: true }).first().click(); await settle(page, watch);

      // Playtest: the library viewer's mode.
      await page.goto(origin + `/deck/${deckId}/playtest`); await settle(page, watch);
      if (!await page.getByRole('dialog', { name: 'New game', exact: true }).count()) {
        await page.getByRole('button', { name: 'More', exact: true }).click(); await page.getByRole('menuitem', { name: /^New game/ }).click();
      }
      await page.getByRole('button', { name: 'Shuffle and draw seven', exact: true }).click(); await settle(page, watch);
      await page.getByRole('button', { name: 'Keep hand', exact: true }).click(); await settle(page, watch);
      await page.getByRole('button', { name: /^Library\b/ }).first().click(); await settle(page, watch);
      const mode = combo('Library viewer mode');
      await choose(session, mode, 'search');
      assert.equal(await page.getByRole('spinbutton', { name: 'Top cards to reveal', exact: true }).count(), 0);
      await choose(session, mode, 'top');
      await page.getByRole('spinbutton', { name: 'Top cards to reveal', exact: true }).waitFor();
      await capture('playtest-library');
    }
    await session.context.close();
    const removedRoleCard = await fetch(`${origin}/api/decks/${deckId}/cards/${roleCard.oracle_id}`, { method: 'DELETE', headers: { Cookie: 'uxeval_user=1', Origin: origin } });
    assert(removedRoleCard.ok, 'remove only the temporary Companion fixture');
    results.push({ width, migratedControls: true });
    console.log(`${width} ok`);
  }

  const after = (await api(`/api/decks/${deckId}`)).state;
  assert.deepEqual(after.cards, before.cards, 'Card assignments are restored');
  writeFileSync(join(out, 'qa.json'), JSON.stringify({ results }, null, 2) + '\n');
  console.log('check-selects passed');
} finally { await browser.close(); }
