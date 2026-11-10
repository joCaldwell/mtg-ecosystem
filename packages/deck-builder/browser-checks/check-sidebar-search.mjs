#!/usr/bin/env node
// Desktop navigation checks on serve.ts's expanded disposable collection.
import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "playwright-core";
import { CHECKS_ROOT } from "./lib/paths.mjs";
import { BROWSER_ARGS, VIEWPORTS, newContext, watchPage, settle, axe, shoot } from "./lib/browser.mjs";

const option = name => { const i = process.argv.indexOf(`--${name}`); return i < 0 ? undefined : process.argv[i + 1]; };
const origin = option("origin") ?? "http://127.0.0.1:8853";
assert.equal(new URL(origin).hostname, "127.0.0.1", "fixture server only");
const output = option("out") ?? join(CHECKS_ROOT, "manual", "sidebar-search");
mkdirSync(output, { recursive: true });
const api = async path => {
  const response = await fetch(origin + path, { headers: { Cookie: "checks_user=1" } });
  assert.equal(response.status, 200); return response.json();
};
const decks = await api("/api/decks");
assert(decks.length > 8, "use the expanded collection fixture");
const before = (await api("/api/decks/2")).state;
const browser = await chromium.launch({ args: BROWSER_ARGS }), results = [];
try {
  for (const width of process.argv.includes("--scoped-only") ? [] : [1280, 1440, 1920, 2560]) {
    const context = await newContext(browser, { origin, vp: VIEWPORTS[width], user: 1, frozenNow: "2026-10-20T17:00:00Z", notes: { blocked: new Set(), imageFailures: new Set() } });
    const page = await context.newPage(), watch = watchPage(page);
    await page.goto(origin + "/"); await settle(page, watch);
    assert.equal(await page.evaluate(() => document.documentElement.scrollHeight <= innerHeight + 1), true, "a long sidebar never expands the desktop document");
    const contentScroll = page.locator('main > div > div[class*="_scroll_"]');
    const contentBox = await contentScroll.boundingBox(); assert(contentBox);
    await page.mouse.move(contentBox.x + contentBox.width / 2, contentBox.y + contentBox.height / 2);
    await page.mouse.wheel(0, 260); await settle(page, watch);
    assert(await contentScroll.evaluate(node => node.scrollTop) > 0, "collection owns its scroll");
    assert.equal(await page.evaluate(() => scrollY), 0, "desktop window stays pinned");
    await page.goto(origin + "/deck/2"); await settle(page, watch);
    const sidebar = page.getByRole("complementary", { name: "Sidebar", exact: true });
    const nav = sidebar.getByRole("navigation", { name: "Decks", exact: true });
    const input = sidebar.getByRole("searchbox", { name: "Find a deck or commander in sidebar", exact: true });
    const ids = async () => (await nav.getByRole("link").evaluateAll(nodes => nodes.map(node => Number(node.getAttribute("href").split("/").at(-1))))).sort((a, b) => a - b);
    assert.equal(await nav.getByRole("link").count(), decks.length);
    const lifecycle = { theorycrafted: "Theorycrafted", archived: "Archived" };
    for (const deck of decks.filter(deck => lifecycle[deck.category])) {
      const link = nav.locator(`a[href="/deck/${deck.id}"]`);
      const label = link.locator("[data-lifecycle]");
      assert.equal(await label.textContent(), lifecycle[deck.category]);
      assert.equal(await label.isVisible(), true);
      assert(await label.evaluate(node => parseFloat(getComputedStyle(node).fontSize)) >= 12);
      assert((await link.getAttribute("title")).includes(lifecycle[deck.category]));
    }
    const currentBounds = await nav.locator('a[aria-current="page"]').boundingBox(), navBounds = await nav.boundingBox();
    assert(currentBounds && navBounds && currentBounds.y >= navBounds.y - 1 && currentBounds.y + currentBounds.height <= navBounds.y + navBounds.height + 1, "initial navigation reveals the current deck inside its own scroller");

    const names = await nav.locator('span[class*="_name_"]').evaluateAll(nodes => nodes.map(node => ({ name: node.textContent, clipped: node.scrollWidth > node.clientWidth + 1 || node.scrollHeight > node.clientHeight + 1 })));
    assert(names.every(name => !name.clipped), JSON.stringify(names.filter(name => name.clipped)));
    const rowLayout = await nav.getByRole("link").evaluateAll(nodes => nodes.map(node => {
      const name = node.querySelector('span[class*="_name_"]').getBoundingClientRect();
      const lifecycle = node.querySelector('span[class*="_lifecycle_"]')?.getBoundingClientRect();
      const row = node.getBoundingClientRect();
      return { label: node.textContent, contained: name.top >= row.top && name.bottom <= row.bottom && (!lifecycle || lifecycle.bottom <= row.bottom + 1), separate: !lifecycle || lifecycle.top >= name.bottom - 1 };
    }));
    assert(rowLayout.every(row => row.contained && row.separate), JSON.stringify(rowLayout.filter(row => !row.contained || !row.separate)));

    await shoot(page, join(output, `default-${width}.png`), { full: false });

    const panel = page.getByRole("complementary", { name: "Deck panel", exact: true });
    await panel.getByRole("button", { name: "Edit the thesis", exact: true }).click();
    const draft = panel.getByRole("textbox", { name: "Thesis", exact: true });
    await draft.fill("Unsaved plan while finding a different deck");
    await page.evaluate(() => { window.__sidebarDraft = document.querySelector('textarea[aria-label="Thesis"]'); });
    const chosen = decks.find(deck => deck.id !== 2 && deck.commanders.length > 0);
    assert(chosen);
    for (const query of [` ${chosen.name.toLocaleUpperCase()} `, chosen.commanders[0]]) {
      await input.fill(query); await settle(page, watch);
      const needle = query.trim().toLocaleLowerCase();
      const expected = decks.filter(deck => [deck.name, ...deck.commanders].some(name => name.toLocaleLowerCase().includes(needle))).map(deck => deck.id).sort((a, b) => a - b);
      assert.deepEqual(await ids(), expected, "name/commander query shows exactly the matching decks");
      assert.equal(await draft.inputValue(), "Unsaved plan while finding a different deck");
      assert.equal(await page.evaluate(() => window.__sidebarDraft === document.querySelector('textarea[aria-label="Thesis"]')), true, "filtering preserves the live editor");
    }
    await shoot(page, join(output, `commander-${width}.png`), { full: false });
    await input.press("Escape"); await settle(page, watch);
    assert.equal(await input.inputValue(), ""); assert.equal(await nav.getByRole("link").count(), decks.length);
    await input.fill("no matching fixture deck 71"); await settle(page, watch);
    assert.equal(await nav.getByRole("link").count(), 0);
    assert.equal(await nav.getByText("No decks found", { exact: true }).isVisible(), true);
    await shoot(page, join(output, `empty-${width}.png`), { full: false });
    await page.getByRole("button", { name: "Collapse sidebar", exact: true }).click(); await settle(page, watch);
    assert.equal(await input.count(), 0); assert.equal(await nav.getByRole("link").count(), decks.length, "hidden filtering never removes compact-rail destinations");
    for (const deck of decks.filter(deck => lifecycle[deck.category])) {
      const link = nav.locator(`a[href="/deck/${deck.id}"]`);
      assert((await link.getAttribute("aria-label")).includes(lifecycle[deck.category]), "collapsed links retain the lifecycle in their accessible names");
      assert.equal(await link.locator("[data-lifecycle]").isVisible(), false);
    }
    await shoot(page, join(output, `collapsed-${width}.png`), { full: false });
    await page.getByRole("button", { name: "Expand sidebar", exact: true }).click(); await settle(page, watch);
    await nav.getByRole("button", { name: "Clear filter", exact: true }).click(); await settle(page, watch);
    assert.equal(await input.evaluate(node => node === document.activeElement), true);
    assert.equal(await nav.getByRole("link").count(), decks.length);
    assert.equal((await axe(page)).length, 0); assert.deepEqual(watch.errors, []);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    await input.fill(chosen.name); await settle(page, watch);
    await input.focus(); await input.press("Tab");
    assert.equal(await nav.locator(`a[href="/deck/${chosen.id}"]`).evaluate(node => node === document.activeElement), true);
    await page.keyboard.press("Enter"); await page.waitForURL(`${origin}/deck/${chosen.id}`); await settle(page, watch);
    assert.equal(await nav.locator(`a[href="/deck/${chosen.id}"]`).getAttribute("aria-current"), "page");
    results.push({ width, deck_count: decks.length, lifecycle_labels_and_collapsed_names: true, full_names_visible: true, name_and_commander_filter: true, draft_dom_preserved: true, empty_and_escape_clear: true, compact_rail_unfiltered: true, keyboard_navigation: true, axe: 0, overflow: 0 });
    await context.close(); console.log(`Sidebar search ${width} passed`);
  }
  // Scoped preservation check: the shared popup/drawer still owns the same
  // live editor after crossing the desktop boundary, without a hidden filter.
  const context = await newContext(browser, { origin, vp: VIEWPORTS[1280], user: 1, frozenNow: "2026-10-20T17:00:00Z", notes: { blocked: new Set(), imageFailures: new Set() } });
  const page = await context.newPage(), watch = watchPage(page);
  await page.goto(origin + "/deck/2"); await settle(page, watch);
  await page.getByRole("button", { name: "Edit the thesis", exact: true }).click();
  await page.getByRole("textbox", { name: "Thesis", exact: true }).fill("Unsaved sidebar resize draft");
  await page.evaluate(() => { window.__sidebarResizeDraft = document.querySelector('textarea[aria-label="Thesis"]'); });
  await page.getByRole("searchbox", { name: "Find a deck or commander in sidebar" }).fill("no matching fixture deck 71");
  await page.setViewportSize({ width: 1024, height: 800 }); await settle(page, watch);
  await page.getByRole("button", { name: "Menu", exact: true }).click(); await settle(page, watch);
  const drawer = page.getByRole("dialog", { name: "Navigation", exact: true });
  assert.equal(await drawer.getByRole("navigation", { name: "Decks", exact: true }).getByRole("link").count(), decks.length);
  assert.equal(await drawer.getByRole("searchbox").count(), 0);
  await page.keyboard.press("Escape"); await settle(page, watch);
  if (await page.getByRole("dialog").count() === 0) await page.getByRole("button", { name: "Open deck details", exact: true }).click();
  await settle(page, watch);
  assert.equal(await page.getByRole("textbox", { name: "Thesis", exact: true }).inputValue(), "Unsaved sidebar resize draft");
  assert.equal(await page.evaluate(() => window.__sidebarResizeDraft === document.querySelector('textarea[aria-label="Thesis"]')), true);
  await page.getByRole("dialog").getByRole("button", { name: "Close", exact: true }).click();
  await page.setViewportSize({ width: 1280, height: 800 }); await settle(page, watch);
  assert.equal(await page.getByRole("textbox", { name: "Thesis", exact: true }).inputValue(), "Unsaved sidebar resize draft");
  await context.close();
  assert.deepEqual((await api("/api/decks/2")).state, before, "navigation and unsaved drafts never change the deck");
  writeFileSync(join(output, "qa.json"), JSON.stringify({ results, drawer_unfiltered: true, shared_resize_draft_preserved: true, deck_unchanged: true }, null, 2) + "\n");
} finally { await browser.close(); }
