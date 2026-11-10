#!/usr/bin/env node
// Real data + real-input checks against serve.ts's disposable fixture copy.
import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Database } from "../src/sqlite.ts";
import { chromium } from "playwright-core";
import { CHECKS_ROOT } from "./lib/paths.mjs";
import { BROWSER_ARGS, VIEWPORTS, newContext, watchPage, settle, axe, shoot } from "./lib/browser.mjs";

const option = name => { const i = process.argv.indexOf(`--${name}`); return i < 0 ? undefined : process.argv[i + 1]; };
const origin = option("origin") ?? "http://127.0.0.1:8858";
assert.equal(new URL(origin).hostname, "127.0.0.1", "fixture loopback only");
const output = option("out") ?? join(CHECKS_ROOT, "manual", "visual-stats-qa"); mkdirSync(output, { recursive: true });
const api = async (method, path, body) => {
  const response = await fetch(origin + path, { method, headers: { Cookie: "checks_user=1", Origin: origin, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined });
  assert.equal(response.status, 200, `${method} ${path}`); return response.json();
};
const facts = new Database(join(CHECKS_ROOT, "seed", "seed.db"), { readOnly: true });
const cases = [
  ["empty", []],
  ["mono-white", [["Plains", 3], ["Witch Enchanter // Witch-Blessed Meadow", 1]]],
  ["mono-black", [["Swamp", 3], ["Ruthless Knave", 1]]],
  ["colourless", [["Sol Ring", 2]]],
  ["any-colour", [["Birds of Paradise", 1]]],
  ["overlapping-sources", [["Glacial Fortress", 1], ["Savai Triome", 1], ["Birds of Paradise", 1]]],
  ["flexible-faces", [["Toluz, Clever Conductor", 1], ["Witch Enchanter // Witch-Blessed Meadow", 1]]],
  ["unresolved", [["Command Tower", 1]]],
];
const fixtures = [];
for (const [key, cards] of cases) {
  const created = await api("POST", "/api/decks", { name: `Stats QA ${key}`, category: "theorycrafted" });
  const id = created.state.deck.id;
  for (const [name, quantity] of cards) {
    const card = facts.prepare("SELECT oracle_id FROM cards WHERE name = ?").get(name);
    assert(card, `exact DB fixture ${name}`);
    await api("POST", `/api/decks/${id}/cards`, { oracle_id: card.oracle_id, quantity });
  }
  fixtures.push({ key, state: (await api("GET", `/api/decks/${id}`)).state });
}
facts.close();
const colours = ["W", "U", "B", "R", "G"], browser = await chromium.launch({ args: BROWSER_ARGS }), results = [];
const percentage = (count, total) => !total ? "—" : !count ? "0%" : count === total ? "100%" : `${Number((100 * count / total).toFixed(1))}%`;
async function wheelTo(page, panel, target) {
  const pane = panel.locator("[data-panel-body]");
  for (let step = 0; step < 3; step++) {
    const a = await target.boundingBox(), b = await pane.boundingBox(); assert(a && b);
    const distance = a.y - b.y - 16;
    if (Math.abs(distance) < 3) return;
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2); await page.mouse.wheel(0, Math.round(distance));
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  }
}
async function checkCharts(page, state) {
  const comparison = page.getByRole("region", { name: "Mana demand and sources", exact: true }); await comparison.waitFor();
  const face = state.computed.mana.first_face;
  const legend = comparison.getByRole("table").first();
  assert.deepEqual(await legend.locator("thead th").allTextContents(), ["Colour", "Cost demand", "Direct sources"]);
  assert.deepEqual(await legend.locator("tbody th").allTextContents(), ["White", "Blue", "Black", "Red", "Green"]);
  for (const [title, values, column, unit] of [["Cost demand", colours.map(c => face.costs.fixed_symbols[c]), 2, "fixed coloured symbols"], ["Direct sources", colours.map(c => face.sources.land.by_type[c] + face.sources.nonland.by_type[c]), 4, "source-colour counts"]]) {
    const figure = comparison.getByRole("figure").filter({ has: page.getByRole("heading", { name: title, exact: true }) }), total = values.reduce((a, b) => a + b, 0);
    await figure.getByText(`${total} ${unit}`, { exact: true }).waitFor();
    assert.deepEqual((await legend.locator(`tbody td:nth-child(${column})`).allTextContents()).map(Number), values);
    assert.deepEqual(await legend.locator(`tbody td:nth-child(${column + 1})`).allTextContents(), values.map(value => percentage(value, total)));
    const slices = await figure.locator('svg circle[pathLength="100"][stroke-width="16"]').evaluateAll(nodes => nodes.map(node => node.getAttribute("stroke-dasharray").split(" ").map(Number)));
    assert(slices.flat().every(Number.isFinite)); assert.equal(slices.length, values.filter(Boolean).length);
    if (!total) { assert.equal(slices.length, 0); assert.equal(await figure.locator("p").count(), 1); }
    if (values.filter(Boolean).length === 1) assert.equal(slices[0][0], 100, "mono colour is a full ring");
  }
  assert.equal(await comparison.locator("svg").evaluateAll(nodes => nodes.every(node => node.getAttribute("aria-hidden") === "true")), true, "visible legends provide the accessible data");
  return comparison;
}
try {
  const main = (await api("GET", "/api/decks/2")).state;
  for (const width of [1280, 1440, 1920, 2560]) {
    const context = await newContext(browser, { origin, vp: VIEWPORTS[width], user: 1, frozenNow: "2026-10-20T17:00:00Z", notes: { blocked: new Set(), imageFailures: new Set() } });
    const page = await context.newPage(), watch = watchPage(page); await page.goto(`${origin}/deck/2`);
    const panel = page.getByRole("complementary", { name: "Deck panel" }), nav = panel.getByRole("navigation", { name: "Deck panel views" });
    await nav.getByRole("button", { name: "Stats", exact: true }).click(); await settle(page, watch);
    const resize = page.getByRole("separator", { name: "Resize the deck panel" });
    for (const [size, key] of [["default", null], ["minimum", "Home"], ["wider", "End"]]) {
      if (key) { await resize.focus(); await resize.press(key); await settle(page, watch); }
      const actualWidth = (await panel.boundingBox()).width;
      const totals = await panel.locator("[data-panel-body] dl").first().locator("dd").evaluateAll(nodes => nodes.map(node => Math.round(node.getBoundingClientRect().top)));
      assert.equal(new Set(totals).size, 1, "summary metric values share a baseline at every panel width");
      const comparison = await checkCharts(page, main); await wheelTo(page, panel, comparison); await settle(page, watch);
      const figures = comparison.getByRole("figure"), first = await figures.nth(0).boundingBox(), second = await figures.nth(1).boundingBox();
      assert(first && second);
      assert(Math.abs(first.y - second.y) < 2, "the paired donuts share one row above their common table");
      const content = await comparison.boundingBox();
      for (const box of [first, second, await comparison.getByRole("table").first().boundingBox()]) assert(box.x >= content.x - 1 && box.x + box.width <= content.x + content.width + 1, "charts and exact colour table fit the actual panel content width");
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
      const violations = await axe(page); assert.equal(violations.length, 0, JSON.stringify(violations));
      assert.deepEqual(watch.errors, []);
      const fontSizes = await comparison.locator("th, td, figcaption").evaluateAll(nodes => nodes.map(node => parseFloat(getComputedStyle(node).fontSize))); assert(fontSizes.length > 0 && fontSizes.every(size => size >= 12));
      await shoot(page, join(output, `main-${width}-${size}-colours.png`), { full: false });
      const types = panel.getByText("Card types", { exact: true }); await wheelTo(page, panel, types); await settle(page, watch);
      const typeValues = await panel.getByRole("list", { name: "Card-type groups and copy counts" }).locator("li b").allTextContents();
      assert.equal(typeValues.map(Number).reduce((a, b) => a + b, 0), main.computed.card_count);
      await shoot(page, join(output, `main-${width}-${size}-types.png`), { full: false });
      results.push({ width, requested: size, actual_width: actualWidth, legend_matches_capability: true, exact_units: true, adaptive_layout: true, axe: 0, overflow: 0, readable_labels: true });
    }
    await context.close(); console.log(`Visual Stats ${width} passed`);
  }
  for (const fixture of fixtures) {
    const context = await newContext(browser, { origin, vp: VIEWPORTS[1440], user: 1, frozenNow: "2026-10-20T17:00:00Z", notes: { blocked: new Set(), imageFailures: new Set() } });
    const page = await context.newPage(), watch = watchPage(page); await page.goto(`${origin}/deck/${fixture.state.deck.id}`);
    const panel = page.getByRole("complementary", { name: "Deck panel" });
    await panel.getByRole("navigation", { name: "Deck panel views" }).getByRole("button", { name: "Stats", exact: true }).click(); await settle(page, watch);
    const comparison = await checkCharts(page, fixture.state); await wheelTo(page, panel, comparison); await settle(page, watch);
    if (fixture.key === "overlapping-sources") await comparison.getByText("5 source-colour counts", { exact: true }).waitFor();
    if (fixture.key === "any-colour") {
      await comparison.getByText("Any-colour sources are listed under Source coverage.", { exact: true }).waitFor();
      await comparison.locator("summary").filter({ hasText: /^Source (coverage|recognition details)/ }).click();
      await comparison.getByText("Any-colour sources", { exact: true }).waitFor();
      await comparison.getByText(`${fixture.state.computed.mana.first_face.sources.land.any_color_copies} lands · ${fixture.state.computed.mana.first_face.sources.nonland.any_color_copies} other permanents`, { exact: true }).waitFor();
    }
    if (fixture.key === "empty") await panel.getByText("No cards in this curve.", { exact: true }).waitFor();
    const violations = await axe(page); assert.equal(violations.length, 0, JSON.stringify(violations)); assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    assert.deepEqual(watch.errors, []);
    await shoot(page, join(output, `edge-${fixture.key}.png`), { full: false });
    assert.deepEqual((await api("GET", `/api/decks/${fixture.state.deck.id}`)).state, fixture.state);
    results.push({ edge: fixture.key, id: fixture.state.deck.id, figures_match_capability: true, axe: 0, overflow: 0 });
    await context.close(); console.log(`Stats edge ${fixture.key} passed`);
  }
  // Sharing remains the existing capability operation; only prose is removed.
  const id = fixtures[0].state.deck.id, context = await newContext(browser, { origin, vp: VIEWPORTS[1440], user: 1, frozenNow: "2026-10-20T17:00:00Z", notes: { blocked: new Set(), imageFailures: new Set() } });
  const page = await context.newPage(), watch = watchPage(page); await page.goto(`${origin}/deck/${id}`); await page.getByRole("button", { name: "Share", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Share this deck", exact: true }), visibility = dialog.getByRole("group", { name: "Who can see this deck" });
  for (const value of ["Unlisted", "Public", "Private"]) { await visibility.getByRole("button", { name: value, exact: true }).click(); await settle(page, watch); assert.equal(await visibility.getByRole("button", { name: value, exact: true }).getAttribute("aria-pressed"), "true"); assert.equal((await api("GET", `/api/decks/${id}`)).state.deck.visibility, value.toLowerCase()); }
  const checkbox = dialog.getByRole("checkbox", { name: "Show this deck's decisions log on its page", exact: true });
  const publication = !await checkbox.isChecked(); await checkbox.click(); await settle(page, watch);
  assert.equal(await checkbox.isChecked(), publication); assert.equal((await api("GET", `/api/decks/${id}`)).state.deck.publish_decisions, publication);
  assert.equal(await dialog.getByText(/Viewers never see|Off by default|Only you can see/).count(), 0);
  await visibility.getByRole("button", { name: "Unlisted", exact: true }).click(); await settle(page, watch); await dialog.getByRole("textbox", { name: "Link to this deck", exact: true }).waitFor(); assert.equal(await dialog.getByRole("button", { name: "Copy link", exact: true }).isEnabled(), true); await dialog.getByRole("link", { name: "Preview public page", exact: true }).waitFor();
  await shoot(page, join(output, "share-controls.png"), { full: false }); await context.close();
  writeFileSync(join(output, "qa.json"), JSON.stringify({ results, fixture_decks: fixtures.map(({ key, state }) => ({ key, id: state.deck.id, mana: state.computed.mana })), share_controls_and_capability_unchanged: true }, null, 2) + "\n");
} finally { await browser.close(); }
