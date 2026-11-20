import { expect, test, type Request } from '@playwright/test';
import { isFixturePrint } from './fixture-images.ts';

test('captured mulligan completes by keyboard with explicit local-only confirmation', async ({ page, browser }, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('/fixtures/decisions');
  const captured = page.getByRole('region', { name: 'Captured foundation choice' });
  await captured.getByText('Capture provenance', { exact: true }).click();
  await expect(captured.getByText(/capture source 5ff4fd94/)).toBeVisible();
  await captured.getByText('Captured hand · 7 objects', { exact: true }).click();
  await expect(captured.getByRole('listitem')).toHaveCount(7);
  const confirm = captured.getByRole('button', { name: 'Confirm local draft' });
  await expect(confirm).toBeEnabled();
  const keep = captured.getByRole('radio', { name: 'Keep hand', exact: true });
  await captured.getByRole('link', { name: 'Open the captured table' }).focus();
  await page.keyboard.press('Tab');
  await expect(keep).toBeFocused();
  expect(await keep.evaluate(element => getComputedStyle(element).outlineStyle)).not.toBe('none');
  await page.keyboard.press('Space');
  await page.keyboard.press('ArrowDown');
  await expect(captured.getByRole('radio', { name: 'Take mulligan' })).toBeChecked();
  const card = captured.getByRole('button', { name: 'Inspect Plains', exact: true }).nth(2);
  await card.focus(); await page.keyboard.press('Enter');
  const inspector = page.getByRole('complementary', { name: 'Card inspector' });
  await expect(inspector.getByRole('heading', { name: 'Plains', exact: true })).toBeFocused();
  await expect(captured.getByRole('radio', { name: 'Take mulligan' })).toBeChecked();
  await expect(confirm).toBeEnabled();
  await page.screenshot({ path: testInfo.outputPath('gallery-inspecting-draft.png'), fullPage: true });
  // Collapsing the source hand must not strand focus when inspection closes.
  await captured.getByText('Captured hand · 7 objects', { exact: true }).click();
  await inspector.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(card).toBeFocused();
  await expect(captured.getByRole('radio', { name: 'Take mulligan' })).toBeChecked();
  await captured.getByRole('radio', { name: 'Take mulligan' }).focus();
  await page.keyboard.press('Tab');
  await expect(confirm).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByText('"take": true')).toBeVisible();
  await expect(captured.getByText('Local draft only. No game action is sent.')).toBeVisible();
  expect(errors).toEqual([]);
  await testInfo.attach('browser-version', { body: browser.version(), contentType: 'text/plain' });
  await page.screenshot({ path: testInfo.outputPath('decision-gallery.png'), fullPage: true });
});

test('opening preview keeps a deliberate choice while inspecting and never sends it', async ({ page }, testInfo) => {
  const requests: Request[] = [];
  page.on('request', request => requests.push(request));
  await page.goto('/fixtures/decisions');
  await page.getByRole('link', { name: 'Try opening choices on the table' }).click();
  await expect(page.getByRole('heading', { name: 'Opening hand preview', exact: true })).toBeVisible();
  const decision = page.getByRole('region', { name: 'Opening hand', exact: true });
  const confirm = decision.getByRole('button', { name: 'Confirm preview choice', exact: true });
  await expect(confirm).toBeEnabled();
  const keep = decision.getByRole('radio', { name: 'Keep hand', exact: true });
  await keep.check();
  const card = page.getByRole('region', { name: 'Your hand', exact: true }).getByRole('button').last();
  await card.focus(); await page.keyboard.press('Enter');
  const inspector = page.getByRole('complementary', { name: 'Card inspector' });
  await expect(inspector.getByRole('heading', { name: 'Plains', exact: true })).toBeFocused();
  await expect(keep).toBeChecked();
  await page.keyboard.press('Escape'); await expect(card).toBeFocused();
  await confirm.click();
  await expect(decision.getByRole('status').filter({ hasText: 'Preview confirmed: keep this hand.' })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Your hand', exact: true }).getByRole('button')).toHaveCount(7);
  await page.screenshot({ path: testInfo.outputPath('opening-preview.png'), fullPage: true });
  await page.reload();
  await expect(keep).not.toBeChecked();
  await expect(confirm).toBeEnabled();
  expect(requests.every(request => isFixturePrint(request) ||
    (new URL(request.url()).origin === new URL(page.url()).origin && !/\/(v1|presentation)\/|client-config/.test(request.url())))).toBe(true);
  expect(await page.evaluate(async () => ({ local: localStorage.length, session: sessionStorage.length, databases: (await indexedDB.databases()).length }))).toEqual({ local: 0, session: 0, databases: 0 });
});

test('same-label candidates, missing metadata and supplied counts retain distinct identity', async ({ page }) => {
  await page.goto('/fixtures/decisions');
  const form = page.getByRole('group', { name: 'Choose cards to discard' });
  const first = form.getByRole('checkbox', { name: 'Same label Handle lab-a', exact: true });
  const second = form.getByRole('checkbox', { name: 'Same label Handle lab-b', exact: true });
  const missing = form.getByRole('checkbox', { name: 'Unidentified object Handle lab-c', exact: true });
  await first.focus(); await page.keyboard.press('Space');
  await expect(first).toBeChecked(); await expect(second).not.toBeChecked();
  await expect(form.getByText('Selected 1 · Required 2')).toBeVisible();
  await missing.focus(); await page.keyboard.press('Space');
  const confirm = form.locator('..').getByRole('button', { name: 'Confirm local draft' });
  await confirm.click();
  const receipt = page.getByRole('region', { name: 'Last local completion' });
  await expect(receipt).toContainText('"lab-a"'); await expect(receipt).toContainText('"lab-c"');
  await expect(receipt).not.toContainText('"lab-b"');
});

test('keyboard ordering preserves chosen handle order and deliberate focus', async ({ page }) => {
  await page.goto('/fixtures/decisions');
  const form = page.getByRole('group', { name: 'Order cards for the bottom of your library' });
  await form.getByRole('checkbox', { name: 'Same label Handle lab-a', exact: true }).check();
  await form.getByRole('checkbox', { name: 'Same label Handle lab-b', exact: true }).check();
  const first = form.getByRole('listitem').filter({ hasText: 'Handle lab-a' });
  const second = form.getByRole('listitem').filter({ hasText: 'Handle lab-b' });
  const later = first.getByRole('button', { name: /^Later / });
  const earlier = second.getByRole('button', { name: /^Earlier / });
  await expect(later).toHaveAccessibleName('Later Same label position 1 of 2');
  await expect(earlier).toHaveAccessibleName('Earlier Same label position 2 of 2');
  await later.focus(); await page.keyboard.press('Enter');
  await expect(later).toBeFocused();
  await expect(later).toHaveAccessibleName('Later Same label position 2 of 2');
  await expect(form.getByRole('status')).toHaveText('Same label moved to position 2 of 2.');
  await expect(earlier).toHaveAttribute('aria-disabled', 'true');
  const returnEarlier = first.getByRole('button', { name: /^Earlier / });
  await returnEarlier.focus(); await page.keyboard.press('Enter');
  await expect(returnEarlier).toBeFocused();
  await expect(form.getByRole('status')).toHaveText('Same label moved to position 1 of 2.');
  await earlier.focus(); await page.keyboard.press('Enter');
  await expect(earlier).toHaveAttribute('aria-disabled', 'true');
  await form.locator('..').getByRole('button', { name: 'Confirm local draft' }).click();
  await expect(page.getByRole('region', { name: 'Last local completion' })).toContainText('"objects": [\n      "lab-b",\n      "lab-a"');
});

test('remote-shaped identity and candidate updates announce, discard intent and preserve stable focus', async ({ page }) => {
  await page.goto('/fixtures/decisions');
  const form = page.getByRole('group', { name: 'Choose cards to discard' });
  const first = form.getByRole('checkbox', { name: 'Same label Handle lab-a', exact: true });
  const second = form.getByRole('checkbox', { name: 'Same label Handle lab-b', exact: true });
  await first.check(); await second.check(); await second.focus();
  // Programmatic fixture update deliberately preserves the existing active
  // element, modeling parent-delivered immutable remote props. No game I/O.
  await page.getByRole('button', { name: 'Update candidate set' }).evaluate(element => { if (element instanceof HTMLButtonElement) element.click(); });
  await expect(second).toBeFocused(); await expect(second).toBeChecked();
  await expect(form.getByText('Selected 1 · Required 2')).toBeVisible();
  await expect(page.getByRole('status').filter({ hasText: 'Decision options updated' })).toHaveCount(2);
  await page.getByRole('button', { name: 'Replace decision identity' }).evaluate(element => { if (element instanceof HTMLButtonElement) element.click(); });
  await expect(second).toBeFocused(); await expect(second).not.toBeChecked();
  await expect(page.getByRole('status').filter({ hasText: 'Previous draft discarded' })).toHaveCount(3);
  await expect(form.getByText('Selected 0 · Required 2')).toBeVisible();
});

test('exact large quantities, unsupported context, reduced motion and narrow layout remain accessible', async ({ page }, testInfo) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.setViewportSize({ width: 640, height: 800 });
  await page.goto('/fixtures/decisions');
  const form = page.getByRole('group', { name: 'Exact quantity bookkeeping' });
  const input = form.getByRole('textbox', { name: 'Exact quantity' });
  await input.fill('9007199254740993123456789');
  await page.keyboard.press('Enter');
  await expect(page.getByText('Synthetic exact quantity: 9007199254740993123456789', { exact: true })).toBeVisible();
  await input.fill('1e20');
  const confirm = form.locator('..').getByRole('button', { name: 'Confirm local draft' });
  await expect(confirm).toBeEnabled();
  await confirm.click();
  await expect(form.locator('..').locator('p[tabindex="-1"]')).toBeFocused();
  await expect(input).toHaveAttribute('aria-invalid', 'true');
  await expect(page.getByText('Synthetic exact quantity: 9007199254740993123456789', { exact: true })).toBeVisible();
  const gaps = page.getByRole('region', { name: 'Explicit contract gaps' });
  await expect(gaps.getByRole('heading', { name: 'Decision context unavailable' })).toBeVisible();
  await expect(gaps.getByRole('button')).toHaveCount(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('decision-gallery-narrow.png'), fullPage: true });
});

test('fixture route refresh makes no API request and stores no secrets or intent', async ({ page }) => {
  const requests: Request[] = [];
  page.on('request', request => requests.push(request));
  const response = await page.goto('/fixtures/decisions');
  expect(response?.headers()['content-security-policy']).toContain("form-action 'none'");
  await page.getByRole('region', { name: 'Captured foundation choice' }).getByRole('radio', { name: 'Keep hand', exact: true }).check();
  await page.reload();
  await expect(page.getByRole('region', { name: 'Captured foundation choice' }).getByRole('radio', { name: 'Keep hand', exact: true })).not.toBeChecked();
  expect(requests.every(request => isFixturePrint(request) ||
    (new URL(request.url()).origin === new URL(page.url()).origin && !/\/(v1|presentation)\/|client-config/.test(request.url())))).toBe(true);
  expect(await page.evaluate(async () => ({ local: localStorage.length, session: sessionStorage.length, databases: (await indexedDB.databases()).length, workers: (await navigator.serviceWorker.getRegistrations()).length, cache: (await caches.keys()).length, cookies: document.cookie }))).toEqual({ local: 0, session: 0, databases: 0, workers: 0, cache: 0, cookies: '' });
});

test('combat choices use keyboard selects, require explicit unused choices and retain exact targets', async ({ page }) => {
  await page.goto('/fixtures/decisions');
  const form = page.getByRole('group', { name: 'Declare attackers', exact: true }).locator('..');
  const first = form.getByRole('combobox', { name: 'Same label Handle lab-a', exact: true });
  const second = form.getByRole('combobox', { name: 'Same label Handle lab-b', exact: true });
  const confirm = form.getByRole('button', { name: 'Confirm local draft' });
  await confirm.click();
  await expect(form.locator('p[tabindex="-1"]')).toBeFocused();
  await first.focus();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(first).toHaveValue('[1]');
  await second.selectOption('unused');
  await confirm.click();
  const receipt = page.getByRole('region', { name: 'Last local completion' });
  await expect(receipt).toContainText('"response": "attackers"');
  await expect(receipt).toContainText('"lab-a",\n        1');
  await expect(receipt).not.toContainText('"lab-b"');
});

test('payment and combat amounts require explicit exact quantities and supplied allocation totals', async ({ page }) => {
  await page.goto('/fixtures/decisions');
  const payment = page.getByRole('group', { name: 'Choose mana to spend', exact: true }).locator('..');
  await payment.getByRole('textbox', { name: 'Blue Available: 1', exact: true }).fill('1');
  await payment.getByRole('textbox', { name: 'Colorless Available: 9007199254740993', exact: true }).fill('9007199254740993');
  await payment.getByRole('button', { name: 'Confirm local draft' }).click();
  const receipt = page.getByRole('region', { name: 'Last local completion' });
  await expect(receipt).toContainText('"C": "9007199254740993"');
  const damage = page.getByRole('group', { name: 'Assign combat damage', exact: true }).locator('..');
  const creature = damage.getByRole('textbox', { name: /Same label.*lab-b/ });
  const player = damage.getByRole('textbox', { name: 'Seat 3 Supplied player recipient', exact: true });
  await creature.fill('2'); await player.fill('2');
  await damage.getByRole('button', { name: 'Confirm local draft' }).click();
  await expect(damage.locator('p[tabindex="-1"]')).toHaveText('Assign exactly the supplied total of 5.');
  await expect(receipt).toContainText('"spend_mana"');
  await player.fill('3');
  await damage.getByRole('button', { name: 'Confirm local draft' }).click();
  await expect(receipt).toContainText('"response": "trample_damage"');
  await expect(receipt).toContainText('"amount": "3"');
});
