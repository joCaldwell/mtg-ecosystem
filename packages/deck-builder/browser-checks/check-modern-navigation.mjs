// Production product regressions on the disposable actor-scoped fixture.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { chromium, firefox } from 'playwright-core';
import { consentHtml, consentScript } from '../src/consent.ts';

const argument = (name, fallback) => { const at = process.argv.indexOf(`--${name}`); return at < 0 ? fallback : process.argv[at + 1]; };
const origin = argument('origin', 'http://127.0.0.1:8871');
assert.ok(['127.0.0.1', 'localhost', '[::1]'].includes(new URL(origin).hostname), 'Disposable loopback fixture only');
const out = resolve(argument('out', '.dev/modern-navigation'));
mkdirSync(out, { recursive: true });
const results = [];
for (const [name, engine] of [['chromium', chromium], ['firefox', firefox]]) {
  const browser = await engine.launch({ headless: true });
  try {
    const context = await browser.newContext({ baseURL: origin, viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' });
    await context.addCookies([{ name: 'checks_user', value: '1', url: origin }]);
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const check = async (label, run) => { await run(); results.push({ browser: name, check: label }); console.log(`Passed (${name}): ${label}`); };
    await check('SPA navigation names and focuses the new page exactly once', async () => {
      await page.goto('/');
      await page.getByRole('heading', { name: 'Home', exact: true }).waitFor();
      await page.getByRole('navigation', { name: 'Sections', exact: true }).getByRole('link', { name: 'Settings', exact: true }).click();
      await page.waitForFunction(() => document.title === 'Settings · Multiverse' && document.activeElement?.tagName === 'H1');
      const input = page.locator('main input').first();
      await input.focus();
      await page.evaluate(() => { const node = document.createElement('p'); node.textContent = 'Background update'; document.querySelector('main').append(node); node.remove(); });
      assert.equal(await input.evaluate(node => node === document.activeElement), true, 'Background updates preserve editing focus');
      await page.goBack();
      await page.waitForFunction(() => document.title === 'Home · Multiverse' && document.activeElement?.textContent === 'Home');
      await page.getByRole('link', { name: 'Start a game', exact: true }).click();
      await page.waitForFunction(() => document.title === 'New game · Multiverse' && document.activeElement?.tagName === 'H1');
      const select = page.getByRole('combobox', { name: 'Your deck', exact: true });
      await select.click();
      const option = page.getByRole('option').filter({ hasNotText: 'Choose' }).first();
      await option.click();
      assert.equal(await select.evaluate(node => node === document.activeElement), true, 'Changing selected deck is not page navigation');
      await page.getByRole('link', { name: 'Edit deck', exact: true }).click();
      await page.waitForFunction(() => document.activeElement?.tagName === 'H1' && document.title === `${document.activeElement.textContent.trim()} · Multiverse`);
    });
    await check('Native links to server resources retain document navigation', async () => {
      await page.evaluate(() => { const link = document.createElement('a'); link.href = '/api/session'; link.textContent = 'Read session document'; document.querySelector('main').append(link); });
      await page.getByRole('link', { name: 'Read session document', exact: true }).click();
      await page.waitForURL('**/api/session');
      assert.match(await page.locator('body').innerText(), /"mode"/);
    });
    await check('Hover previews can be entered, dismissed, and opened with keyboard focus', async () => {
      await page.goto('/deck/2');
      const row = page.locator('[data-card-row]').first();
      // The row selectors are retained product hooks, and names come from the fixture.
      const trigger = await row.count() ? row : page.locator('main [data-oracle-id]').first();
      await trigger.waitFor();
      await trigger.hover();
      const tooltip = page.getByRole('tooltip');
      await tooltip.waitFor();
      await tooltip.hover();
      await page.waitForTimeout(350);
      assert.equal(await tooltip.isVisible(), true, 'Pointer can rest over preview text');
      await page.keyboard.press('Escape');
      await tooltip.waitFor({ state: 'detached' });
      await trigger.focus();
      await tooltip.waitFor();
      const id = await tooltip.getAttribute('id');
      await page.waitForFunction(({ selector, id }) => document.querySelector(selector)?.getAttribute('aria-describedby')?.split(/\s+/).includes(id), { selector: `main [data-oracle-id="${await trigger.getAttribute('data-oracle-id')}"]`, id });
      assert.equal(await trigger.evaluate(node => !node.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', isComposing: true, bubbles: true, cancelable: true }))), false, 'IME Escape stays native');
      assert.equal(await trigger.evaluate(node => !node.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', keyCode: 229, bubbles: true, cancelable: true }))), false, 'Safari IME Escape stays native');
      await page.keyboard.press('Escape');
      await tooltip.waitFor({ state: 'detached' });
      assert.equal(await trigger.evaluate(node => node === document.activeElement), true);
      await trigger.press('Enter');
      await page.waitForTimeout(650);
      assert.equal(await page.getByRole('tooltip').count(), 0, 'Activation cancels delayed previews before entering the real inspector');
      const back = page.getByRole('button', { name: /^Back to/ });
      await back.waitFor();
      const primary = page.getByRole('combobox', { name: 'Primary slot', exact: true });
      await primary.click();
      await page.getByRole('listbox').waitFor();
      await page.keyboard.press('Escape');
      await page.waitForFunction(() => document.querySelector('[role=combobox][aria-label="Primary slot"]')?.getAttribute('aria-expanded') === 'false');
      assert.equal(await back.isVisible(), true, 'Native dropdown Escape must preserve its surrounding inspector');
    });
    await check('Missing public pages replace the previous private page title', async () => {
      await page.goto('/decks');
      await page.getByRole('heading', { name: 'My decks', exact: true }).waitFor();
      await page.evaluate(() => { const link = document.createElement('a'); link.href = '/d/never-shared-modern-review'; link.textContent = 'Missing fixture share'; document.querySelector('main').append(link); });
      await page.getByRole('link', { name: 'Missing fixture share', exact: true }).click();
      await page.waitForFunction(() => document.title === 'There is no shared deck here · Multiverse' && document.activeElement?.tagName === 'H1');
      await page.goto('/u/never-shared-modern-review');
      await page.waitForFunction(() => document.title === 'There is no one with that handle · Multiverse');
    });
    await check('Session bootstrap failures have a keyboard reachable retry', async () => {
      let failed = false;
      await page.route('**/api/session', route => {
        if (!failed) { failed = true; return route.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ error: 'Fixture temporary outage' }) }); }
        return route.continue();
      });
      await page.goto('/');
      await page.getByRole('heading', { name: 'Unable to open Multiverse', exact: true }).waitFor();
      await page.getByRole('button', { name: 'Retry', exact: true }).click();
      await page.getByRole('heading', { name: 'Home', exact: true }).waitFor();
      await page.unroute('**/api/session');
    });
    await check('Standalone consent scrolls on phones, wraps redirects, and restores failed-action focus', async () => {
      // Exercise the actual standalone renderer with a synthetic OAuth identity;
      // authorization/signature validation remains the backend test's authority.
      const response = await context.request.get('/');
      const styles = [...(await response.text()).matchAll(/href="([^" ]+\.css)"/g)].map(match => match[1]);
      const identity = { name: 'Fixture client', identityHost: null, redirectHost: '127.0.0.1:9876', redirectUri: `http://127.0.0.1:9876/${'redirect'.repeat(50)}`, loopback: true };
      await page.route('**/consent?fixture=unchanged', route => route.fulfill({ status: 200, contentType: 'text/html', headers: { 'content-security-policy': response.headers()['content-security-policy'] }, body: consentHtml(identity, styles) }));
      await page.route('**/consent.js', route => route.fulfill({ status: 200, contentType: 'text/javascript', body: consentScript }));
      let body;
      await page.route('**/api/auth/oauth2/consent', route => { body = route.request().postDataJSON(); return route.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ error: 'Fixture temporary outage' }) }); });
      await page.setViewportSize({ width: 375, height: 600 });
      await page.goto('/consent?fixture=unchanged');
      await page.evaluate(() => { document.documentElement.style.fontSize = '200%'; });
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Long redirect must not overflow');
      await page.mouse.move(160, 250);
      await page.mouse.wheel(0, 900);
      await page.waitForFunction(() => scrollY > 0);
      const deny = page.getByRole('button', { name: 'Deny', exact: true });
      await deny.click();
      await page.getByRole('alert').filter({ hasText: 'Fixture temporary outage' }).waitFor();
      assert.equal(await deny.evaluate(node => node === document.activeElement), true);
      assert.equal(await deny.isEnabled(), true);
      assert.deepEqual(body, { accept: false, oauth_query: 'fixture=unchanged' });
    });
    assert.deepEqual(errors, []);
    await context.close();
    results.push({ browser: name, version: browser.version() });
  } finally { await browser.close(); }
}
writeFileSync(join(out, 'report.json'), JSON.stringify({ origin, results }, null, 2) + '\n');
