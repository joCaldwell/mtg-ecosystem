// Browser regressions against the disposable UX-eval server, never live data.
// PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs AXE_MODULE=/path/to/axe.min.js \
//   node scripts/check-deck-editor.mjs [http://127.0.0.1:8873]
import assert from 'node:assert/strict';
import { mkdir, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const origin = process.argv[2] ?? 'http://127.0.0.1:8873';
assert.equal(new URL(origin).hostname, '127.0.0.1', 'use the disposable local UX-eval server');
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE ? pathToFileURL(resolve(process.env.PLAYWRIGHT_MODULE)).href : 'playwright');
const axe = await readFile(process.env.AXE_MODULE, 'utf8');
const artifacts = new URL('../.dev/ux-p4-a', import.meta.url).pathname;
await mkdir(artifacts, { recursive: true });
const browser = await chromium.launch({ headless: true });

async function settle(page) {
  await page.evaluate(() => new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done))));
}
async function until(check, message) {
  for (let attempt = 0; attempt < 50; attempt++) {
    if (await check()) return;
    await new Promise(done => setTimeout(done, 100));
  }
  assert.fail(message);
}
async function audit(page, label) {
  await page.evaluate(axe);
  const violations = await page.evaluate(async () => (await axe.run(document)).violations.map(v => ({ id: v.id, nodes: v.nodes.map(n => n.target) })));
  assert.deepEqual(violations, [], label + ': axe violations');
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), label + ': horizontal page overflow');
}
async function tracks(page) {
  const result = await page.locator('[data-deck-list] > div').first().evaluate(el => {
    const rows = [...el.querySelectorAll('button[data-oracle-id], h2')].filter(r => r.getClientRects().length);
    const overlaps = [];
    rows.forEach((a, i) => rows.slice(i + 1).forEach(b => {
      const x = a.getBoundingClientRect(), y = b.getBoundingClientRect();
      if (Math.min(x.right, y.right) - Math.max(x.left, y.left) > 1 && Math.min(x.bottom, y.bottom) - Math.max(x.top, y.top) > 1) overlaps.push([a.innerText, b.innerText]);
    }));
    return { overlaps, fragmented: rows.filter(r => r.getClientRects().length !== 1).map(r => r.innerText), widths: rows.filter(r => r.matches('button')).map(r => r.getBoundingClientRect().width), columns: new Set(rows.filter(r => r.matches('button')).map(r => Math.round(r.getBoundingClientRect().left))).size };
  });
  assert.deepEqual(result.overlaps, [], 'rows and headers reserve non-overlapping tracks');
  assert.deepEqual(result.fragmented, [], 'rows and headers never split across columns');
  assert(result.widths.every(w => w >= 249), 'card columns are at least about 250px');
  return result;
}
async function click(locator) {
  const page = locator.page();
  await locator.waitFor({ state: 'visible' });
  for (let attempt = 0; attempt < 40; attempt++) {
    const box = await locator.boundingBox();
    const viewport = page.viewportSize();
    if (box && box.y >= 0 && box.y + box.height <= viewport.height) {
      await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
      return;
    }
    const pane = await locator.evaluate(el => {
      const pane = el.closest('[data-panel-body]');
      const r = pane?.getBoundingClientRect();
      return r ? { x: r.x + r.width / 2, y: Math.max(80, Math.min(innerHeight - 80, r.y + r.height / 2)) } : null;
    });
    await page.mouse.move(pane?.x ?? viewport.width / 2, pane?.y ?? viewport.height / 2);
    await page.mouse.wheel(0, Math.sign((box?.y ?? 0) - viewport.height / 2) * 450);
    await settle(page);
  }
  assert.fail('could not reveal control with wheel input');
}
async function visibleInPane(locator) {
  return locator.evaluate(el => {
    const r = el.getBoundingClientRect(), pane = el.closest('[data-panel-body]')?.getBoundingClientRect();
    return r.top >= (pane?.top ?? 0) - 1 && r.bottom <= (pane?.bottom ?? innerHeight) + 1;
  });
}

try {
  for (const width of [375, 768, 1024, 1280, 1440]) {
    // Fine pointer as well: tablet sizing must not depend on device emulation.
    const context = await browser.newContext({ viewport: { width, height: width < 1100 ? 1024 : 900 }, reducedMotion: 'reduce' });
    await context.addCookies([{ name: 'uxeval_user', value: '1', url: origin }]);
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    const get = async path => {
      const response = await context.request.get(origin + path);
      assert(response.ok(), path);
      return response.json();
    };
    const decks = await get('/api/decks');
    const deck = decks.find(d => d.pending_items >= 4);
    assert(deck, 'seed has a deck with several proposal decisions');
    const { state } = await get(`/api/decks/${deck.id}`);
    const proposals = state.proposals.filter(p => p.items.some(i => i.status === 'pending')).sort((a, b) => a.id - b.id);
    const proposal = proposals[1];
    const first = proposal.items.find(i => i.status === 'pending');
    await page.goto(origin + `/deck/${deck.id}`);
    await page.getByRole('heading', { level: 1, name: deck.name }).waitFor();
    await settle(page);
    await audit(page, `${width} default`);
    if (width < 720) {
      const before = await page.evaluate(() => scrollY);
      await page.mouse.move(width / 2, 800);
      await page.mouse.wheel(0, 700);
      await until(() => page.evaluate(y => scrollY > y, before), 'phone window wheel scrolling works');
    } else {
      const layout = await tracks(page);
      if (width >= 1280) assert.equal(layout.columns, width === 1440 ? 3 : 2, 'three columns fit at 1440; a narrow list falls back to two');
      if (width < 1100) {
        assert(await page.locator('[data-deck-list] button[data-oracle-id]').first().evaluate(el => el.getBoundingClientRect().height >= 44), 'tablet card rows are 44px with a fine pointer');
        assert(await page.getByRole('combobox', { name: 'Add a card or search' }).evaluate(el => el.getBoundingClientRect().height >= 44), 'tablet add input is 44px');
        const before = await page.evaluate(() => scrollY);
        await page.mouse.move(width / 2, 800);
        await page.mouse.wheel(0, 700);
        await until(() => page.evaluate(y => scrollY > y, before), 'tablet window wheel scrolling works');
      }
      await page.goto(origin + `/inbox?entry=proposal-${proposal.id}`);
      await click(page.getByRole('link', { name: /^Review in the list/ }).or(page.getByRole('button', { name: /^Review in the list/ })));
      await page.waitForURL(url => url.pathname === `/deck/${deck.id}/review` && url.searchParams.get('proposal') === String(proposal.id));
      const chosen = page.locator(`[data-proposal="${proposal.id}"] [data-review-unit]`).first();
      const reason = chosen.getByRole('textbox', { name: /Reason for decision/ });
      await until(() => reason.evaluate(el => document.activeElement === el).catch(() => false), 'handoff focuses chosen proposal reason');
      await until(() => visibleInPane(chosen.getByRole('button', { name: /^Accept/ })), 'chosen decision controls remain visible after history loads');
      await audit(page, `${width} review`);
      if (width < 1100) await click(page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }));
      const current = page.locator(`[data-deck-list] [data-current="true"][data-oracle-id="${first.oracle_id}"]`).first();
      assert(await current.evaluate(el => {
        const r = el.getBoundingClientRect(), list = el.closest('[data-deck-list]').getBoundingClientRect();
        return r.top >= Math.max(0, list.top) - 1 && r.bottom <= Math.min(innerHeight, list.bottom) + 1;
      }), 'handoff reveals its first changed card, including after closing the tablet sheet');
      await tracks(page);
      // Variable row/header heights must trigger the browser's column reflow.
      const style = await page.addStyleTag({ content: '[data-deck-list] button[data-oracle-id] { min-height: 63px } [data-deck-list] h2 { padding-bottom: 25px }' });
      await settle(page);
      await tracks(page);
      await style.evaluate(el => el.remove());
      await page.screenshot({ path: `${artifacts}/review-${width}.png` });
      // Start from the normal editor so a request to open a card closes no review draft.
      await page.goto(origin + `/deck/${deck.id}`);
      await click(page.locator('[data-deck-list] button[data-oracle-id]').first());
      await page.getByRole('combobox', { name: 'Primary slot', exact: true }).waitFor();
      const fields = await page.getByRole('combobox', { name: 'Primary slot', exact: true }).evaluate(el => {
        const fields = el.closest('dl'), facts = fields.parentElement.parentElement, image = facts.firstElementChild;
        const r = fields.getBoundingClientRect(), art = image.getBoundingClientRect();
        const triggers = [...fields.querySelectorAll('button[aria-haspopup="listbox"]')];
        return { width: el.getBoundingClientRect().width, container: r.width, artBeside: art.right <= r.left && Math.abs(art.top - r.top) < 1, overflow: triggers.some(t => t.scrollWidth > t.clientWidth || t.getBoundingClientRect().right > r.right + 1), truncated: triggers.some(t => { const text = t.querySelector('span'); return text.scrollWidth > text.clientWidth; }) };
      });
      assert(fields.width >= 140 && fields.container >= 240 && fields.artBeside && !fields.overflow && !fields.truncated, 'compact art and fields fit without clipped selects or chevrons');
      await audit(page, `${width} card panel`);
      if (width === 1024) {
        const select = page.getByRole('combobox', { name: 'Primary slot', exact: true });
        const handle = await select.elementHandle();
        await page.setViewportSize({ width: 1280, height: 900 });
        await until(async () => await page.getByRole('dialog').count() === 0 && await select.isVisible(), 'desktop panel attaches after resize');
        assert(await select.evaluate((el, old) => el === old, handle), 'panel is not remounted crossing 1100px');
        await page.setViewportSize({ width: 1024, height: 1024 });
        await settle(page);
        assert(await handle.evaluate(el => el.tagName === 'SELECT'), 'card controls survive in the detached tablet panel host');
        await page.setViewportSize({ width: 1280, height: 900 });
        await until(() => select.isVisible(), 'desktop panel returns after second rotation');
        assert(await select.evaluate((el, old) => el === old, handle), 'panel identity survives both directions across 1100px');
      }
      if (width >= 1280) {
        // Resolve the query from installed card data; no fixture card facts from memory.
        const response = await get('/api/search?q=' + encodeURIComponent('t:creature') + '&limit=100');
        const candidates = response.cards ?? response;
        const candidate = candidates.find(c => !state.cards.some(owned => owned.oracle_id === c.oracle_id));
        assert(candidate, 'search supplies a card absent from this deck');
        await page.goto(origin + `/deck/${deck.id}`);
        const add = page.getByRole('combobox', { name: 'Add a card or search' });
        await add.fill(candidate.name);
        await page.getByRole('option').filter({ hasText: candidate.name }).first().waitFor();
        await add.press('Shift+Enter');
        await until(async () => await add.inputValue() === candidate.name && await add.getAttribute('aria-expanded') === 'true' && await page.getByRole('option').filter({ hasText: candidate.name }).first().innerText().then(t => /maybe list/i.test(t)), 'maybe add keeps results open and updates membership');
        assert(await add.evaluate(el => document.activeElement === el), 'successful add keeps input focus');
        const added = await get(`/api/decks/${deck.id}`);
        assert(added.state.cards.some(c => c.oracle_id === candidate.oracle_id && c.maybe), 'card was added to maybe list');
        await click(page.getByRole('button', { name: 'Undo', exact: true }));
        await until(async () => !(await get(`/api/decks/${deck.id}`)).state.cards.some(c => c.oracle_id === candidate.oracle_id), 'undo toast reverses maybe add');
        await add.fill(candidate.name);
        await page.getByRole('option').first().waitFor();
        await add.press('Enter');
        await until(async () => await add.inputValue() === '' && await add.getAttribute('aria-expanded') === 'false', 'normal deck add clears and closes results too');
        await click(page.getByRole('button', { name: 'Undo', exact: true }));
        await until(async () => !(await get(`/api/decks/${deck.id}`)).state.cards.some(c => c.oracle_id === candidate.oracle_id), 'undo toast reverses normal deck add');
      }
    }
    if (width === 768) {
      await page.goto(origin + '/decks');
      await click(page.getByRole('button', { name: 'Import deck', exact: true }));
      await page.getByRole('textbox', { name: /deck name/i }).fill('Tablet import regression');
      const list = await readFile(new URL('../test/fixtures/moxfield/kinnan-midrange.copy-plain-text.txt', import.meta.url), 'utf8');
      await page.getByRole('textbox', { name: /list to import/i }).fill(list);
      await click(page.getByRole('button', { name: /^Preview/ }));
      await click(page.getByRole('button', { name: /^Apply the import/ }));
      await page.waitForURL(/\/deck\/\d+$/);
      await page.locator('[data-deck-list] button[data-oracle-id]').first().waitFor();
      const sizes = await page.locator('main button, main input, main select').evaluateAll(elements => elements.filter(el => el.getClientRects().length).map(el => ({ name: el.getAttribute('aria-label') ?? el.textContent, height: el.getBoundingClientRect().height })));
      assert.deepEqual(sizes.filter(item => item.height < 43.5), [], 'post-import tablet controls retain 44px targets with a fine pointer');
      await tracks(page);
      await audit(page, '768 post-import');
      const importedId = Number(new URL(page.url()).pathname.split('/').pop());
      const removed = await context.request.delete(origin + `/api/decks/${importedId}`, { headers: { Origin: origin } });
      assert(removed.ok(), 'remove only the disposable imported test deck');
    }
    assert.deepEqual(errors, [], `${width}: browser errors`);
    await page.goto(origin + '/decks');
    await page.getByRole('heading', { level: 1 }).waitFor();
    await audit(page, `${width} roster`);
    console.log(`PASS ${width}px: accessibility, scrolling, layout and editor regressions`);
    await context.close();
  }
} finally {
  await browser.close();
}
