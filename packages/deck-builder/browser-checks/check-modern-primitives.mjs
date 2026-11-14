// Browser contracts for shared primitives against a disposable loopback fixture.
import assert from "node:assert/strict";
import { chromium, firefox, webkit } from "playwright-core";

const index = process.argv.indexOf("--origin");
const origin = index < 0 ? "http://127.0.0.1:8872" : process.argv[index + 1];
const address = new URL(origin);
assert.equal(address.protocol, "http:");
assert.ok(["127.0.0.1", "localhost", "[::1]"].includes(address.hostname));
const browserIndex = process.argv.indexOf("--browser");
const browserName = browserIndex < 0 ? "chromium" : process.argv[browserIndex + 1];
assert.ok(["chromium", "firefox", "webkit"].includes(browserName));
const browser = await ({ chromium, firefox, webkit })[browserName].launch({ headless: true });
const passed = [];
async function check(name, run) { await run(); passed.push(name); console.log(`Passed: ${name}`); }
async function absent(locator) { await locator.waitFor({ state: "hidden" }); }
async function focused(locator) { await locator.evaluate(element => element.focus()); assert.ok(await locator.evaluate(element => element === document.activeElement)); }

try {
  for (const fallback of [false, true]) {
    const context = await browser.newContext({ baseURL: origin, viewport: { width: 1440, height: 900 }, reducedMotion: "reduce" });
    await context.addCookies([{ name: "checks_user", value: "1", url: origin }]);
    if (fallback) await context.addInitScript(() => {
      // Exercise the retained, no-dependency fallback independently of browser age.
      delete HTMLElement.prototype.popover;
      delete HTMLElement.prototype.focusGroup;
      delete HTMLDialogElement.prototype.closedBy;
      const setAttribute = Element.prototype.setAttribute;
      HTMLDialogElement.prototype.setAttribute = function (name, value) {
        if (name === "closedby") return;
        setAttribute.call(this, name, value);
      };
      const supports = CSS.supports.bind(CSS);
      CSS.supports = (...args) => args.join(":").includes("position-area") ? false : supports(...args);
    });
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", error => errors.push(error.message));
    await page.goto("/design");
    await page.getByRole("heading", { name: "Design", exact: true }).waitFor();
    const mode = fallback ? "fallback" : "native";

    await check(`${mode}: modal is native, autofocused, inert outside, and restores focus`, async () => {
      const trigger = page.getByRole("button", { name: "Dialog md", exact: true });
      const overflow = await page.evaluate(() => document.body.style.overflow);
      await trigger.click();
      const dialog = page.getByRole("dialog", { name: "Rename deck", exact: true });
      await dialog.waitFor();
      assert.equal(await dialog.evaluate(element => element.matches("dialog:modal")), true);
      assert.equal(await dialog.getByRole("textbox").evaluate(element => element === document.activeElement), true);
      await trigger.evaluate(element => element.focus());
      assert.equal(await dialog.evaluate(element => element.contains(document.activeElement)), true, "Outside content cannot take focus");
      for (let i = 0; i < 8; i++) {
        await page.keyboard.press("Tab");
        assert.ok(await dialog.evaluate(element => element.contains(document.activeElement) || document.activeElement === document.body));
      }
      await page.keyboard.press("Escape"); await absent(dialog);
      assert.ok(await trigger.evaluate(element => element === document.activeElement));
      assert.equal(await page.evaluate(() => document.body.style.overflow), overflow, "No body overflow mutation");
      await trigger.click(); await dialog.waitFor();
      const field = await dialog.getByRole("textbox").boundingBox();
      await page.mouse.move(field.x + field.width / 2, field.y + field.height / 2);
      await page.mouse.down(); await page.mouse.move(3, 3); await page.mouse.up();
      assert.ok(await dialog.isVisible(), "Dragging a selection onto the backdrop keeps the dialog open");
      await page.mouse.click(3, 3); await absent(dialog);
    });

    await check(`${mode}: destructive confirmation autofocuses the safe action`, async () => {
      await page.getByRole("button", { name: "Confirm, danger", exact: true }).click();
      const dialog = page.getByRole("dialog", { name: "Delete this deck?", exact: true });
      await dialog.waitFor();
      assert.ok(await dialog.getByRole("button", { name: "Cancel", exact: true }).evaluate(element => element === document.activeElement));
      await page.keyboard.press("Escape"); await absent(dialog);
    });

    await check(`${mode}: menu keyboard and pointer contracts`, async () => {
      const trigger = page.getByRole("button", { name: "Actions", exact: true });
      await trigger.click();
      const menu = page.locator('[focusgroup~="menu"]').first();
      await menu.waitFor();
      if (!fallback && browserName === "chromium") {
        // Playwright's old DOM role query does not understand native focusgroup
        // inference yet; verify the browser's actual accessibility tree instead.
        const client = await context.newCDPSession(page);
        const { nodes } = await client.send("Accessibility.getFullAXTree");
        assert.ok(nodes.some(node => node.role?.value === "menu" && node.name?.value === "Actions"));
        assert.ok(nodes.some(node => node.role?.value === "menuitem" && node.name?.value === "Rename"));
        await client.detach();
      }
      assert.ok(await trigger.evaluate(element => element === document.activeElement), "Pointer open stays at trigger");
      await trigger.press("ArrowDown");
      assert.equal(await page.evaluate(() => document.activeElement.textContent.trim()), "Rename");
      await page.keyboard.press("ArrowDown");
      assert.equal(await page.evaluate(() => document.activeElement.textContent.trim()), "Duplicate");
      await page.keyboard.press("ArrowDown");
      assert.equal(await page.evaluate(() => document.activeElement.textContent.trim()), "Delete deck", "Disabled items are skipped");
      await page.keyboard.press("Escape"); await absent(menu);
      assert.ok(await trigger.evaluate(element => element === document.activeElement));
      await trigger.press("ArrowUp"); await menu.waitFor();
      assert.equal(await page.evaluate(() => document.activeElement.textContent.trim()), "Delete deck");
      await page.keyboard.press("r");
      assert.equal(await page.evaluate(() => document.activeElement.textContent.trim()), "Rename");
      await page.keyboard.press("Tab"); await absent(menu);
      assert.ok(await page.evaluate(() => document.activeElement?.isConnected));
    });

    await check(`${mode}: multiselect uses real searchable checkboxes`, async () => {
      const trigger = page.getByRole("button", { name: "Select tags…, Example tags", exact: true });
      await trigger.click();
      const search = page.getByRole("searchbox", { name: "Search example tags", exact: true });
      await search.waitFor();
      await search.fill("Engine");
      const option = page.getByRole("checkbox", { name: "Engine piece", exact: true });
      await option.check();
      assert.ok(await option.isChecked());
      await page.keyboard.press("Escape"); await absent(search);
      assert.ok(await trigger.evaluate(element => element === document.activeElement));
    });

    await check(`${mode}: field preserves explicit ID and existing descriptions`, async () => {
      const field = page.getByRole("textbox", { name: "Explicit field ID", exact: true });
      await field.waitFor();
      const association = await field.evaluate(element => ({
        id: element.id,
        label: element.labels?.[0]?.htmlFor,
        descriptions: element.getAttribute("aria-describedby").split(/\s+/).map(id => document.getElementById(id)?.textContent),
      }));
      assert.equal(association.id, association.label);
      assert.deepEqual(association.descriptions, ["Existing description from the caller.", "The field keeps its own ID and both descriptions."]);
      await page.getByText("Explicit field ID", { exact: true }).click();
      assert.ok(await field.evaluate(element => element === document.activeElement));
    });

    await check(`${mode}: typography scales and meaningful boundaries retain contrast`, async () => {
      const result = await page.evaluate(() => {
        const root = document.documentElement;
        const baseline = parseFloat(getComputedStyle(document.body).fontSize);
        root.style.fontSize = "200%";
        const enlarged = parseFloat(getComputedStyle(document.body).fontSize);
        root.style.removeProperty("font-size");
        const style = getComputedStyle(root);
        const rgb = hex => hex.trim().replace("#", "").match(/.{2}/g).map(value => parseInt(value, 16) / 255).map(value => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4);
        const luminance = hex => rgb(hex).reduce((sum, value, index) => sum + value * [.2126, .7152, .0722][index], 0);
        const border = luminance(style.getPropertyValue("--control-line"));
        const background = luminance(style.getPropertyValue("--raised"));
        return { baseline, enlarged, contrast: (border + .05) / (background + .05) };
      });
      assert.equal(result.enlarged, result.baseline * 2);
      assert.ok(result.contrast >= 3, `Control boundary contrast ${result.contrast}`);
    });

    await check(`${mode}: combobox remains reachable above a native modal`, async () => {
      await page.goto("/decks");
      await page.locator("main").getByRole("button", { name: "New deck", exact: true }).click();
      const dialog = page.getByRole("dialog", { name: "New deck", exact: true });
      await dialog.waitFor();
      const combo = dialog.getByRole("combobox");
      await combo.click();
      const list = dialog.getByRole("listbox");
      await list.waitFor();
      assert.ok(await list.evaluate(element => {
        const rect = element.getBoundingClientRect();
        return element.contains(document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2));
      }), "List paints over the modal");
      await page.keyboard.press("Escape"); await absent(list);
      assert.ok(await dialog.isVisible(), "First Escape dismisses only the dropdown");
      await page.keyboard.press("Escape"); await absent(dialog);
    });
    assert.deepEqual(errors, []);
    await context.close();
  }
  console.log(`Modern primitive checks passed (${passed.length}). ${browserName} ${browser.version()}`);
} finally { await browser.close(); }
