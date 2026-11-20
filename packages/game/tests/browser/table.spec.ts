import { expect, test, type Request } from '@playwright/test';
import { isFixturePrint } from './fixture-images.ts';

test('read-only production table, decision jump and honest join boundary', async ({ page, browser }, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    const imageFailure = message.text().startsWith('Failed to load resource:') && message.location().url.startsWith('https://cards.scryfall.io/');
    if (message.type() === 'error' && !imageFailure) errors.push(message.text());
  });
  const response = await page.goto('/');
  expect(response?.headers()['content-security-policy']).toContain("connect-src 'self'");
  await expect(page.getByText('Not connected', { exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Commander table', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Jump to decision' }).click();
  await expect(page.getByRole('region', { name: 'Opening hand', exact: true })).toBeFocused();
  await expect(page.getByText('Read-only fixture. Choices cannot be submitted.')).toBeVisible();
  await page.getByRole('link', { name: 'Join', exact: true }).click();
  await expect(page.getByLabel('Single-seat bootstrap file')).toBeVisible();
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Join a Commander table' })).toBeVisible();
  await page.getByRole('link', { name: 'Inspect the fixture' }).click();
  await expect(page.getByRole('complementary', { name: 'Card inspector' })).toHaveCount(0);
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

test('failed print loads leave the permitted card readable and inspectable', async ({ page }) => {
  await page.route('https://cards.scryfall.io/**', route => route.fulfill({ status: 503 }));
  await page.goto('/');
  const hand = page.getByRole('region', { name: 'Your hand', exact: true });
  const card = hand.getByRole('button', { name: 'Inspect Plains', exact: true }).first();
  await expect(card.locator('img')).toHaveCount(0);
  await expect(card.getByText('Basic Land — Plains')).toBeVisible();
  await card.focus();
  await page.keyboard.press('Enter');
  const inspector = page.getByRole('complementary', { name: 'Card inspector' });
  await expect(inspector.getByText('({T}: Add {W}.)', { exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(card).toBeFocused();
});

test('inspection build makes no API requests and persists no session data', async ({ page }) => {
  await page.addInitScript(() => { Reflect.deleteProperty(URL, 'canParse'); });
  const requests: Request[] = [];
  page.on('request', request => requests.push(request));
  await page.goto('/');
  await page.getByRole('link', { name: 'About this build' }).click();
  await expect(page.getByRole('heading', { name: 'A table with opening choices' })).toBeVisible();
  expect(requests.some(isFixturePrint)).toBe(true);
  expect(requests.every(request => isFixturePrint(request) ||
    (new URL(request.url()).origin === new URL(page.url()).origin && !/\/(v1|presentation)\/|client-config/.test(request.url())))).toBe(true);
  expect(await page.evaluate(async () => ({ local: localStorage.length, session: sessionStorage.length, databases: (await indexedDB.databases()).length, serviceWorkers: (await navigator.serviceWorker.getRegistrations()).length }))).toEqual({ local: 0, session: 0, databases: 0, serviceWorkers: 0 });
});

test('desktop workspace keeps the hand in view while switching between decisions and inspection', async ({ page }, testInfo) => {
  for (const viewport of [{ width: 1280, height: 720 }, { width: 2560, height: 1440 }]) {
    await page.setViewportSize(viewport);
    await page.goto('/');
    await expect(page.getByRole('region', { name: 'Players' }).locator('img')).toHaveCount(0);
    const hand = page.getByRole('region', { name: 'Your hand', exact: true });
    const choice = page.getByRole('region', { name: 'Opening hand', exact: true });
    const inspector = page.getByRole('complementary', { name: 'Card inspector' });
    await expect(hand.getByRole('heading')).toBeInViewport({ ratio: 1 });
    await expect(choice).toBeInViewport({ ratio: 1 });
    await expect(page.getByRole('region', { name: 'Command zone', exact: true }).getByRole('button')).toBeInViewport({ ratio: 1 });
    // Reaching the last same-name card must scroll only its hand, leaving the
    // current choice accessible. Selection still uses its exact object handle.
    const last = hand.getByRole('button').last();
    await last.focus(); await page.keyboard.press('Enter');
    await expect(last).toHaveAttribute('aria-pressed', 'true');
    await expect(inspector.getByRole('heading', { name: 'Plains', exact: true })).toBeFocused();
    await expect(choice).toHaveCount(0);
    await expect(hand.getByRole('heading')).toBeInViewport({ ratio: 1 });
    await page.getByRole('button', { name: 'Jump to decision' }).click();
    await expect(choice).toBeFocused();
    await expect(inspector).toHaveCount(0);
    await last.click();
    await expect(inspector.getByRole('heading', { name: 'Plains', exact: true })).toBeInViewport({ ratio: 1 });
    await inspector.getByText('Source & identity', { exact: true }).click();
    await expect(inspector.getByText('Object:', { exact: false })).toContainText('Object:');
    await inspector.getByRole('button', { name: 'Close', exact: true }).click();
    await expect(last).toBeFocused();
    await expect(choice).toBeInViewport({ ratio: 1 });
    await page.screenshot({ path: testInfo.outputPath(`workspace-${viewport.width}.png`), fullPage: true });
  }
});

test('intermediate and narrow windows retain decision jump and inspection focus return', async ({ page }) => {
  for (const width of [960, 800, 640, 375]) {
    await page.setViewportSize({ width, height: 720 });
    await page.goto('/');
    const card = page.getByRole('region', { name: 'Your hand', exact: true }).getByRole('button').last();
    await card.focus(); await page.keyboard.press('Enter');
    const inspector = page.getByRole('complementary', { name: 'Card inspector' });
    await expect(inspector.getByRole('heading', { name: 'Plains', exact: true })).toBeFocused();
    await page.getByRole('button', { name: 'Jump to decision' }).click();
    await expect(page.getByRole('region', { name: 'Opening hand', exact: true })).toBeFocused();
    await expect(inspector).toHaveCount(0);
    await card.click();
    await inspector.getByRole('button', { name: 'Close', exact: true }).click();
    await expect(card).toBeFocused();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  }
});
