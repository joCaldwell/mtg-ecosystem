// Unified product checks against a disposable browser-checks/serve.ts fixture.
// Existing fixture cookies select test actors; no production authentication is used.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { chromium } from 'playwright-core';

function argument(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  return index < 0 ? fallback : process.argv[index + 1];
}
const origin = argument('origin', 'http://127.0.0.1:8862');
const url = new URL(origin);
assert.equal(url.protocol, 'http:');
assert.ok(['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname), 'Disposable loopback fixture only');
const out = resolve(argument('out', '.dev/product-shell-check'));
mkdirSync(out, { recursive: true });
const browser = await chromium.launch({ headless: true });
const checks = [];
async function check(name, run) {
  await run();
  checks.push(name);
  console.log(`Passed: ${name}`);
}
async function actor(id, viewport = { width: 1440, height: 900 }) {
  const context = await browser.newContext({ baseURL: origin, viewport, reducedMotion: 'reduce' });
  if (id !== null) await context.addCookies([{ name: 'checks_user', value: String(id), url: origin }]);
  return context;
}
async function heading(page, name) { await page.getByRole('heading', { name, exact: true }).waitFor(); }
async function noOverflow(page) {
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'No page-wide horizontal overflow');
}

try {
  const owner = await actor(1);
  const response = await owner.request.get('/api/decks');
  assert.equal(response.status(), 200);
  const decks = await response.json();
  const deck = decks.find(value => value.commanders.length > 0 && value.category !== 'archived');
  assert.ok(deck, 'Source-backed populated deck fixture required');
  const before = await (await owner.request.get(`/api/decks/${deck.id}`)).json();
  const page = await owner.newPage();
  const errors = [], gameRequests = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', request => { if (/\/(v1|presentation)\//.test(new URL(request.url()).pathname)) gameRequests.push(request.url()); });

  await check('Home and owned deck setup share the existing account', async () => {
    await page.goto('/'); await heading(page, 'Home');
    await page.getByRole('link', { name: 'Start a game', exact: true }).click();
    await heading(page, 'New game');
    const select = page.getByRole('combobox', { name: 'Your deck', exact: true });
    await select.click();
    await page.getByRole('option', { name: deck.name, exact: true }).click();
    await page.getByRole('link', { name: 'Edit deck', exact: true }).waitFor();
    assert.ok((await page.locator('main').innerText()).includes(deck.name));
    assert.ok(await page.getByRole('button', { name: 'Start game', exact: true }).isDisabled());
    await page.screenshot({ path: join(out, 'setup-1440.png'), fullPage: true });
  });

  await check('Qualified in-app preview preserves choice and return deck context', async () => {
    const rootTokens = await page.evaluate(() => ['--bg', '--text', '--font-body'].map(name => getComputedStyle(document.documentElement).getPropertyValue(name)));
    await page.getByRole('link', { name: 'Preview table', exact: true }).click();
    await heading(page, 'Table preview');
    const keep = page.getByRole('radio', { name: 'Keep hand', exact: true });
    await keep.check();
    const card = page.getByRole('button', { name: 'Inspect Plains', exact: true }).last();
    await card.focus(); await page.keyboard.press('Enter');
    const inspector = page.getByRole('complementary', { name: 'Card inspector', exact: true });
    await inspector.getByRole('heading', { name: 'Plains', exact: true }).waitFor();
    assert.ok(await keep.isChecked());
    await page.keyboard.press('Escape');
    assert.ok(await card.evaluate(element => element === document.activeElement));
    await page.screenshot({ path: join(out, 'preview-1440.png'), fullPage: true });
    await page.reload(); await heading(page, 'Table preview');
    assert.equal(await keep.isChecked(), false);
    await page.getByRole('link', { name: 'Back to setup', exact: true }).click();
    await heading(page, 'New game');
    assert.ok((await page.getByRole('combobox', { name: 'Your deck', exact: true }).innerText()).includes(deck.name));
    await page.getByRole('link', { name: 'Edit deck', exact: true }).click();
    await heading(page, deck.name);
    assert.deepEqual(await page.evaluate(() => ['--bg', '--text', '--font-body'].map(name => getComputedStyle(document.documentElement).getPropertyValue(name))), rootTokens, 'Game styles must not change the host theme');
    assert.deepEqual(await (await owner.request.get(`/api/decks/${deck.id}`)).json(), before, 'Setup/preview must not change the builder deck');
    assert.deepEqual(gameRequests, [], 'Preview sends no game requests');
  });

  await check('Library, deep links and deck-to-Play entry remain available', async () => {
    await page.getByRole('link', { name: 'Play with this deck', exact: true }).click();
    await heading(page, 'New game');
    assert.ok((await page.getByRole('combobox', { name: 'Your deck', exact: true }).innerText()).includes(deck.name));
    await page.goto('/decks'); await heading(page, 'My decks');
    await page.reload(); await heading(page, 'My decks');
    await page.goto(`/play?deck=${deck.id}`); await heading(page, 'New game');
    assert.ok((await page.getByRole('combobox', { name: 'Your deck', exact: true }).innerText()).includes(deck.name));
    await page.goto('/'); await heading(page, 'Home');
    await page.screenshot({ path: join(out, 'home-1440.png'), fullPage: true });
  });

  await check('A delayed prior deck read cannot replace the current selection', async () => {
    const second = decks.find(value => value.id !== deck.id && value.category !== 'archived');
    assert.ok(second, 'Two owned fixture decks required');
    let arrived, release, handled;
    const requestArrived = new Promise(resolve => { arrived = resolve; });
    const unblock = new Promise(resolve => { release = resolve; });
    const requestHandled = new Promise(resolve => { handled = resolve; });
    const pattern = `**/api/decks/${deck.id}`;
    await page.route(pattern, async route => {
      const response = await route.fetch();
      arrived();
      await unblock;
      try { await route.fulfill({ response }); }
      catch (error) { if (!/aborted|closed|disposed/i.test(String(error))) throw error; }
      finally { handled(); }
    });
    try {
      await page.goto(`/play?deck=${deck.id}`);
      await requestArrived;
      await page.getByRole('combobox', { name: 'Your deck', exact: true }).click();
      await page.getByRole('option', { name: second.name, exact: true }).click();
      await heading(page, second.name);
      release(); await requestHandled;
      assert.equal(await page.getByRole('heading', { name: deck.name, exact: true }).count(), 0);
      assert.ok((await page.getByRole('link', { name: 'Edit deck', exact: true }).getAttribute('href')).endsWith(`/deck/${second.id}`));
    } finally {
      release();
      await page.unroute(pattern);
    }
  });

  await check('Empty account and unreadable selected deck never leak another owner', async () => {
    const empty = await actor(2), other = await empty.newPage();
    await other.goto('/'); await heading(other, 'Home');
    assert.equal((await other.locator('main').innerText()).includes(deck.name), false);
    await other.goto(`/play?deck=${deck.id}`); await heading(other, 'New game');
    await other.getByText('Deck unavailable', { exact: true }).waitFor();
    assert.equal((await other.locator('main').innerText()).includes(deck.name), false);
    assert.equal(await other.getByRole('link', { name: 'Edit deck', exact: true }).count(), 0);
    await empty.close();
  });

  await check('Signed-out game setup stays behind the existing sign-in boundary', async () => {
    const anonymous = await actor(null), signedOut = await anonymous.newPage();
    const privateRequests = [];
    signedOut.on('request', request => { if (new URL(request.url()).pathname === '/api/decks') privateRequests.push(request.url()); });
    await signedOut.goto('/play');
    await signedOut.getByRole('button', { name: /Google|Discord/ }).first().waitFor();
    assert.equal(await signedOut.getByRole('combobox', { name: 'Your deck', exact: true }).count(), 0);
    assert.deepEqual(privateRequests, []);
    await anonymous.close();
  });

  await check('Desktop, tablet and phone Home/Play retain navigation and fit', async () => {
    for (const width of [1280, 768, 375]) {
      await page.setViewportSize({ width, height: 800 });
      await page.goto('/'); await heading(page, 'Home'); await noOverflow(page);
      await page.screenshot({ path: join(out, `home-${width}.png`), fullPage: true });
      await page.goto(`/play?deck=${deck.id}`);
      await page.getByRole('heading', { name: /New game|Open on a larger screen/ }).waitFor();
      await noOverflow(page);
      await page.screenshot({ path: join(out, `setup-${width}.png`), fullPage: true });
    }
  });
  assert.deepEqual(errors, []);
  await owner.close();
  writeFileSync(join(out, 'report.json'), JSON.stringify({ status: 'passed', checks, browser: browser.version(), omissions: ['Real native host', 'Live match creation', 'Full-game play', 'Dense-board usability'] }, null, 2) + '\n');
} catch (error) {
  writeFileSync(join(out, 'report.json'), JSON.stringify({ status: 'failed', checks, error: String(error) }, null, 2) + '\n');
  throw error;
} finally {
  await browser.close();
}
