import { expect, test } from '@playwright/test';

test('route navigation names the view and moves focus without changing fixture data', async ({ page }) => {
  await page.goto('/');
  await expect(page).toHaveTitle('Commander table · Multiverse Commander');
  await page.getByRole('link', { name: 'Join', exact: true }).click();
  await expect(page).toHaveTitle('Join a Commander table · Multiverse Commander');
  await expect(page.getByRole('heading', { name: 'Join a Commander table' })).toBeFocused();
  await page.getByRole('link', { name: 'Decision gallery', exact: true }).click();
  await expect(page).toHaveTitle('Decision drafts · Multiverse Commander');
  await expect(page.getByRole('heading', { name: 'Decision drafts' })).toBeFocused();
  await page.getByRole('link', { name: 'Try opening choices on the table' }).click();
  await expect(page).toHaveTitle('Opening hand preview · Multiverse Commander');
  await expect(page.getByRole('heading', { name: 'Opening hand preview' })).toBeFocused();
  await page.goBack();
  await expect(page.getByRole('heading', { name: 'Decision drafts' })).toBeFocused();
});

test('incomplete decisions expose an actionable error on confirmation, without per-key live announcements', async ({ page }) => {
  await page.goto('/fixtures/decisions');
  const form = page.getByRole('form', { name: 'Opening hand', exact: true });
  const captured = page.getByRole('region', { name: 'Captured foundation choice' });
  const confirmation = captured.getByRole('button', { name: 'Confirm local draft' });
  await confirmation.click();
  const summary = captured.locator('p[tabindex="-1"]');
  await expect(summary).toBeFocused();
  await expect(summary).toContainText('Choose 1 item(s).');
  await expect(page.getByRole('region', { name: 'Last local completion' })).toContainText('No local draft confirmed.');
  const keep = captured.getByRole('radio', { name: 'Keep hand', exact: true });
  await keep.check();
  await confirmation.click();
  await expect(page.getByRole('region', { name: 'Last local completion' })).toContainText('"take": false');
  const quantity = page.getByRole('group', { name: 'Exact quantity bookkeeping' });
  const input = quantity.getByRole('textbox', { name: 'Exact quantity' });
  await input.fill('1e20');
  await expect(input).not.toHaveAttribute('aria-invalid', 'true');
  expect(await quantity.locator('..').locator('[aria-live]').count()).toBe(0);
  await page.keyboard.press('Enter');
  await expect(input).toHaveAttribute('aria-invalid', 'true');
  await input.fill('900719925474099312345678901234567890');
  await expect(input).not.toHaveAttribute('aria-invalid', 'true');
  await page.keyboard.press('Enter');
  await expect(page.getByText('Synthetic exact quantity: 900719925474099312345678901234567890', { exact: true })).toBeVisible();
  // The form is named by its own visible legend, even among several drafts.
  await expect(form).toHaveCount(1);
});

test('shared game table responds to its embedding container in a wide window', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('/');
  const tableHeading = page.getByRole('heading', { name: 'Commander table', exact: true });
  await tableHeading.evaluate(heading => {
    const table = heading.parentElement?.parentElement?.parentElement;
    const container = table?.parentElement;
    if (container) container.style.inlineSize = '600px';
  });
  const hand = page.getByRole('region', { name: 'Your hand', exact: true });
  const decision = page.getByRole('region', { name: 'Opening hand', exact: true });
  const inspector = page.getByRole('complementary', { name: 'Card inspector' });
  const handBox = await hand.boundingBox();
  const decisionBox = await decision.boundingBox();
  const inspectorBox = await inspector.boundingBox();
  expect(handBox && decisionBox && inspectorBox).toBeTruthy();
  expect(Math.abs(handBox!.x - decisionBox!.x)).toBeLessThan(1);
  expect(Math.abs(handBox!.width - decisionBox!.width)).toBeLessThan(1);
  expect(decisionBox!.y).toBeGreaterThan(handBox!.y);
  expect(inspectorBox!.y).toBeGreaterThan(decisionBox!.y);
  await hand.getByRole('button').last().click();
  await expect(inspector.getByRole('heading', { name: 'Plains', exact: true })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(hand.getByRole('button').last()).toBeFocused();
});

test('text scaling reflows the table and keeps labels and controls inside the page', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto('/');
  await page.evaluate(() => { document.documentElement.style.fontSize = '200%'; });
  const heading = page.getByRole('heading', { name: 'Commander table', exact: true });
  expect(await heading.evaluate(element => parseFloat(getComputedStyle(element).fontSize))).toBeGreaterThanOrEqual(30);
  await page.getByRole('button', { name: 'Jump to decision' }).click();
  await expect(page.getByRole('region', { name: 'Opening hand', exact: true })).toBeFocused();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole('link', { name: 'Decision gallery', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Exact quantity' });
  expect(await input.evaluate(element => parseFloat(getComputedStyle(element).fontSize))).toBeGreaterThanOrEqual(32);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('forced colors preserves selected state and focus without relying on shadows', async ({ page }) => {
  await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  await page.goto('/');
  const card = page.getByRole('region', { name: 'Your hand', exact: true }).getByRole('button').last();
  await card.click();
  await expect(card).toHaveAttribute('aria-pressed', 'true');
  const outline = await card.evaluate(element => ({ width: getComputedStyle(element).outlineWidth, style: getComputedStyle(element).outlineStyle }));
  expect(outline).toEqual({ width: '2px', style: 'solid' });
  await page.keyboard.press('Escape');
  await expect(card).toBeFocused();
  expect(await card.evaluate(element => getComputedStyle(element).transitionDuration)).not.toBe('0s');
});

test('all repeated decision controls and table regions retain unique label targets', async ({ page }) => {
  for (const route of ['/', '/fixtures/decisions', '/join']) {
    await page.goto(route);
    expect(await page.evaluate(() => {
      const ids = [...document.querySelectorAll('[id]')].map(element => element.id);
      const missing = [...document.querySelectorAll('[aria-labelledby], [aria-describedby], label[for]')].flatMap(element => {
        const references = [element.getAttribute('aria-labelledby'), element.getAttribute('aria-describedby'), element.getAttribute('for')].filter(Boolean).join(' ').split(/\s+/).filter(Boolean);
        return references.filter(id => !document.getElementById(id));
      });
      return { duplicates: ids.filter((id, index) => ids.indexOf(id) !== index), missing };
    })).toEqual({ duplicates: [], missing: [] });
  }
});


test('entry controls maintain baseline contrast and coarse-pointer target sizes', async ({ page, browser }) => {
  await page.goto('/fixtures/decisions');
  const input = page.getByRole('textbox', { name: 'Exact quantity' });
  const contrast = await input.evaluate(element => {
    const style = getComputedStyle(element);
    const background = style.backgroundColor.match(/[\d.]+/g)!.slice(0, 3).map(Number);
    const border = style.borderTopColor.match(/[\d.]+/g)!.map(Number);
    const alpha = border[3] ?? 1;
    const composite = border.slice(0, 3).map((value, index) => value * alpha + background[index]! * (1 - alpha));
    const luminance = (channels: number[]) => channels.map(value => value / 255).map(value => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4).reduce((sum, value, index) => sum + value * [0.2126, 0.7152, 0.0722][index]!, 0);
    return (Math.max(luminance(composite), luminance(background)) + .05) / (Math.min(luminance(composite), luminance(background)) + .05);
  });
  expect(contrast).toBeGreaterThanOrEqual(3);
  const context = await browser.newContext({ hasTouch: true, viewport: { width: 1280, height: 900 } });
  const touchPage = await context.newPage();
  await touchPage.goto(new URL('/fixtures/decisions', page.url()).href);
  expect(await touchPage.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(true);
  const keep = touchPage.getByRole('region', { name: 'Captured foundation choice' }).getByRole('radio', { name: 'Keep hand', exact: true });
  expect(await keep.locator('..').evaluate(element => element.getBoundingClientRect().height)).toBeGreaterThanOrEqual(48);
  const confirm = touchPage.getByRole('region', { name: 'Captured foundation choice' }).getByRole('button', { name: 'Confirm local draft' });
  expect(await confirm.evaluate(element => element.getBoundingClientRect().height)).toBeGreaterThanOrEqual(48);
  await context.close();
});
