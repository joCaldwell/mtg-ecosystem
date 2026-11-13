import { expect, test } from '@playwright/test';

test('read-only production table, decision jump and honest join boundary', async ({ page, browser }, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  const response = await page.goto('/');
  expect(response?.headers()['content-security-policy']).toContain("connect-src 'none'");
  await expect(page.getByText('Not connected', { exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Commander table', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Jump to decision' }).click();
  await expect(page.getByRole('region', { name: 'Opening hand', exact: true })).toBeFocused();
  await expect(page.getByText('Read-only fixture. Choices cannot be submitted.')).toBeVisible();
  await page.getByRole('link', { name: 'Join', exact: true }).click();
  await expect(page.getByText('Live connections are not available in this build.')).toBeVisible();
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Join a Commander table' })).toBeVisible();
  await page.getByRole('link', { name: 'Return to inspection table' }).click();
  await expect(page.getByRole('complementary', { name: 'Card inspector' })).toBeVisible();
  expect(errors).toEqual([]);
  await testInfo.attach('browser-version', { body: browser.version(), contentType: 'text/plain' });
  await page.screenshot({ path: testInfo.outputPath('desktop.png'), fullPage: true });
});

test('keyboard navigation, reduced motion and narrow viewport retain access', async ({ page }, testInfo) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.setViewportSize({ width: 640, height: 720 });
  await page.goto('/');
  await page.keyboard.press('Tab');
  await expect(page.getByRole('link', { name: 'Skip to table content' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('main')).toBeFocused();
  const jump = page.getByRole('button', { name: 'Jump to decision' });
  await jump.focus(); await page.keyboard.press('Enter');
  await expect(page.getByRole('region', { name: 'Opening hand', exact: true })).toBeFocused();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('narrow.png'), fullPage: true });
});

test('source-backed card inspection preserves same-name identity and restores keyboard focus', async ({ page }, testInfo) => {
  await page.goto('/');
  const copies = page.getByRole('button', { name: 'Inspect Plains', exact: true });
  await expect(copies).toHaveCount(7);
  await copies.nth(2).focus(); await page.keyboard.press('Enter');
  const inspector = page.getByRole('complementary', { name: 'Card inspector' });
  await expect(inspector.getByRole('heading', { name: 'Plains', exact: true })).toBeFocused();
  await expect(inspector.getByText('({T}: Add {W}.)', { exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(copies.nth(2)).toBeFocused();
  await expect(copies.nth(2)).toHaveAttribute('aria-pressed', 'false');
  const otherCommander = page.getByRole('button', { name: 'Inspect Isamaru, Hound of Konda for Seat 3', exact: true });
  await otherCommander.click();
  await expect(inspector.getByText('Seat 3 · Command zone', { exact: true })).toBeVisible();
  await expect(inspector.getByText('No printed rules text', { exact: true })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('inspector.png'), fullPage: true });
  await inspector.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(otherCommander).toBeFocused();
});

test('omitted metadata shows identity fallback and observed characteristics without card invention', async ({ page }) => {
  await page.goto('/fixtures/missing-metadata');
  await page.getByRole('region', { name: 'Command zone', exact: true }).getByRole('button', { name: 'Inspect Unidentified card', exact: true }).click();
  const inspector = page.getByRole('complementary', { name: 'Card inspector' });
  await expect(inspector.getByRole('heading', { name: 'Unidentified card', exact: true })).toBeFocused();
  await expect(inspector.getByText('Source text is not available for this object.')).toBeVisible();
  await expect(inspector.getByText('Power', { exact: true })).toBeVisible();
  await expect(inspector.getByText('2', { exact: true })).toHaveCount(2);
  await expect(page.getByText('Isamaru, Hound of Konda', { exact: true })).toHaveCount(0);
});

test('inspection build makes no API requests and persists no session data', async ({ page }) => {
  const requests: string[] = [];
  page.on('request', request => requests.push(request.url()));
  await page.goto('/');
  await page.getByRole('link', { name: 'About this build' }).click();
  await expect(page.getByRole('heading', { name: 'An inspection surface' })).toBeVisible();
  expect(requests.every(url => new URL(url).origin === 'http://127.0.0.1:4179' && !url.includes('/v1/'))).toBe(true);
  expect(await page.evaluate(async () => ({ local: localStorage.length, session: sessionStorage.length, databases: (await indexedDB.databases()).length, serviceWorkers: (await navigator.serviceWorker.getRegistrations()).length }))).toEqual({ local: 0, session: 0, databases: 0, serviceWorkers: 0 });
});
