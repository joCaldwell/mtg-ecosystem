// Only run against a fresh disposable fixture. This removes one slot and tag.
import assert from "node:assert/strict";
import { chromium, firefox } from "playwright-core";

const arg = (name, fallback) => process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : fallback;
const origin = arg("--origin", "http://127.0.0.1:19713");
const url = new URL(origin);
assert.equal(url.protocol, "http:");
assert.ok(["127.0.0.1", "localhost", "[::1]"].includes(url.hostname));
const engine = arg("--browser", "chromium");
assert.ok(["chromium", "firefox"].includes(engine));
const browser = await (engine === "firefox" ? firefox : chromium).launch();
try {
  const context = await browser.newContext({ baseURL: origin, viewport: { width: 1440, height: 900 }, reducedMotion: "reduce" });
  await context.addCookies([{ name: "checks_user", value: "1", url: origin }]);
  await context.route(/^(?!http:\/\/127\.0\.0\.1|http:\/\/localhost|http:\/\/\[::1\]).*/, route => route.abort());
  const page = await context.newPage();
  page.setDefaultTimeout(10_000);
  const decks = await (await context.request.get("/api/decks")).json();
  const states = await Promise.all(decks.filter(deck => deck.category !== "archived").map(async ({id}) =>
    (await (await context.request.get(`/api/decks/${id}`)).json()).state));
  const list = states.find(state => state.tags.length >= 2 && state.computed.slot_deltas.length >= 2);
  const shared = states.find(state => state.deck.slug && state.deck.visibility !== "private");
  assert.ok(list && shared, "Fixture needs an editable deck with tags and slots and a shared deck.");

  const more = page.getByRole("button", { name: "More", exact: true });
  const menuItem = name => page.locator('[focusgroup~="menu"] button, [role="menuitem"]').getByText(name, { exact: true });
  const button = (scope, name) => scope.getByRole("button", { name, exact: true });
  const input = (scope, name) => scope.getByRole("textbox", { name, exact: true });
  const focused = locator => locator.evaluate(el => document.activeElement === el);
  const frame = () => page.evaluate(() => new Promise(requestAnimationFrame));
  async function holdFailure(path, method) {
    const arrived = Promise.withResolvers();
    const proceed = Promise.withResolvers();
    await page.route(path, async route => {
      if (route.request().method() !== method) return route.continue();
      arrived.resolve();
      await proceed.promise;
      await route.fulfill({ status: 409, json: { error: "Fixture request failed" } });
    });
    return { arrived: arrived.promise, release: proceed.resolve };
  }

  await page.goto(`/deck/${list.deck.id}`);
  await more.click();
  await page.keyboard.press("Tab");
  assert.equal(await more.getAttribute("aria-expanded"), "false");
  assert.equal(await page.evaluate(() => document.activeElement?.getAttribute("role")), "combobox");
  await more.press("Enter");
  await page.waitForFunction(() => !!document.activeElement?.closest('[focusgroup~="menu"]'));
  const firstItem = await page.evaluate(() => document.activeElement?.textContent);
  await page.keyboard.press("ArrowDown");
  assert.notEqual(await page.evaluate(() => document.activeElement?.textContent), firstItem);
  await page.keyboard.press("Escape");
  assert.equal(await more.getAttribute("aria-expanded"), "false");
  assert.ok(await focused(more));
  await more.click();
  await menuItem("Delete deck").click();
  const deletion = page.getByRole("dialog", { name: "Delete this deck", exact: true });
  assert.ok(await focused(button(deletion, "Cancel")));
  const pendingDelete = await holdFailure(`**/api/decks/${list.deck.id}`, "DELETE");
  await button(deletion, "Delete deck").click();
  await pendingDelete.arrived;
  await deletion.getByRole("status").getByText(/Deleting this deck/).waitFor();
  assert.ok(await button(deletion, "Close").isDisabled());
  assert.ok(await button(deletion, "Cancel").isDisabled());
  await page.keyboard.press("Escape");
  await page.mouse.click(5, 5);
  assert.ok(await deletion.isVisible());
  pendingDelete.release();
  await deletion.getByRole("alert").getByText("Fixture request failed").waitFor();
  await button(deletion, "Cancel").click();

  await more.click();
  await menuItem("Edit slots and tags").click();
  const slots = page.getByRole("dialog", { name: "Slots and tags", exact: true });
  const [slot, nextSlot] = list.computed.slot_deltas;
  const [tag, nextTag] = list.tags;
  const min = input(slots, `Minimum for ${slot.name}`);
  const validMin = await min.inputValue();
  await min.fill("-2");
  assert.equal(await min.getAttribute("aria-invalid"), null);
  await min.press("Tab");
  await slots.getByRole("status").getByText(`Minimum for ${slot.name}. Enter a whole number of zero or more.`).waitFor();
  const description = await min.getAttribute("aria-describedby");
  assert.ok(description && await page.locator(`[id="${description}"]`).isVisible());
  assert.equal(await min.evaluate(el => el.validity.customError), true);
  await min.press("Escape");
  assert.ok(await slots.isVisible());
  assert.equal(await min.inputValue(), validMin);
  assert.equal(await min.getAttribute("aria-invalid"), null);
  const newName = input(slots, "Name of the new slot");
  const newMin = input(slots, "Minimum for the new slot");
  const newMax = input(slots, "Maximum for the new slot");
  await newName.fill("Fixture validation only");
  await newMin.fill("-2");
  await newMax.fill("-5");
  await newName.press("Enter");
  await frame();
  assert.ok(await focused(newMin));
  assert.equal(await newMin.getAttribute("aria-invalid"), "true");
  assert.equal(await newMax.getAttribute("aria-invalid"), "true");
  await newName.fill("");
  await newMin.fill("");
  await newMax.fill("");
  for (const [kind, item, next] of [["slot", slot, nextSlot], ["tag", tag, nextTag]]) {
    const trigger = button(slots, `Delete ${item.name}`);
    await trigger.click();
    await page.keyboard.press("Escape");
    assert.ok(await slots.isVisible());
    assert.ok(await focused(trigger));
    await trigger.click();
    await button(slots, `Delete ${kind}`).click();
    await trigger.waitFor({ state: "detached" });
    await frame();
    assert.ok(await focused(input(slots, `Name of the ${kind} ${next.name}`)));
  }
  await button(slots, "Close").last().click();

  await page.goto(`/deck/${shared.deck.id}`);
  await page.getByRole("button", { name: /^Share(?: ·|$)/ }).click();
  const sharing = page.getByRole("dialog", { name: "Share this deck", exact: true });
  const pendingShare = await holdFailure(`**/api/decks/${shared.deck.id}/sharing`, "PATCH");
  await button(sharing, "Private").click();
  await pendingShare.arrived;
  assert.ok(await button(sharing, "Done").isDisabled());
  await page.keyboard.press("Escape");
  await page.mouse.click(5, 5);
  assert.ok(await sharing.isVisible());
  pendingShare.release();
  await sharing.getByRole("alert").getByText("Fixture request failed").waitFor();
  for (const succeeds of [true, false]) {
    await page.evaluate(succeeds => {
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: { writeText: async () => { if (!succeeds) throw new Error("Fixture permission denied"); } },
      });
    }, succeeds);
    await button(sharing, "Copy link").click();
    await sharing.getByRole("status").getByText(succeeds ? "Link copied." : /^Could not copy the link/).waitFor();
  }
  console.log(`${engine} verified pending deletes and sharing, scoped validation, Escape/focus, and clipboard statuses.`);
} finally {
  await browser.close();
}
