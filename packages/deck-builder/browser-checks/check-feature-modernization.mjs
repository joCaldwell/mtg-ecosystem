// Runs only against a disposable loopback browser-checks fixture.
import assert from "node:assert/strict";
import { chromium } from "playwright-core";

const index = process.argv.indexOf("--origin");
const origin = index < 0 ? "http://127.0.0.1:8873" : process.argv[index + 1];
const url = new URL(origin);
assert.equal(url.protocol, "http:");
assert.ok(["127.0.0.1", "localhost", "[::1]"].includes(url.hostname));
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ baseURL: origin, viewport: { width: 1440, height: 900 }, reducedMotion: "reduce" });
await context.addCookies([{ name: "checks_user", value: "1", url: origin }]);
const page = await context.newPage();
const pageErrors = [];
page.on("pageerror", error => pageErrors.push(error.message));
const mutations = [];
page.on("request", request => { if (["POST", "PATCH", "DELETE"].includes(request.method())) mutations.push(new URL(request.url()).pathname); });
const frame = () => page.evaluate(() => new Promise(requestAnimationFrame));
const checks = [];
async function check(name, fn) { await fn(); checks.push(name); console.log(`Passed: ${name}`); }
async function openSlots(deckId) {
  await page.goto(`/deck/${deckId}`, { waitUntil: "domcontentloaded" });
  await page.getByRole("button", { name: "More", exact: true }).click();
  await page.locator('[focusgroup~="menu"] button, [role="menuitem"]').filter({ hasText: "Edit slots and tags" }).click();
  return page.getByRole("dialog", { name: "Slots and tags", exact: true });
}
try {
  const summaries = await (await context.request.get("/api/decks")).json();
  let data;
  for (const summary of summaries.filter(deck => deck.category !== "archived")) {
    const candidate = (await (await context.request.get(`/api/decks/${summary.id}`)).json()).state;
    if (candidate.tags.length >= 2 && candidate.computed.slot_deltas.length) { data = candidate; break; }
  }
  assert.ok(data, "Two tags and a slot from source-backed fixture required");

  await check("Empty and whitespace-only deck names reach native validation without a request", async () => {
    await page.goto("/decks", { waitUntil: "domcontentloaded" });
    await page.locator("main").getByRole("button", { name: "New deck", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "New deck", exact: true });
    const name = dialog.getByRole("textbox", { name: "Deck name", exact: true });
    const submit = dialog.getByRole("button", { name: "Create deck", exact: true });
    assert.equal(await submit.isEnabled(), true);
    const before = mutations.length;
    await submit.click();
    assert.equal(await name.evaluate(el => el === document.activeElement && !el.validity.valid), true);
    await name.fill("   "); await submit.click();
    assert.equal(await name.evaluate(el => el.validity.patternMismatch), true);
    await name.fill("Native validation check");
    assert.equal(await name.evaluate(el => el.validity.valid), true);
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    assert.equal(mutations.length, before);
  });

  await check("Memory budget errors wait for submit and native constraints retain keyboard focus", async () => {
    await page.goto("/settings", { waitUntil: "domcontentloaded" });
    const form = page.getByRole("form", { name: "Memory budgets", exact: true });
    const global = form.getByRole("spinbutton", { name: "Global limit (tokens)", exact: true });
    await global.waitFor();
    assert.equal(await global.getAttribute("aria-invalid"), null);
    const before = mutations.length;
    await global.fill("0");
    assert.equal(await form.getByRole("alert").count(), 0, "Typing does not announce validation errors");
    const save = form.getByRole("button", { name: "Save", exact: true });
    assert.equal(await save.isEnabled(), true);
    await save.click();
    assert.equal(await global.evaluate(el => el === document.activeElement && el.validity.rangeUnderflow), true);
    assert.equal(mutations.length, before);
  });

  await check("IME-confirming Enter never saves slot or tag drafts, including Safari keyCode 229", async () => {
    const dialog = await openSlots(data.deck.id);
    for (const [label, original] of [
      [`Name of the slot ${data.computed.slot_deltas[0].name}`, data.computed.slot_deltas[0].name],
      [`Name of the tag ${data.tags[0].name}`, data.tags[0].name],
    ]) {
      const field = dialog.getByRole("textbox", { name: label, exact: true });
      await field.fill(`${original} draft`);
      const before = mutations.length;
      for (const event of [{ isComposing: true, keyCode: 13 }, { isComposing: false, keyCode: 229 }]) {
        await field.dispatchEvent("keydown", { key: "Enter", code: "Enter", bubbles: true, ...event });
        await frame();
      }
      assert.equal(mutations.length, before, "IME confirmation must not write");
      assert.equal(await field.inputValue(), `${original} draft`);
      await field.fill(original);
    }
    await dialog.getByRole("button", { name: "Close", exact: true }).last().click();
  });

  await check("Delayed older mutations cannot roll back a newer server composite", async () => {
    const dialog = await openSlots(data.deck.id);
    const first = data.tags[0], second = data.tags[1];
    let release, arrived, handled;
    const unblock = new Promise(resolve => { release = resolve; });
    const captured = new Promise(resolve => { arrived = resolve; });
    const delivered = new Promise(resolve => { handled = resolve; });
    const pattern = `**/api/decks/${data.deck.id}/tags/${first.id}`;
    await page.route(pattern, async route => {
      const response = await route.fetch();
      arrived(); await unblock;
      try { await route.fulfill({ response }); } finally { handled(); }
    });
    try {
      const one = dialog.getByRole("textbox", { name: `Name of the tag ${first.name}`, exact: true });
      await one.fill(`${first.name} first`); await one.press("Enter"); await captured;
      const two = dialog.getByRole("textbox", { name: `Name of the tag ${second.name}`, exact: true });
      await two.fill(`${second.name} latest`); await two.press("Enter");
      await dialog.getByRole("textbox", { name: `Name of the tag ${second.name} latest`, exact: true }).waitFor();
      release(); await delivered; await frame();
      assert.equal(await dialog.getByRole("textbox", { name: `Name of the tag ${second.name} latest`, exact: true }).inputValue(), `${second.name} latest`);
    } finally { release(); await page.unroute(pattern); }
    await dialog.getByRole("button", { name: "Close", exact: true }).last().click();
  });

  await check("Standalone legal content has one main heading and scrolls by wheel", async () => {
    await page.goto("/privacy", { waitUntil: "domcontentloaded" });
    await page.getByRole("heading", { name: "Privacy", exact: true }).waitFor();
    assert.equal(await page.locator("main#main-content").count(), 1);
    assert.equal(await page.locator("h1").count(), 1);
    await page.mouse.move(900, 500); await page.mouse.wheel(0, 900);
    await page.waitForFunction(() => scrollY > 0);
  });

  await check("Card result actions remain outside options and reachable by Tab", async () => {
    const created = await context.request.post("/api/decks", { headers: { Origin: origin }, data: { name: `Keyboard card search check ${summaries.length}`, category: "theorycrafted" } });
    assert.ok(created.ok(), await created.text());
    const fresh = await created.json();
    const commander = data.cards.find(card => card.role === "commander");
    assert.ok(commander);
    await page.goto(`/deck/${fresh.state.deck.id}`, { waitUntil: "domcontentloaded" });
    const input = page.getByRole("combobox", { name: "Add a card or search", exact: true });
    await input.fill(commander.name);
    const list = page.getByRole("listbox", { name: "Card search results", exact: true });
    await list.getByRole("option").first().waitFor();
    assert.equal(await list.locator("button, input, select, textarea, a[href]").count(), 0);
    const actions = page.getByRole("group", { name: `Actions for ${commander.name}`, exact: true });
    await actions.getByRole("button", { name: "Add as commander", exact: true }).waitFor();
    await input.focus(); await page.keyboard.press("Tab");
    assert.equal(await actions.getByRole("button", { name: "Add", exact: true }).evaluate(el => el === document.activeElement), true);
    await page.keyboard.press("Tab");
    assert.equal(await actions.getByRole("button", { name: /^Add to maybe/ }).evaluate(el => el === document.activeElement), true);
    await page.keyboard.press("Tab");
    assert.equal(await actions.getByRole("button", { name: "Add as commander", exact: true }).evaluate(el => el === document.activeElement), true);
    assert.equal((await (await context.request.get(`/api/decks/${fresh.state.deck.id}`)).json()).state.cards.length, 0);
  });
  assert.deepEqual(pageErrors, []);
  console.log(JSON.stringify({ browser: browser.version(), checks }, null, 2));
} finally { await context.close(); await browser.close(); }
