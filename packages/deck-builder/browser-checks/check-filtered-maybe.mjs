#!/usr/bin/env node
// Real-input geometry and inspection checks on the long-note disposable fixture.
import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "playwright-core";
import { CHECKS_ROOT } from "./lib/paths.mjs";
import { BROWSER_ARGS, VIEWPORTS, newContext, watchPage, settle, scrollUntilVisible, axe, shoot } from "./lib/browser.mjs";

const opt = name => { const i = process.argv.indexOf(`--${name}`); return i < 0 ? undefined : process.argv[i + 1]; };
const origin = opt("origin") ?? "http://127.0.0.1:8885";
assert.equal(new URL(origin).hostname, "127.0.0.1", "disposable loopback fixture only");
const out = opt("out") ?? join(CHECKS_ROOT, "manual", "filtered-maybe");
mkdirSync(out, { recursive: true });
const fixture = JSON.parse(readFileSync(join(CHECKS_ROOT, "long-note-fixture-v1", "coverage.json")));
const read = async () => (await (await fetch(`${origin}/api/decks/${fixture.deck}`, { headers: { Cookie: "checks_user=1" } })).json()).state;
const before = await read();
const maybeCards = before.cards.filter(card => card.maybe);
const chosen = maybeCards.find(card => card.maybe_note?.includes("\n\n") && card.mana_cost);
const main = before.cards.find(card => !card.maybe && card.role === "card");
assert(chosen && main);
const browser = await chromium.launch({ args: BROWSER_ARGS }), results = [];
try {
  for (const width of [1280, 1440, 1920, 2560]) {
    const context = await newContext(browser, { origin, vp: VIEWPORTS[width], user: 1, frozenNow: "2026-10-20T17:00:00Z", notes: { blocked: new Set(), imageFailures: new Set() } });
    const page = await context.newPage(), watch = watchPage(page);
    await page.goto(`${origin}/deck/${fixture.deck}`); await settle(page, watch);
    const panel = page.getByRole("complementary", { name: "Deck panel", exact: true });
    const list = page.locator('[data-deck-list="true"]');
    const maybe = list.getByRole("region", { name: "Maybe list", exact: true });
    const rows = maybe.locator("[data-oracle-id]");
    const originalOrder = await rows.evaluateAll(nodes => nodes.map(node => node.dataset.oracleId));
    await panel.getByRole("button", { name: "Edit the thesis", exact: true }).click();
    const draft = panel.getByRole("textbox", { name: "Thesis", exact: true });
    await draft.fill("Unsaved plan while finding a shortlist card");
    const mainRow = list.locator(`button[data-oracle-id="${main.oracle_id}"]`).first();
    await scrollUntilVisible(page, mainRow, VIEWPORTS[width]);
    await mainRow.click({ modifiers: ["Control"] }); await settle(page, watch);
    await scrollUntilVisible(page, maybe, VIEWPORTS[width], { top: true });
    const beforeScroll = await list.evaluate(node => node.scrollTop);
    const normalWidth = (await rows.first().boundingBox()).width;
    const search = page.getByRole("searchbox", { name: "Search this deck", exact: true });
    await search.pressSequentially(chosen.name); await settle(page, watch);
    assert.equal(await rows.count(), 1, "nonmatching maybe cards are hidden");
    assert.equal(await list.locator('[data-filter-match]:not(button)').count(), 0, "only actual native row buttons carry match markers");
    assert.equal(await list.locator('button[data-oracle-id]').count(), 0, "nonmatching main-deck rows are hidden");
    const row = rows.first(), hit = row.getByRole("button", { name: chosen.name, exact: true });
    const box = await row.boundingBox(), hitBox = await hit.boundingBox();
    assert(Math.abs(box.width - normalWidth) <= 1, "a single match keeps the original column width");
    const border = await row.evaluate(node => parseFloat(getComputedStyle(node).borderLeftWidth));
    assert(Math.abs(hitBox.width + border - box.width) <= 1 && Math.abs(hitBox.height - box.height) <= 1, "the native button covers the row inside its decorative border");
    assert.equal(await maybe.getByText(`1 of ${maybeCards.length} · outside the 100`, { exact: true }).count(), 1);
    await shoot(page, join(out, `match-${width}.png`), { full: false });
    // The price end of the row is the same pointer target as its name.
    await page.mouse.click(box.x + box.width - 4, box.y + box.height / 2); await settle(page, watch);
    const note = panel.getByRole("region", { name: "Maybe note", exact: true });
    assert.deepEqual(await note.locator("p").allTextContents(), chosen.maybe_note.split(/\n\s*\n/).map(p => p.replace(/\[\[|\]\]/g, "")));
    await shoot(page, join(out, `details-${width}.png`), { full: false });
    await panel.getByRole("button", { name: "Back to Brief", exact: true }).click(); await settle(page, watch);
    assert.equal(await draft.inputValue(), "Unsaved plan while finding a shortlist card");
    await hit.focus(); await hit.press("?"); await settle(page, watch);
    assert.equal(await page.getByRole("dialog").count(), 1);
    await page.keyboard.press("Escape"); await settle(page, watch);
    await hit.press("Space"); await settle(page, watch);
    assert.equal(await panel.getByRole("heading", { name: chosen.name, exact: true }).count(), 1);
    await panel.getByRole("button", { name: "Back to Brief", exact: true }).click();
    await search.focus(); await search.press("Escape"); await settle(page, watch);
    assert.deepEqual(await rows.evaluateAll(nodes => nodes.map(node => node.dataset.oracleId)), originalOrder);
    assert(Math.abs(await list.evaluate(node => node.scrollTop) - beforeScroll) <= 1, "clearing restores the unfiltered scroll position");
    assert.equal(await list.locator(`button[data-oracle-id="${main.oracle_id}"]`).first().evaluate(node => node.className.includes("selected")), true, "selection survives being hidden by search");
    assert.equal((await axe(page)).length, 0); assert.deepEqual(watch.errors, []);
    results.push({ width, normal_row_width: normalWidth, match_row_width: box.width, hidden_nonmatches: true, whole_row_pointer_keyboard: true, full_note_inspect_back_draft: true, order_selection_scroll_preserved: true, axe: 0 });
    await context.close(); console.log(`Filtered Maybe ${width} passed`);
  }
  assert.deepEqual(await read(), before, "search and inspection preserve exact notes, membership, assignments and order");
  writeFileSync(join(out, "qa.json"), JSON.stringify({ results, exact_deck_unchanged: true }, null, 2) + "\n");
} finally { await browser.close(); }
