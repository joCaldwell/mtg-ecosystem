// Run against browser-checks/serve.ts --port 8871. No writes to the fixture deck.
// PLAYWRIGHT_MODULE may name an installed Playwright index.mjs; no app dependency.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE ?? "playwright");
const origin = process.env.UX_ORIGIN ?? "http://127.0.0.1:8871";
const axe = await readFile(new URL("../browser-checks/node_modules/axe-core/axe.min.js", import.meta.url), "utf8");
const browser = await chromium.launch({ headless: true });
const rows = 'button[class*="_row_"]';

async function wheelTo(page, locator) {
  for (let i = 0; i < 30; i++) {
    const box = await locator.boundingBox();
    assert.ok(box, "the target exists in the unchanged list");
    if (box.y >= 60 && box.y + box.height <= 880) return;
    await page.mouse.move(box.x + Math.min(box.width / 2, 100), 700);
    await page.mouse.wheel(0, box.y < 60 ? -500 : 500);
    await page.waitForTimeout(60);
  }
  assert.fail("wheel input did not reveal the target");
}

async function audit(page) {
  await page.evaluate(axe);
  const violations = await page.evaluate(async () => (await window.axe.run(document, { resultTypes: ["violations"] })).violations);
  assert.deepEqual(violations.map(v => ({ id: v.id, nodes: v.nodes.map(n => n.target) })), []);
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false, "no page overflow");
}

try {
  for (const width of [375, 768, 1024, 1280]) {
    const context = await browser.newContext({ viewport: { width, height: 900 }, hasTouch: width < 1100, reducedMotion: "reduce" });
    await context.addCookies([{ name: "checks_user", value: "1", url: origin }]);
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", error => errors.push(error.message));
    await page.goto(`${origin}/deck/2`);
    await page.locator(rows).first().waitFor();
    const { state } = await (await context.request.get(`${origin}/api/decks/2`)).json();
    const first = await page.locator(rows).first().boundingBox();
    assert.equal(first.height, width < 1100 ? 44 : 24);
    if (width === 768 || width === 1024) assert.ok(first.y < 500, `${width}: cards start in the upper 500px (${first.y})`);
    await audit(page);

    if (width === 375) {
      const read = page.getByRole("link", { name: `Read deck · ${state.computed.card_count} cards`, exact: true });
      assert.ok((await read.boundingBox()).y < 500, "the shortcut is visible under the title");
      await read.click();
      assert.ok((await page.locator('#deck-list').boundingBox()).y >= 0);
      assert.ok((await page.locator('#deck-list').boundingBox()).y < 100);
      const review = page.getByRole("link", { name: "Review proposals", exact: true });
      await review.click();
      assert.ok((await page.locator('#deck-proposals').boundingBox()).y < 100);
      const before = await page.evaluate(() => scrollY);
      await page.mouse.move(180, 700);
      await page.mouse.wheel(0, 350);
      await page.waitForTimeout(100);
      assert.ok(await page.evaluate(y => scrollY > y, before), "phone window scroll responds to wheel input");
      assert.equal(await page.getByRole("button", { name: "Edit the thesis" }).count(), 0);
      // DESIGN.md: deck-wide actions follow the individual phone decisions
      // inside Review all decisions, rather than competing above them.
      const all = page.locator("summary").filter({ hasText: /^Review all decisions/ });
      assert.equal(await all.locator("..").getAttribute("open"), null);
      assert.equal(await page.getByRole("button", { name: "Accept all", exact: true }).count(), 0);
      await all.click();
      assert.equal(await page.getByRole("button", { name: "Accept all", exact: true }).count(), 1);
      assert.equal(await page.getByRole("button", { name: "Reject all", exact: true }).count(), 1);
      assert.equal(await page.getByRole("textbox", { name: "Reason for accepting or rejecting everything, optional", exact: true }).count(), 1);
    } else {
      if (state.computed.card_count !== 100) {
        // The compact header also has a stats/action row in the DOM. Select
        // the accessible card-count control, not that hidden row's first button.
        assert.equal(await page.getByRole('button', { name: `${state.computed.card_count} / 100 cards`, exact: true }).evaluate(e => e.className.includes("_warn_")), true, "off-target card count is amber");
      }
      const numericSizes = await page.locator('[class*="_price_"], [class*="_count_"], [class*="_figure_"]').evaluateAll(elements => elements.map(e => parseFloat(getComputedStyle(e).fontSize)));
      assert.ok(numericSizes.length > 0 && numericSizes.every(size => size >= 12), "numeric metadata is at least 12px");
      const count = await page.locator(rows).count();
      const groups = await page.locator('[data-deck-list] h2').allTextContents();
      const target = await page.locator(rows).last().locator('[class*="_name_"] > span').first().innerText();
      assert.ok(state.cards.some(c => c.name === target), "search query comes from stored card facts");
      const search = page.getByRole("searchbox", { name: "Search this deck" });
      await search.fill(target);
      await page.waitForTimeout(100);
      const match = page.locator('[data-deck-list] [data-filter-match="true"]').first();
      const box = await match.boundingBox();
      assert.ok(box.y >= 0 && box.y + box.height <= 900, "query changes reveal the first match");
      assert.equal(await search.evaluate(e => document.activeElement === e), true, "search keeps focus");
      const shown = await page.locator(rows).count();
      assert.ok(shown > 0 && shown < count, "nonmatching rows are hidden");
      assert.equal(await page.locator(`[data-deck-list] ${rows}:not([data-filter-match="true"])`).count(), 0, "every row shown is a match");
      assert.ok((await page.locator('[data-deck-list] h2').count()) < groups.length, "groups without matches are hidden");
      const status = page.getByRole('group', { name: 'Active deck filters' }).getByRole('status');
      assert.match(await status.innerText(), /^\d+ match(es)?$/);
      await audit(page);
      // Wheel back to the controls: never use script scrolling in QA.
      await wheelTo(page, search);
      await search.fill("no-such-deck-card");
      assert.equal(await status.innerText(), "No matches");
      assert.equal(await page.locator(`[data-deck-list] ${rows}`).count(), 0);
      assert.equal(await page.getByText("No cards in this deck match “no-such-deck-card”.").count(), 1);
      await search.press("Escape");
      assert.equal(await page.locator('[data-filter-match]').count(), 0);
      assert.equal(await page.locator(rows).count(), count, "clearing restores every row");

      if (width < 1100) {
        const before = await page.evaluate(() => scrollY);
        await page.mouse.move(width / 2, 700);
        await page.mouse.wheel(0, 250);
        await page.waitForTimeout(100);
        assert.ok(await page.evaluate(y => scrollY > y, before), "tablet window responds to wheel input");
        const row = page.locator(rows).first();
        await wheelTo(page, row);
        const hit = await row.boundingBox();
        // Tap the right edge (the price), not just the name.
        await page.touchscreen.tap(hit.x + hit.width - 12, hit.y + hit.height / 2);
        await page.getByRole("dialog").waitFor();
        await audit(page);
        await page.keyboard.press("Escape");
      } else {
        // Keep DOM identity and an unsaved brief draft across the panel boundary.
        await page.locator(rows).first().evaluate(e => { window.qaRow = e; });
        await page.getByRole("button", { name: "Edit the thesis" }).click();
        const thesis = page.getByRole("textbox", { name: "Thesis", exact: true });
        await thesis.fill("Unsaved rotation check");
        await page.setViewportSize({ width: 1024, height: 900 });
        assert.equal(await page.evaluate(() => window.qaRow === document.querySelector('button[class*="_row_"]')), true);
        await page.getByRole("button", { name: "Deck details", exact: true }).click();
        assert.equal(await thesis.inputValue(), "Unsaved rotation check");
        await page.setViewportSize({ width: 1280, height: 900 });
        assert.equal(await thesis.inputValue(), "Unsaved rotation check");
        assert.equal(await page.evaluate(() => window.qaRow === document.querySelector('button[class*="_row_"]')), true);
        await thesis.press("Escape");
      }
    }
    assert.deepEqual(errors, []);
    console.log(`${width}: owner deck passed; first card ${Math.round(first.y)}px, row ${first.height}px; axe 0`);

    const { state: shared } = await (await context.request.get(`${origin}/api/decks/4`)).json();
    // Removing cookies while the private app is mounted races its background
    // requests and 401 sign-in navigation against the public-page navigation.
    const publicContext = await browser.newContext({ viewport: { width, height: 900 }, hasTouch: width < 1100, reducedMotion: "reduce" });
    const publicPage = await publicContext.newPage();
    publicPage.on("pageerror", error => errors.push(error.message));
    await publicPage.goto(`${origin}/d/${shared.deck.slug}`);
    await publicPage.locator(rows).first().waitFor();
    assert.ok((await publicPage.locator(rows).first().boundingBox()).height >= (width < 1100 ? 44 : 24));
    await audit(publicPage);
    assert.deepEqual(errors, []);
    console.log(`${width}: signed-out public deck passed; axe 0`);
    await publicContext.close();
    await context.close();
  }
} finally {
  await browser.close();
}
