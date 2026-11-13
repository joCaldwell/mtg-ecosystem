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
  await expect(page.getByRole('region', { name: 'No decision supplied' })).toBeFocused();
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
  await expect(page.getByRole('region', { name: 'No decision supplied' })).toBeFocused();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('narrow.png'), fullPage: true });
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
