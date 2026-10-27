#!/usr/bin/env node
// Real-input regression coverage against serve.ts's disposable seed copy.
// Never point this at hosted data. Run with the fixture server already up.
import assert from "node:assert/strict";
import { writeFileSync } from "node:fs";
import { chromium } from "playwright-core";
import { BROWSER_ARGS, VIEWPORTS, newContext, watchPage, settle, scrollUntilVisible, axe } from "./lib/browser.mjs";

const option = name => { const i = process.argv.indexOf(`--${name}`); return i < 0 ? undefined : process.argv[i + 1]; };
const origin = option("origin") ?? "http://127.0.0.1:8853";
assert.equal(new URL(origin).hostname, "127.0.0.1", "only the loopback fixture server is allowed");
const deckId = Number(option("deck") ?? 2);
const read = async () => (await (await fetch(`${origin}/api/decks/${deckId}`, { headers: { Cookie: "uxeval_user=1" } })).json()).state;
const before = await read();
const cards = before.cards.filter(card => !card.maybe && card.role === "card").slice(0, 2);
assert.equal(cards.length, 2);
const browser = await chromium.launch({ args: BROWSER_ARGS }), results = [];
try {
  for (const width of [1280, 1440, 1920, 2560]) {
    const context = await newContext(browser, { origin, vp: VIEWPORTS[width], user: 1, frozenNow: "2026-10-20T17:00:00Z", notes: { blocked: new Set(), imageFailures: new Set() } });
    const page = await context.newPage(), watch = watchPage(page);
    const panel = page.getByRole("complementary", { name: "Deck panel" });
    const nav = panel.getByRole("navigation", { name: "Deck panel views" });
    const current = label => nav.getByRole("button", { name: new RegExp(`^${label}(?:\\s*\\d+)?$`) });
    const body = () => panel.locator("[data-panel-body]");
    const row = card => page.locator(`[data-deck-list="true"] button[data-oracle-id="${card.oracle_id}"]`).first();
    const open = async card => { await scrollUntilVisible(page, row(card), VIEWPORTS[width]); await row(card).click(); await settle(page, watch); await panel.getByRole("heading", { name: card.name, exact: true }).waitFor(); assert.equal(await nav.count(), 0); };
    const back = async label => { await panel.getByRole("button", { name: `Back to ${label}`, exact: true }).click(); await settle(page, watch); assert.equal(await current(label).getAttribute("aria-current"), "true"); };
    await page.goto(`${origin}/deck/${deckId}`); await current("Brief").waitFor();
    assert.deepEqual(await nav.getByRole("button").allTextContents(), ["Review3", "Brief", "Power", "Stats", "Checks", "Memories", "History"]);
    assert.equal(await panel.getByRole("combobox", { name: "Panel view" }).count(), 0);
    const bounds = await nav.getByRole("button").evaluateAll(nodes => nodes.map(node => ({ label: node.textContent, top: Math.round(node.getBoundingClientRect().top) })));
    assert.equal(new Set(bounds.map(item => item.top)).size, 1, "all default-width destinations stay on one line");
    for (const label of ["Review", "Brief", "Power", "Stats", "Checks", "Memories", "History"]) {
      const link = current(label);
      await link.focus(); await link.press("Enter"); await settle(page, watch);
      assert.equal(await link.getAttribute("aria-current"), "true");
      await panel.getByRole("region", { name: label, exact: true }).waitFor();
      if (label === "Stats") {
        const comparison = panel.getByRole("region", { name: "Mana demand and sources", exact: true });
        await comparison.waitFor();
        const expected = before.computed.mana.first_face;
        const costCounts = ["W", "U", "B", "R", "G"].map(color => expected.costs.fixed_symbols[color]);
        const sourceCounts = ["W", "U", "B", "R", "G"].map(color => expected.sources.land.by_type[color] + expected.sources.nonland.by_type[color]);
        assert.deepEqual((await comparison.getByRole("figure", { name: "Cost demand", exact: true }).locator("li b").allTextContents()).map(Number), costCounts);
        assert.deepEqual((await comparison.getByRole("figure", { name: "Direct sources", exact: true }).locator("li b").allTextContents()).map(Number), sourceCounts);
        await comparison.getByText(`${costCounts.reduce((a, b) => a + b, 0)} fixed coloured symbols`, { exact: true }).waitFor();
        await comparison.getByText(`${sourceCounts.reduce((a, b) => a + b, 0)} source-colour counts`, { exact: true }).waitFor();
        const exact = comparison.locator("summary").filter({ hasText: "Exact colour counts" });
        await exact.focus(); await exact.press("Enter");
        const rows = comparison.locator("table").first().locator("tbody tr");
        for (const [index, color] of ["W", "U", "B", "R", "G"].entries()) {
          const expected = before.computed.mana.first_face;
          assert.deepEqual((await rows.nth(index).locator("td").allTextContents()).map(Number), [expected.costs.fixed_symbols[color], expected.sources.land.by_type[color], expected.sources.nonland.by_type[color]]);
        }
        await exact.press("Enter");
        const costs = comparison.locator("summary").filter({ hasText: "Other cost symbols" });
        await costs.focus(); await costs.press("Enter");
        await comparison.getByText(`${before.computed.mana.first_face.costs.generic_units} generic units · ${before.computed.mana.first_face.costs.snow_symbols} snow symbols`, { exact: true }).waitFor();
        const coverage = comparison.locator("summary").filter({ hasText: /^Source coverage/ });
        await coverage.focus(); await coverage.press("Space");
        await comparison.getByText(/Across printed faces:/).waitFor();
      }
    }
    await current("Brief").click(); await panel.getByRole("button", { name: "Edit the thesis", exact: true }).click();
    const draft = panel.getByRole("textbox", { name: "Thesis", exact: true });
    const text = Array.from({ length: 30 }, (_, i) => `Unsaved inspection draft ${i}`).join("\n");
    await draft.fill(text); await page.evaluate(() => { window.__inspectionDraft = document.querySelector('textarea[aria-label="Thesis"]'); window.__inspectionBody = document.querySelector("[data-panel-body]"); });
    const pane = await body().boundingBox(); await page.mouse.move(pane.x + pane.width / 2, pane.y + pane.height / 2); await page.mouse.wheel(0, 220); await settle(page, watch);
    const scroll = await body().evaluate(node => node.scrollTop); assert(scroll > 0);
    await open(cards[0]); assert.equal(await row(cards[0]).evaluate(node => node === document.activeElement), true, "list focus stays in list");
    assert.equal(await panel.locator("[hidden][inert]").count(), 1);
    assert.equal(await panel.getByRole("textbox", { name: "Thesis", exact: true }).count(), 0, "hidden draft is absent from accessibility tree");
    await open(cards[1]); await back("Brief");
    assert.equal(await draft.inputValue(), text);
    assert.equal(await page.evaluate(() => window.__inspectionDraft === document.querySelector('textarea[aria-label="Thesis"]')), true);
    assert.equal(await page.evaluate(() => window.__inspectionBody === document.querySelector("[data-panel-body]")), true);
    assert(Math.abs(await body().evaluate(node => node.scrollTop) - scroll) <= 1, "inspection restores deck scroll");
    assert.equal(await row(cards[0]).evaluate(node => node === document.activeElement), true, "card B retains card A's first origin");
    await open(cards[0]); await page.keyboard.press("Escape"); await settle(page, watch); assert.equal(await draft.inputValue(), text); assert.equal(await row(cards[0]).evaluate(node => node === document.activeElement), true);
    // Use an existing, DB-backed engine card link while a different field is edited.
    const piece = before.brief.engines[0].pieces[0];
    const trigger = panel.getByRole("button", { name: `✓ ${piece.name}`, exact: true }).first();
    await scrollUntilVisible(page, trigger, VIEWPORTS[width]); await trigger.focus(); await trigger.press("Enter"); await settle(page, watch);
    assert.equal(await panel.getByRole("button", { name: "Back to Brief", exact: true }).evaluate(node => node === document.activeElement), true, "hidden panel trigger hands focus to Back");
    await back("Brief"); assert.equal(await trigger.evaluate(node => node === document.activeElement), true); assert.equal(await draft.inputValue(), text);
    await page.getByRole("button", { name: /^Review in the list/ }).click(); await page.keyboard.press("j"); await settle(page, watch);
    const cursor = await page.locator('[data-deck-list="true"] [data-current="true"]').evaluateAll(nodes => nodes.map(node => node.dataset.oracleId));
    const reason = panel.getByRole("textbox", { name: /Reason for decision .*optional/ }).first();
    await scrollUntilVisible(page, reason, VIEWPORTS[width]); await reason.fill("Unsaved inspection reason");
    await page.evaluate(() => { window.__inspectionReason = document.querySelector('[data-panel-body] input'); });
    const reviewScroll = await body().evaluate(node => node.scrollTop);
    await open(cards[0]); await back("Review"); assert.equal(await reason.inputValue(), "Unsaved inspection reason");
    assert.deepEqual(await page.locator('[data-deck-list="true"] [data-current="true"]').evaluateAll(nodes => nodes.map(node => node.dataset.oracleId)), cursor);
    assert(Math.abs(await body().evaluate(node => node.scrollTop) - reviewScroll) <= 1, "review scroll survives card inspection");
    await page.keyboard.press("r");
    const rejecting = panel.getByRole("textbox", { name: "Reason for rejecting, optional", exact: true });
    await rejecting.fill("Unsaved rejection draft");
    await open(cards[0]); await page.keyboard.press("Escape"); await settle(page, watch);
    assert.equal(await rejecting.inputValue(), "Unsaved rejection draft", "list Escape leaves card before cancelling rejection");
    await panel.getByRole("button", { name: "Cancel", exact: true }).click();
    await current("Brief").click(); await panel.getByRole("button", { name: "Add an engine", exact: true }).click();
    const engine = panel.getByRole("textbox", { name: "Name", exact: true }); await engine.fill("Unsaved engine"); await open(cards[0]); await back("Brief"); assert.equal(await engine.inputValue(), "Unsaved engine");
    assert.equal((await axe(page)).length, 0);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    const resize = page.getByRole("separator", { name: "Resize the deck panel" }); await resize.focus(); await resize.press("Home");
    const minimum = await nav.getByRole("button").evaluateAll(nodes => nodes.map(node => Math.round(node.getBoundingClientRect().top)));
    assert.equal(new Set(minimum).size, 1, "minimum panel keeps every destination on one line");
    assert.equal(await nav.evaluate(node => node.scrollWidth > node.clientWidth), false, "minimum panel does not clip navigation");
    await resize.press("End");
    const roomy = await nav.getByRole("button").evaluateAll(nodes => nodes.map(node => Math.round(node.getBoundingClientRect().top)));
    assert.equal(new Set(roomy).size, 1, "roomy panel has one navigation row");
    results.push({ width, visible_navigation: true, single_row_at_min_default_wide: true, keyboard_activation: true, draft_dom_scroll_preserved: true, reason_scroll_preserved: true, rejection_draft_escape_preserved: true, engine_form_preserved: true, first_card_origin_return: true, panel_trigger_focus_return: true, hidden_content_inaccessible: true });
    await context.close(); console.log(`Panel inspection ${width} passed`);
  }
  const context = await newContext(browser, { origin, vp: VIEWPORTS[1440], user: 1, frozenNow: "2026-10-20T17:00:00Z", notes: { blocked: new Set(), imageFailures: new Set() } });
  const page = await context.newPage(), watch = watchPage(page);
  await page.goto(`${origin}/deck/${deckId}`);
  const panel = page.getByRole("complementary", { name: "Deck panel" });
  await panel.getByRole("button", { name: "Edit the thesis", exact: true }).click();
  const draft = panel.getByRole("textbox", { name: "Thesis", exact: true }); await draft.fill("Unsaved shared popup draft");
  await page.evaluate(() => { window.__sharedDraft = document.querySelector('textarea[aria-label="Thesis"]'); });
  for (const width of [1024, 1440, 768, 1440]) {
    await page.setViewportSize({ width, height: width < 1100 ? 1024 : 900 }); await settle(page, watch);
    if (width < 1100 && await page.getByRole("dialog").count() === 0) await page.getByRole("button", { name: "Open deck details", exact: true }).click();
    assert.equal(await draft.inputValue(), "Unsaved shared popup draft");
    assert.equal(await page.evaluate(() => window.__sharedDraft === document.querySelector('textarea[aria-label="Thesis"]')), true);
  }
  const trigger = panel.getByRole("button", { name: `✓ ${before.brief.engines[0].pieces[0].name}`, exact: true }).first();
  await scrollUntilVisible(page, trigger, VIEWPORTS[1440]); await trigger.click(); await settle(page, watch);
  for (const width of [1024, 768]) {
    await page.setViewportSize({ width, height: 1024 }); await settle(page, watch);
    if (await page.getByRole("dialog").count() === 0) await page.getByRole("button", { name: "Open deck details", exact: true }).click();
    const sheet = page.getByRole("dialog", { name: "Deck details", exact: true }); await sheet.waitFor();
    const back = panel.getByRole("button", { name: "Back to Brief", exact: true }); await back.focus(); await back.press("Escape"); await settle(page, watch);
    await page.getByRole("dialog", { name: "Deck details · Brief", exact: true }).waitFor(); assert.equal(await draft.inputValue(), "Unsaved shared popup draft");
    await scrollUntilVisible(page, trigger, { width, height: 1024 }); await trigger.click(); await settle(page, watch);
  }
  await page.getByRole("dialog").getByRole("button", { name: "Close", exact: true }).click(); await settle(page, watch); assert.equal(await page.getByRole("dialog").count(), 0);
  await page.getByRole("button", { name: "Open deck details", exact: true }).click(); assert.equal(await draft.inputValue(), "Unsaved shared popup draft");
  assert.equal((await axe(page)).length, 0); await context.close();
  assert.deepEqual(await read(), before, "inspection and unsaved forms do not write deck data");
  const report = { results, deck_data_unchanged: true, shared_popup_escape_back_close_distinct: true, shared_draft_dom_preserved_across_resize: true };
  if (option("report")) writeFileSync(option("report"), JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify(report));
} finally { await browser.close(); }
