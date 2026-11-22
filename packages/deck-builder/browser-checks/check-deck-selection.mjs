#!/usr/bin/env node
// Real-input regression for stable card and sidebar coordinates on the owner deck.
import assert from "node:assert/strict";
import { join } from "node:path";
import { chromium } from "playwright-core";
import { BROWSER_ARGS, VIEWPORTS, newContext, settle, shoot, watchPage } from "./lib/browser.mjs";
import { CHECKS_ROOT } from "./lib/paths.mjs";

const option = (name) => {
  const index = process.argv.indexOf(`--${name}`);
  return index < 0 ? undefined : process.argv[index + 1];
};
const origin = option("origin") ?? "http://127.0.0.1:8830";
const output = option("out") ?? join(CHECKS_ROOT, "manual", "deck-selection");
assert.equal(new URL(origin).hostname, "127.0.0.1", "disposable fixture only");
const response = await fetch(`${origin}/api/decks/2`, { headers: { Cookie: "checks_user=1" } });
assert.equal(response.status, 200);
const { state } = await response.json();
const commander = state.cards.find((card) => !card.maybe && card.role === "commander");
assert(commander, "fixture has a commander");

const browser = await chromium.launch({ args: BROWSER_ARGS });
try {
  for (const width of [1440, 768]) {
    const vp = VIEWPORTS[width];
    const context = await newContext(browser, {
      origin, vp, user: 1, frozenNow: "2026-10-20T17:00:00Z",
      notes: { blocked: new Set(), imageFailures: new Set() },
    });
    const page = await context.newPage();
    const watch = watchPage(page);
    await page.goto(`${origin}/deck/2`);
    const list = page.getByRole("region", { name: "Deck list", exact: true });
    const row = list.locator(`button[data-oracle-id="${commander.oracle_id}"]`).first();
    const another = list.locator(`button[data-oracle-id]:not([data-oracle-id="${commander.oracle_id}"])`).first();
    await row.waitFor();
    await settle(page, watch);

    const actions = page.getByRole("group", { name: "Selected cards", exact: true });
    const checkboxes = list.locator('input[type="checkbox"]');
    const isSelected = async (cardRow) => (await cardRow.getAttribute("data-selected")) === "true";
    assert.equal(await list.getByRole("heading", { name: "Main deck", exact: true }).count(), 0);
    assert.equal(await checkboxes.count(), 0);
    assert.equal(await actions.count(), 0);

    if (vp.touch) {
      await row.tap();
      const details = page.getByRole("dialog");
      await details.getByRole("heading", { name: commander.name, exact: true }).waitFor();
      const before = await row.boundingBox();
      await details.getByRole("button", { name: "Select card", exact: true }).tap();
      await details.getByRole("button", { name: "Close", exact: true }).tap();
      await details.waitFor({ state: "hidden" });
      assert.deepEqual(await row.boundingBox(), before);
      assert.equal(await isSelected(row), true);
      assert(await actions.evaluate((element) => Math.abs(element.getBoundingClientRect().bottom - innerHeight) < 1));
      await shoot(page, join(output, "tablet-selected.png"), { full: false });
      await another.tap({ position: { x: 32, y: 10 } });
      assert.equal(await details.count(), 0, "touch can add to an active selection directly");
    } else {
      await row.click({ position: { x: 32, y: 10 } });
      const panel = page.getByRole("complementary", { name: "Deck panel" });
      await panel.getByRole("heading", { name: commander.name, exact: true }).waitFor();
      assert.equal(await row.getAttribute("aria-current"), "true");
      assert.equal(await isSelected(row), false);
      const openFill = await row.evaluate((element) => getComputedStyle(element).backgroundColor);
      await shoot(page, join(output, "desktop-inspection.png"), { full: false });
      await panel.getByRole("button", { name: /^Back/ }).click();
      const before = await row.boundingBox();
      assert(before);
      const point = { x: before.x + 32, y: before.y + 10 };
      await page.mouse.click(point.x, point.y, { button: "right" });
      await actions.waitFor();
      assert.equal(await row.getAttribute("aria-current"), null);
      assert.deepEqual(await row.boundingBox(), before);
      assert.equal(await row.evaluate((element, p) => element.contains(document.elementFromPoint(p.x, p.y)), point), true);
      assert.equal(await isSelected(row), true);
      assert.equal(await row.getAttribute("aria-description"), "Selected for bulk actions");
      const selectionStyle = await row.evaluate((element) => {
        const { backgroundColor, outlineStyle } = getComputedStyle(element);
        return { backgroundColor, outlineStyle };
      });
      assert.notEqual(selectionStyle.backgroundColor, openFill);
      assert.equal(selectionStyle.outlineStyle, "solid");
      const listBounds = await list.boundingBox();
      const actionBounds = await actions.boundingBox();
      assert(actionBounds && listBounds && actionBounds.y >= listBounds.y + listBounds.height);
      await shoot(page, join(output, "desktop-selected.png"), { full: false });
      await row.click({ position: { x: 32, y: 10 } });
      assert.equal(await row.getAttribute("aria-current"), "true");
      assert.equal(await isSelected(row), true);
      assert.equal(await row.evaluate((element) => getComputedStyle(element).backgroundColor), openFill);
      await panel.getByRole("button", { name: /^Back/ }).click();
      await another.click({ modifiers: ["Control"], position: { x: 32, y: 10 } });
    }

    assert.equal(await checkboxes.count(), 0);
    assert.equal(await isSelected(another), true);
    assert.equal(await actions.getByRole("status").textContent(), "2 selected");
    await actions.getByRole("button", { name: /^Clear/ }).click();
    assert.equal(await list.locator("[data-selected]").count(), 0);

    if (!vp.touch) {
      await row.press("Shift+Space");
      assert.equal(await isSelected(row), true);
      await row.press("Shift+Space");
      assert.equal(await isSelected(row), false);
      assert.equal(await row.evaluate((element) => element === document.activeElement), true);

      const collapse = page.getByRole("button", { name: "Collapse sidebar", exact: true });
      const expand = page.getByRole("button", { name: "Expand sidebar", exact: true });
      await collapse.focus();
      const before = await collapse.boundingBox();
      await collapse.press("Enter");
      await expand.waitFor();
      assert.deepEqual(await expand.boundingBox(), before);
      assert.equal(await expand.evaluate((element) => element === document.activeElement), true);
      await shoot(page, join(output, "desktop-collapsed.png"), { full: false });
      await expand.press("Enter");
      await collapse.waitFor();
      assert.deepEqual(await collapse.boundingBox(), before);
    }
    assert.deepEqual(watch.errors, []);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    await context.close();
    console.log(`Deck selection passed at ${width}px`);
  }
} finally {
  await browser.close();
}
