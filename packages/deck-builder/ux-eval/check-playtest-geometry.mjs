#!/usr/bin/env node
// Compare host sizing to the established pre-wrapper table, using real input.
import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "playwright-core";
import { UX_EVAL_ROOT } from "./lib/paths.mjs";
import { BROWSER_ARGS, VIEWPORTS, newContext, watchPage, settle, shoot, axe } from "./lib/browser.mjs";
const option = name => { const i = process.argv.indexOf(`--${name}`); return i < 0 ? undefined : process.argv[i + 1]; };
const origins = { baseline: option("baseline") ?? "http://127.0.0.1:8855", candidate: option("origin") ?? "http://127.0.0.1:8853" };
for (const url of Object.values(origins)) assert.equal(new URL(url).hostname, "127.0.0.1", "disposable fixture only");
const output = option("out") ?? join(UX_EVAL_ROOT, "manual", "playtest-host-geometry"); mkdirSync(output, { recursive: true });
const browser = await chromium.launch({ args: BROWSER_ARGS }), results = {};
try {
  for (const [version, origin] of Object.entries(origins)) {
    results[version] = [];
    const api = async path => (await (await fetch(origin + path, { headers: { Cookie: "uxeval_user=1" } })).json());
    const before = (await api("/api/decks/2")).state;
    for (const width of [1280, 1440, 1920, 2560]) {
      const context = await newContext(browser, { origin, vp: VIEWPORTS[width], user: 1, frozenNow: "2026-10-20T17:00:00Z", notes: { blocked: new Set(), imageFailures: new Set() } });
      const page = await context.newPage(), watch = watchPage(page);
      await page.goto(origin + "/deck/2/playtest"); await settle(page, watch);
      if (!await page.getByRole("dialog", { name: "New game", exact: true }).count()) {
        await page.getByRole("button", { name: "More", exact: true }).click();
        await page.getByRole("menuitem", { name: /^New game/ }).click();
      }
      await page.getByRole("button", { name: "Shuffle and draw seven", exact: true }).click(); await settle(page, watch);
      const hand = page.getByRole("region", { name: "Hand", exact: true });
      const cards = hand.locator("[data-object-id]"); assert.equal(await cards.count(), 7);
      const handGeometry = await cards.evaluateAll(nodes => nodes.map(node => { const r = node.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width }; }));
      const rail = page.locator('[class*="_rail_"] [data-object-id]').first();
      const railBox = await rail.boundingBox(); assert(railBox);
      await shoot(page, join(output, `${version}-opening-${width}.png`), { full: false });
      await page.getByRole("button", { name: "Keep hand", exact: true }).click(); await settle(page, watch);
      await cards.first().focus(); await cards.first().press("Enter"); await settle(page, watch);
      assert.equal(await cards.first().getAttribute("data-selected"), "true");
      const inspection = page.getByRole("complementary", { name: "Inspection panel" }).locator('section[aria-label^="Inspecting"]');
      const inspected = inspection.locator("[data-object-id]").first();
      const inspectionBox = await inspection.boundingBox(), cardBox = await inspected.boundingBox(); assert(inspectionBox && cardBox);
      assert(Math.abs(inspectionBox.width - cardBox.width) <= 1, `${version}: inspected card uses its host width`);
      await shoot(page, join(output, `${version}-inspection-${width}.png`), { full: false });
      await cards.first().press("Shift+F10"); await page.getByRole("menu").waitFor(); await page.keyboard.press("Escape");
      await cards.nth(1).hover(); await settle(page, watch);
      const preview = page.locator('[class*="_preview_"]').filter({ has: page.locator("[data-object-id]") }).last();
      const previewBox = await preview.boundingBox(), previewCard = await preview.locator("[data-object-id]").first().boundingBox();
      assert(previewBox && previewCard); assert(Math.abs(previewBox.width - previewCard.width) <= 1, `${version}: hover card uses its host width`);
      if (version === "candidate") assert.equal((await axe(page)).length, 0);
      assert.deepEqual(watch.errors, []); assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
      results[version].push({ width, hand: handGeometry, rail_height: railBox.height, inspected_width: cardBox.width, inspector_width: inspectionBox.width, preview_width: previewCard.width, hand_rows: new Set(handGeometry.map(card => Math.round(card.y))).size, keyboard_selection_and_menu: true });
      await context.close(); console.log(`${version} geometry ${width} passed`);
    }
    const after = (await api("/api/decks/2")).state;
    assert.equal(after.deck.revision, before.deck.revision); assert.deepEqual(after.cards, before.cards);
  }
  for (let i = 0; i < results.baseline.length; i++) {
    const b = results.baseline[i], c = results.candidate[i];
    assert.equal(c.hand_rows, b.hand_rows, `${c.width}: opening hand row count restored`);
    for (let j = 0; j < 7; j++) assert(Math.abs(c.hand[j].width - b.hand[j].width) <= 1, `${c.width}: hand width ${j} restored`);
    assert(Math.abs(c.rail_height - b.rail_height) <= 1, `${c.width}: commander rail height restored`);
    assert(Math.abs(c.inspected_width - b.inspected_width) <= 1);
    assert(Math.abs(c.preview_width - b.preview_width) <= 1);
    if (c.width >= 1920) assert.equal(c.hand_rows, 1, "seven opening cards fit a single row on roomy desktops");
  }
  writeFileSync(join(output, "qa.json"), JSON.stringify({ results, baseline_geometry_restored: true, candidate_axe: 0, deck_unchanged: true }, null, 2) + "\n");
} finally { await browser.close(); }
