import { expect, test } from '@playwright/test';

test('captured mulligan completes by keyboard with explicit local-only confirmation', async ({ page, browser }, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('/fixtures/decisions');
  const captured = page.getByRole('region', { name: 'Captured foundation choice' });
  await expect(captured.getByText(/capture source 5ff4fd94/)).toBeVisible();
  await captured.getByText('Captured hand · 7 objects', { exact: true }).click();
  await expect(captured.getByRole('listitem')).toHaveCount(7);
  const confirm = captured.getByRole('button', { name: 'Confirm local draft' });
  await expect(confirm).toBeDisabled();
  const keep = captured.getByRole('radio', { name: 'Keep hand', exact: true });
  await page.keyboard.press('Tab');
  await expect(captured.getByRole('link', { name: 'Inspect the captured cards and source text' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(keep).toBeFocused();
  expect(await keep.evaluate(element => getComputedStyle(element).outlineStyle)).not.toBe('none');
  await page.keyboard.press('Space');
  await page.keyboard.press('ArrowDown');
  await expect(captured.getByRole('radio', { name: 'Take mulligan' })).toBeChecked();
  await page.keyboard.press('Tab');
  await expect(confirm).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByText('"take": true')).toBeVisible();
  await expect(captured.getByText('Local draft only. No game action is sent.')).toBeVisible();
  expect(errors).toEqual([]);
  await testInfo.attach('browser-version', { body: browser.version(), contentType: 'text/plain' });
  await page.screenshot({ path: testInfo.outputPath('decision-gallery.png'), fullPage: true });
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
  const earlier = form.getByRole('button', { name: 'Move lab-b earlier', exact: true });
  await earlier.focus(); await page.keyboard.press('Enter');
  await expect(earlier).toBeFocused();
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
  await expect(form.locator('..').getByRole('button', { name: 'Confirm local draft' })).toBeDisabled();
  const gaps = page.getByRole('region', { name: 'Explicit contract gaps' });
  await expect(gaps.getByRole('heading', { name: 'Decision context unavailable' })).toBeVisible();
  await expect(gaps.getByRole('button')).toHaveCount(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('decision-gallery-narrow.png'), fullPage: true });
});

test('fixture route refresh makes no API request and stores no secrets or intent', async ({ page }) => {
  const requests: string[] = [];
  page.on('request', request => requests.push(request.url()));
  const response = await page.goto('/fixtures/decisions');
  expect(response?.headers()['content-security-policy']).toContain("form-action 'none'");
  await page.getByRole('region', { name: 'Captured foundation choice' }).getByRole('radio', { name: 'Keep hand', exact: true }).check();
  await page.reload();
  await expect(page.getByRole('region', { name: 'Captured foundation choice' }).getByRole('radio', { name: 'Keep hand', exact: true })).not.toBeChecked();
  expect(requests.every(url => new URL(url).origin === 'http://127.0.0.1:4179' && !/\/(v1|presentation)\/|client-config/.test(url))).toBe(true);
  expect(await page.evaluate(async () => ({ local: localStorage.length, session: sessionStorage.length, databases: (await indexedDB.databases()).length, workers: (await navigator.serviceWorker.getRegistrations()).length, cache: (await caches.keys()).length, cookies: document.cookie }))).toEqual({ local: 0, session: 0, databases: 0, workers: 0, cache: 0, cookies: '' });
});
