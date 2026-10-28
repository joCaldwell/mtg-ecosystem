#!/usr/bin/env node
// Native-select desktop audit. Only run against a dedicated serve.ts fixture.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { chromium } from 'playwright-core';
import { UX_EVAL_ROOT } from './lib/paths.mjs';
import { BROWSER_ARGS, VIEWPORTS, newContext, watchPage, settle, scrollUntilVisible, axe, shoot } from './lib/browser.mjs';
import { layoutDeck } from '../web/src/domain/grouping.ts';

const opt = name => { const i = process.argv.indexOf('--' + name); return i < 0 ? undefined : process.argv[i + 1]; };
const origin = opt('origin') ?? 'http://127.0.0.1:8871';
assert.equal(new URL(origin).hostname, '127.0.0.1');
assert(['8870', '8871'].includes(new URL(origin).port), 'Use a reserved, dedicated fixture server');
const out = opt('out') ?? join(UX_EVAL_ROOT, 'native-select-candidate');
mkdirSync(out, { recursive: true });
const auditOnly = process.argv.includes('--audit-only');
const browser = await chromium.launch({ args: BROWSER_ARGS });
const results = [];
const api = async path => {
  const response = await fetch(origin + path, { headers: { Cookie: 'uxeval_user=1' } });
  assert.equal(response.status, 200); return response.json();
};
const before = (await api('/api/decks/2')).state, settings = await api('/api/settings'), decks = await api('/api/decks');
async function metrics(page) {
  return page.locator('select').evaluateAll(els => els.map(el => {
    const r = el.getBoundingClientRect(), s = getComputedStyle(el), icon = el.nextElementSibling?.getBoundingClientRect();
    const canvas = document.createElement('canvas'), ctx = canvas.getContext('2d'); ctx.font = s.font;
    const text = el.selectedOptions[0]?.textContent.trim() ?? '';
    return { label: el.getAttribute('aria-label') ?? document.querySelector(`label[for="${el.id}"]`)?.textContent ?? '', text,
      value: el.value, width: r.width, height: r.height, x: r.x, y: r.y,
      font: s.font, paddingLeft: s.paddingLeft, paddingRight: s.paddingRight,
      textWidth: ctx.measureText(text).width, available: r.width - parseFloat(s.paddingLeft) - parseFloat(s.paddingRight) - 2,
      color: s.color, background: s.backgroundColor, border: s.borderColor,
      icon: icon && { x: icon.x, y: icon.y, width: icon.width, height: icon.height },
      disabled: el.disabled, invalid: el.getAttribute('aria-invalid') };
  }));
}
try {
  for (const width of (opt('widths')?.split(',').map(Number) ?? (auditOnly ? [1440] : [1280, 1440, 1920, 2560]))) {
    const vp = VIEWPORTS[width] ?? {width, height: 1024, touch: true, mobile: false}, context = await newContext(browser, { origin, vp, user: 1, frozenNow: '2026-10-20T17:00:00Z', notes: { blocked: new Set(), imageFailures: new Set() } });
    const page = await context.newPage(), watch = watchPage(page); page.setDefaultTimeout(12000);
    const combo = name => page.getByRole('combobox', { name, exact: true });
    // Open the real native control and use only physical keys. No selectOption,
    // DOM value assignment, synthetic change events or scripted scrolling.
    const choose = async (control, value, menuShot) => {
      await settle(page, watch);
      await scrollUntilVisible(page, control, vp);
      const options = await control.locator('option').evaluateAll(els => els.map(el => ({ value: el.value, disabled: el.disabled })));
      const enabled = options.filter(option => !option.disabled), index = enabled.findIndex(option => option.value === String(value));
      assert(index >= 0, 'Expected enabled native option ' + value + ': ' + JSON.stringify(options));
      await control.click();
      if (menuShot) await shoot(page, join(out, `${menuShot}-${width}.png`), { full: false });
      // Exercise both boundaries and both arrow directions across the suite.
      if (index > enabled.length / 2) {
        await page.keyboard.press('End');
        for (let i = enabled.length - 1; i > index; i--) await page.keyboard.press('ArrowUp');
      } else {
        await page.keyboard.press('Home');
        for (let i = 0; i < index; i++) await page.keyboard.press('ArrowDown');
      }
      await page.keyboard.press('Enter'); await settle(page, watch);
      if (menuShot) {
        // Chromium's native-popup screenshot can leave stale compositor tiles
        // behind. A viewport round trip repaints the page without DOM edits.
        await page.setViewportSize({width: width + 1, height: vp.height});
        await page.setViewportSize({width, height: vp.height}); await settle(page, watch);
      }
      assert.equal(await control.inputValue(), String(value));
      const focused = await control.evaluate(el => el === document.activeElement), label = await control.getAttribute('aria-label');
      if (label === 'Card face' || label === 'Attachment host') {
        // These consumers disable while saving. Record the browser's focus
        // result for baseline comparison; the primitive must preserve disabled.
        results.push({width, nativeSelectionFocus: {label, focused}});
        if (process.argv.includes('--expect-playtest-focus')) assert(focused, 'Saved native selection restores focus to the same control');
      } else assert(focused, 'Native focus stays on the select');
    };
    if (process.argv.includes('--states-only')) {
      await page.goto(origin + '/design'); await settle(page, watch);
      const example = combo('Select example'); await scrollUntilVisible(page, example, vp, {margin: 220});
      await choose(example, 'a'); await page.mouse.move(width - 1, vp.height - 1); await settle(page, watch);
      const focusedBorder = await example.evaluate(el => getComputedStyle(el).borderColor);
      await example.hover(); assert.equal(await example.evaluate(el => getComputedStyle(el).borderColor), focusedBorder, 'Hover retains keyboard focus border');
      await capture('select-focus-hover'); await example.press('Tab');
      const invalid = combo('Required format'); assert(await invalid.evaluate(el => el === document.activeElement));
      const invalidBorder = await invalid.evaluate(el => getComputedStyle(el).borderColor);
      await invalid.hover(); assert.equal(await invalid.evaluate(el => getComputedStyle(el).borderColor), invalidBorder, 'Hover retains invalid border');
      assert.notEqual(invalidBorder, focusedBorder); await capture('select-invalid-hover');
      await choose(invalid, 'commander'); assert.notEqual(await invalid.getAttribute('aria-invalid'), 'true');
      const disabled = combo('Disabled select example'); assert(await disabled.isDisabled()); await disabled.hover();
      assert.equal(await disabled.evaluate(el => getComputedStyle(el.nextElementSibling).opacity), '0.5');
      await capture('select-valid-disabled'); results.push({width, hoverPreservesFocusAndInvalid: true, invalidClearsAfterSelection: true, disabledChevronDimmed: true}); await context.close(); continue;
    }
    if (process.argv.includes('--pointer-only')) {
      await page.goto(origin + '/deck/2'); await settle(page, watch);
      const group = combo('Group the list'); await choose(group, 'slot');
      const bounds = await group.boundingBox(); await group.click();
      await shoot(page, join(out, `pointer-native-menu-${width}.png`), {full: false});
      // The inspected Chromium captures show two 26px native option rows below
      // this control. Click Card type in the platform popup, not an HTML option.
      await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height + 39);
      await page.waitForTimeout(150);
      await shoot(page, join(out, `pointer-native-hover-${width}.png`), {full: false});
      await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height + 39);
      await settle(page, watch);
      const pointerSelected = await group.inputValue() === 'type';
      // Headless CDP may paint the native popup without routing pointer events
      // into its separate widget. Record that limit, then verify real keys.
      if (!pointerSelected) await choose(group, 'type');
      assert.equal(await group.inputValue(), 'type');
      const expected = layoutDeck(before, 'type', 'curve').groups.map(group => group.cards.map(card => card.oracle_id)).filter(ids => ids.length);
      const actual = await page.locator('[data-deck-list] section').evaluateAll(els => els.map(el => [...el.querySelectorAll(':scope > [data-oracle-id]')].map(row => row.dataset.oracleId)).filter(ids => ids.length));
      assert.deepEqual(actual.slice(1, expected.length + 1), expected);
      await page.setViewportSize({width: width + 1, height: vp.height}); await page.setViewportSize({width, height: vp.height});
      await capture('pointer-selection'); results.push({width, nativePopupPointerSelection: pointerSelected, keyboardFallback: !pointerSelected, groupOutcomeVerified: true}); await context.close(); continue;
    }
    if (process.argv.includes('--narrow-only')) {
      await page.goto(origin + '/decks'); await settle(page, watch);
      await choose(combo('Lifecycle'), 'paper'); assert.equal(await combo('Lifecycle').evaluate(el => el.getBoundingClientRect().height), 44);
      await capture('narrow-lifecycle');
      await page.getByRole('button', {name: 'New deck', exact: true}).click();
      const dialog = page.getByRole('dialog', {name: 'New deck', exact: true});
      await choose(dialog.getByRole('combobox', {name: 'Lifecycle', exact: true}), 'paper'); await capture('narrow-dialog');
      await dialog.getByRole('button', {name: 'Cancel', exact: true}).click();
      await page.goto(origin + '/settings'); await settle(page, watch); await choose(combo('Currency'), 'EUR'); await choose(combo('Store'), 'cardmarket'); await capture('narrow-pricing');
      await page.goto(origin + '/deck/2'); await settle(page, watch); await choose(combo('Group the list'), 'type'); await choose(combo('Order the cards in each group'), 'price');
      await capture('narrow-toolbar'); results.push({width, narrowFunctional: true, touchSelectHeight: 44}); await context.close(); continue;
    }
    async function capture(name) {
      await settle(page, watch);
      await shoot(page, join(out, `${name}-${width}.png`), { full: false });
      const selects = await metrics(page), violations = await axe(page);
      results.push({ name, width, selects, axe: violations, errors: [...watch.errors], overflow: await page.evaluate(() => document.documentElement.scrollWidth - innerWidth) });
      writeFileSync(join(out, 'qa.json'), JSON.stringify({ auditOnly, results }, null, 2) + '\n');
      console.log(`${width} ${name}`);
      if (!auditOnly) {
        assert.deepEqual(watch.errors, []);
        assert.equal(results.at(-1).overflow, 0);
        if (name === 'design' || name.startsWith('select-')) {
          // The baseline gallery repeats headings and table landmarks. Do not
          // hide new violations behind an exemption for the entire gallery.
          assert.deepEqual(violations.map(item => [item.id, item.nodes]), [['heading-order', 2], ['landmark-unique', 2]]);
        } else assert.deepEqual(violations, []);
        for (const control of selects.filter(control => control.y >= 0 && control.y < vp.height)) {
          assert(control.textWidth <= control.available + 1, `${name}: selected text fits ${control.label}: ${control.text}`);
          assert(control.height >= 26);
          assert(control.icon.x >= control.x + control.width - parseFloat(control.paddingRight));
        }
      }
    }
    if (process.argv.includes('--playtest-details-only')) {
      const source = (await api('/api/decks/3')).state;
      await page.goto(origin + '/deck/3/playtest'); await settle(page, watch);
      if (!await page.getByRole('dialog', { name: 'New game', exact: true }).count()) {
        await page.getByRole('button', { name: 'More', exact: true }).click(); await page.getByRole('menuitem', { name: /^New game/ }).click();
      }
      await page.getByRole('button', { name: 'Shuffle and draw seven', exact: true }).click(); await settle(page, watch);
      await page.getByRole('button', { name: 'Keep hand', exact: true }).click(); await settle(page, watch);
      await page.getByRole('button', { name: /^Library\b/ }).first().click(); await settle(page, watch);
      await choose(combo('Library viewer mode'), 'search');
      const playtest = (await api('/api/decks/3/playtest')).playtest;
      const target = Object.values(playtest.session.frame.objects).find(object => playtest.session.frame.zones.library.includes(object.id) && playtest.session.cards[object.oracle_id].faces.length > 1);
      assert(target, 'A real multi-face library card exists in the seeded deck');
      const card = playtest.session.cards[target.oracle_id];
      await page.getByRole('textbox', { name: 'Search Library', exact: true }).fill(card.name);
      const viewer = page.getByRole('region', { name: 'Library viewer', exact: true });
      await viewer.getByRole('button', { name: card.name, exact: true }).click(); await settle(page, watch);
      await choose(combo('Card face'), '1', 'card-face-native-menu');
      assert.equal((await api('/api/decks/3/playtest')).playtest.session.frame.objects[target.id].face, 1);
      await capture('playtest-card-face');
      const item = viewer.locator('[data-object-id="' + target.id + '"]'); await item.focus(); await item.press('Shift+F10');
      await page.getByRole('menuitem', { name: 'Move to battlefield', exact: true }).click(); await settle(page, watch);
      await page.getByRole('button', { name: 'Close viewer', exact: true }).click(); await settle(page, watch);
      const commanderId = playtest.session.frame.zones.command[0];
      const commander = page.locator('[data-object-id="' + commanderId + '"]').first(); await commander.focus(); await commander.press('Shift+F10');
      await page.getByRole('menuitem', { name: 'Move to battlefield', exact: true }).click(); await settle(page, watch);
      const edit = page.locator('summary').filter({ hasText: 'Edit card state' }); await scrollUntilVisible(page, edit, vp); await edit.click();
      await choose(combo('Attachment host'), target.id, 'attachment-native-menu');
      assert.equal((await api('/api/decks/3/playtest')).playtest.session.frame.objects[commanderId].attached_to, target.id);
      await capture('playtest-attachment-long'); await choose(combo('Attachment host'), '');
      assert.equal((await api('/api/decks/3/playtest')).playtest.session.frame.objects[commanderId].attached_to, null);
      assert.deepEqual((await api('/api/decks/3')).state.cards, source.cards);
      results.push({width, nativeCardFaceStored: true, nativeAttachmentStoredAndCleared: true, sourceDeckUnchanged: true});
      await context.close(); continue;
    }
    for (const route of ['decks', 'memories', 'settings', 'deck/2', 'design']) {
      await page.goto(origin + '/' + route); await settle(page, watch);
      if (route === 'design') await scrollUntilVisible(page, page.getByRole('combobox', { name: 'Select example', exact: true }), vp);
      await capture(route.replace('/', '-'));
      if (auditOnly && route !== 'design') {
        for (const [index, control] of (await page.getByRole('combobox').all()).entries()) {
          await scrollUntilVisible(page, control, vp); await control.click();
          await shoot(page, join(out, `${route.replace('/', '-')}-native-menu-${index}-${width}.png`), { full: false });
          await page.keyboard.press('Escape');
        }
      }
    }
    await page.goto(origin + '/decks'); await settle(page, watch);
    await page.getByRole('button', { name: 'New deck', exact: true }).click(); await capture('new-deck');
    await page.getByRole('button', { name: 'Cancel', exact: true }).click();
    await page.getByRole('button', { name: 'Import deck', exact: true }).click(); await capture('import');
    if (!auditOnly) {
      let dialog = page.getByRole('dialog', { name: 'Import deck', exact: true });
      await choose(dialog.getByRole('combobox', { name: 'Lifecycle', exact: true }), 'paper', 'import-native-menu');
      await capture('import-paper');
      await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
      await page.getByRole('button', { name: 'New deck', exact: true }).click();
      dialog = page.getByRole('dialog', { name: 'New deck', exact: true });
      await choose(dialog.getByRole('combobox', { name: 'Lifecycle', exact: true }), 'paper');
      await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
      for (const category of ['archived', 'paper', 'theorycrafted', 'all']) {
        await choose(combo('Lifecycle'), category, category === 'archived' ? 'lifecycle-native-menu' : undefined);
        const visibleIds = await page.locator('main ul a[href^="/deck/"]').evaluateAll(els => els.map(el => Number(el.getAttribute('href').split('/')[2])));
        assert.deepEqual(visibleIds.sort((a,b) => a-b), decks.filter(deck => category === 'all' || deck.category === category).map(deck => deck.id).sort((a,b) => a-b));
      }
      await choose(combo('Lifecycle'), 'theorycrafted'); await capture('lifecycle-long');
      await page.goto(origin + '/memories'); await settle(page, watch);
      const draft = page.getByRole('textbox', { name: 'Memory text', exact: true });
      // all and global share the global composer identity by design.
      const composer = await draft.count() ? draft : page.locator('textarea').first();
      await composer.fill('Unsaved native-select scope draft');
      await choose(combo('Scope'), 'global'); assert.equal(await composer.inputValue(), 'Unsaved native-select scope draft');
      await choose(combo('Scope'), 'all'); assert.equal(await composer.inputValue(), 'Unsaved native-select scope draft');
      const longestDeck = decks.filter(deck => deck.category !== 'archived').sort((a,b) => b.name.length - a.name.length)[0];
      await choose(combo('Scope'), longestDeck.id, 'scope-native-menu');
      await page.getByText('Adding to: ' + longestDeck.name, { exact: true }).waitFor();
      await composer.fill('Preserved while scope remains selected');
      await page.setViewportSize({ width: width + 16, height: vp.height }); await page.setViewportSize({ width, height: vp.height });
      assert.equal(await composer.inputValue(), 'Preserved while scope remains selected');
      assert.equal(await combo('Scope').inputValue(), String(longestDeck.id)); await capture('scope-long');
      const archived = decks.find(deck => deck.category === 'archived');
      await choose(combo('Scope'), archived.id); await page.getByText('Archived deck memories are read-only.', { exact: true }).waitFor();
      await capture('scope-archived');
      await page.goto(origin + '/settings'); await settle(page, watch);
      const tokenLabel = page.getByRole('textbox', { name: 'Token label', exact: true }); await tokenLabel.fill('Unsaved connection draft');
      await choose(combo('Connection method'), 'codex', 'connection-native-menu');
      await page.getByLabel('Codex configuration', { exact: true }).waitFor();
      assert.equal(await tokenLabel.inputValue(), 'Unsaved connection draft');
      await choose(combo('Connection method'), 'claude-code');
      await capture('connection');
      await choose(combo('Currency'), 'EUR'); assert.equal(await combo('Store').inputValue(), 'lowest');
      await choose(combo('Store'), 'cardmarket');
      await page.getByRole('button', { name: 'Save price source', exact: true }).click(); await settle(page, watch);
      assert.deepEqual((await api('/api/settings')).price_source, { currency: 'EUR', store: 'cardmarket' });
      await page.reload(); await settle(page, watch); assert.equal(await combo('Currency').inputValue(), 'EUR'); assert.equal(await combo('Store').inputValue(), 'cardmarket');
      await choose(combo('Store'), 'lowest'); await capture('pricing');
      await choose(combo('Currency'), settings.price_source.currency); await choose(combo('Store'), settings.price_source.store);
      await page.getByRole('button', { name: 'Save price source', exact: true }).click(); await settle(page, watch);
      assert.deepEqual((await api('/api/settings')).price_source, settings.price_source);
      await page.goto(origin + '/deck/2'); await settle(page, watch);
      const group = combo('Group the list'), sort = combo('Order the cards in each group');
      for (const groupBy of ['type', 'slot']) for (const sortBy of ['price', 'name', 'curve']) {
        await choose(group, groupBy, groupBy === 'type' && sortBy === 'price' ? 'group-native-menu' : undefined);
        await choose(sort, sortBy);
        const layout = layoutDeck(before, groupBy, sortBy);
        const actual = await page.locator('[data-deck-list] section').evaluateAll(els => els.map(el => [...el.querySelectorAll(':scope > [data-oracle-id]')].map(row => row.dataset.oracleId)).filter(ids => ids.length));
        const expected = [layout.commanders, layout.companions, ...layout.groups.map(group => group.cards)].filter(cards => cards.length).map(cards => cards.map(card => card.oracle_id));
        assert.deepEqual(actual.slice(0, expected.length), expected, 'Grouping and sorting affect actual rendered rows');
      }
      const search = page.getByRole('searchbox', { name: 'Search this deck', exact: true });
      await search.fill(before.cards.find(card => !card.maybe && card.role === 'card').name); await settle(page, watch);
      const matches = await page.locator('[data-deck-list] [data-filter-match="true"]').count(); assert(matches > 0);
      await choose(sort, 'price'); assert.equal(await page.locator('[data-deck-list] [data-filter-match="true"]').count(), matches);
      await search.fill(''); await choose(sort, 'curve');
      const resize = page.getByRole('separator', { name: 'Resize the deck panel', exact: true });
      const panelWidths = [];
      for (const size of [340, 392, 640]) {
        if (size === 392) await resize.dblclick(); else { await resize.focus(); await resize.press(size === 340 ? 'Home' : 'End'); }
        await settle(page, watch);
        const actualWidth = await page.locator('[data-deck-panel]').evaluate(el => el.getBoundingClientRect().width);
        assert.equal(actualWidth, size); panelWidths.push(actualWidth);
        const links = page.getByRole('navigation', { name: 'Deck panel views' });
        const tops = await links.getByRole('button').evaluateAll(els => els.map(el => el.getBoundingClientRect().y)); assert.equal(new Set(tops).size, 1);
        await capture('editor-panel-' + size);
        const card = before.cards.find(card => !card.maybe && card.role === 'card');
        const row = page.locator('[data-deck-list] [data-oracle-id="' + card.oracle_id + '"]').first();
        await scrollUntilVisible(page, row, vp); await row.click(); await settle(page, watch);
        const primary = combo('Primary slot'), role = combo('Role');
        await capture('card-before-' + size);
        if (size === 340) {
          const otherSlot = before.slots.find(slot => slot.id !== card.slot_id).id;
          await choose(primary, otherSlot);
          assert.equal((await api('/api/decks/2')).state.cards.find(item => item.oracle_id === card.oracle_id).slot_id, otherSlot);
        }
        await choose(primary, card.slot_id ?? ''); await choose(role, card.role);
        await capture('card-panel-' + size);
        await page.getByRole('button', { name: /^Back to/ }).click();
      }
      await page.getByRole('navigation', { name: 'Deck panel views' }).getByRole('button', { name: 'Power', exact: true }).click();
      await page.getByRole('button', { name: 'Assess deck', exact: true }).click(); await settle(page, watch);
      dialog = page.getByRole('dialog', { name: 'Assess the current deck', exact: true });
      for (const control of await dialog.getByRole('combobox').all()) {
        const longest = await control.locator('option').evaluateAll(els => [...els].sort((a,b) => b.textContent.length - a.textContent.length)[0].value);
        await choose(control, longest);
      }
      await capture('power-editor'); await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
      await page.getByRole('button', { name: 'More', exact: true }).click(); await page.getByRole('menuitem', { name: 'Compare', exact: true }).click();
      const compareWith = combo('Compare with'); await choose(compareWith, longestDeck.id, 'compare-native-menu'); await capture('compare-long');
      await page.getByRole('dialog', { name: 'Compare decks', exact: true }).getByRole('button', { name: 'Close', exact: true }).first().click();
      await page.goto(origin + '/design'); await settle(page, watch);
      const example = combo('Select example'); await scrollUntilVisible(page, example, vp, { margin: 220 });
      await choose(example, 'a'); await example.hover(); await capture('select-hover');
      const hoverBorder = await example.evaluate(el => getComputedStyle(el).borderColor);
      await example.press('Tab'); assert.equal(await combo('Required format').evaluate(el => document.activeElement === el), true, 'Tab skips disabled select');
      const invalid = combo('Required format'); assert.equal(await invalid.getAttribute('aria-invalid'), 'true');
      const invalidFocus = await invalid.evaluate(el => ({border: getComputedStyle(el).borderColor, shadow: getComputedStyle(el).boxShadow}));
      assert.notEqual(invalidFocus.border, hoverBorder); assert.notEqual(invalidFocus.shadow, 'none');
      await capture('select-invalid-focus'); await choose(invalid, 'commander'); assert.notEqual(await invalid.getAttribute('aria-invalid'), 'true');
      assert.equal(await combo('Disabled select example').isDisabled(), true);
      assert.equal(await combo('Disabled select example').evaluate(el => getComputedStyle(el.nextElementSibling).opacity), '0.5');
      await capture('design');
      await page.goto(origin + '/deck/2/playtest'); await settle(page, watch);
      if (!await page.getByRole('dialog', { name: 'New game', exact: true }).count()) {
        await page.getByRole('button', { name: 'More', exact: true }).click(); await page.getByRole('menuitem', { name: /^New game/ }).click();
      }
      await page.getByRole('button', { name: 'Shuffle and draw seven', exact: true }).click(); await settle(page, watch);
      await page.getByRole('button', { name: 'Keep hand', exact: true }).click(); await settle(page, watch);
      const libraryButton = page.getByRole('button', { name: /^Library\b/ }).first(); await libraryButton.click(); await settle(page, watch);
      const mode = combo('Library viewer mode'); await choose(mode, 'search', 'playtest-native-menu');
      assert.equal(await page.getByRole('spinbutton', { name: 'Top cards to reveal', exact: true }).count(), 0);
      await choose(mode, 'top'); await page.getByRole('spinbutton', { name: 'Top cards to reveal', exact: true }).waitFor();
      await capture('playtest-library');
      results.push({ width, panelWidths, actualInput: true, lifecycleOutcomes: true, groupSortFilterOutcomes: true, memoriesScopeAndDraftStable: true, priceSourceSavedReloadedRestored: true, connectionDraftStable: true, cardValuesPreserved: true, powerDraftCancelled: true, compareSelection: true, disabledAndInvalidKeyboard: true, libraryModes: true });
    }
    await context.close();
  }
  if (!auditOnly) {
    const after = (await api('/api/decks/2')).state;
    assert.deepEqual(after.cards, before.cards);
  }
  writeFileSync(join(out, 'qa.json'), JSON.stringify({ auditOnly, results, cardAssignmentsAndMembershipRestored: true, priceSourceRestored: true, nativeMenuVisualAssessment: 'Inspect the attempted native-menu screenshots; computed styles alone cannot establish popup rendering.' }, null, 2) + '\n');
} finally { await browser.close(); }
