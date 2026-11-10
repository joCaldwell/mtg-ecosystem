#!/usr/bin/env node
// Real Chromium wheel input with native smooth scrolling (the shared helpers disable
// it). Run on both builds: --dist /tmp/... --out /tmp/...json [--port 8841].
// Frame data is evidence, not a hardware mouse/trackpad certification.
import assert from 'node:assert/strict';
import { chromium } from 'playwright-core';
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const here = dirname(fileURLToPath(import.meta.url));
const arg = (key, fallback) => { const i = process.argv.indexOf(`--${key}`); return i >= 0 ? process.argv[i + 1] : fallback; };
const dist = arg('dist');
if (!dist || dist === process.argv[0]) throw Error('--dist required');
const port = process.argv.includes('--port') ? Number(arg('port')) : 8841;
const seed = process.argv.includes('--seed') ? arg('seed') : `${process.env.HOME}/.cache/deck-builder-browser-checks/seed-tokens/seed.db`;
const info = JSON.parse(readFileSync(resolve(dirname(seed), 'seed.json')));
const origin = `http://127.0.0.1:${port}`;
const server = spawn(process.execPath, [resolve(here, 'serve.ts'), '--port', `${port}`, '--seed', seed, '--dist', dist], { stdio: ['ignore', 'ignore', 'pipe'] });
let browser;
const results = [];
const frames = (page, n) => page.evaluate(n => new Promise(done => { const tick = () => --n <= 0 ? done() : requestAnimationFrame(tick); requestAnimationFrame(tick); }), n);
try {
  await new Promise((done, reject) => {
    let log = '';
    const timer = setTimeout(() => reject(Error(log)), 30000);
    server.stderr.on('data', data => { log += data; if (log.includes('CHECKS READY')) { clearTimeout(timer); done(); } });
    server.on('exit', code => { clearTimeout(timer); reject(Error(`server ${code}: ${log}`)); });
  });
  browser = await chromium.launch({ args: ['--enable-smooth-scrolling'], ignoreDefaultArgs: ['--hide-scrollbars'] });
  for (const inputMode of ['precise', 'pixel-wheel']) for (const reducedMotion of ['no-preference', 'reduce']) {
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion });
    await context.addCookies([{ name: 'checks_user', value: `${info.users.owner}`, url: origin }]);
    // Keep external image loading out of timing measurements, consistently on
    // both builds. Art fallbacks have the same dimensions as loaded images.
    await context.route(/^https:\/\//, route => route.abort());
    const page = await context.newPage();
    const cdp = await context.newCDPSession(page);
    await cdp.send('Performance.enable');
    let pointer;
    // CDP dispatchMouseEvent (used by Playwright) explicitly uses
    // kScrollByPrecisePixel. emulateTouchFromMouseEvent's wheel path leaves
    // granularity at kScrollByPixel, exercising Chromium's wheel animator.
    // Source: https://github.com/chromium/chromium/blob/main/content/browser/devtools/protocol/input_handler.cc
    const wheel = deltaY => inputMode === 'precise' ? page.mouse.wheel(0, deltaY)
      : cdp.send('Input.emulateTouchFromMouseEvent', { type: 'mouseWheel', x: pointer.x, y: pointer.y, deltaX: 0, deltaY: -deltaY, button: 'none' });
    await page.exposeBinding('sendHeaderWheel', () => wheel(4));
    for (const scenario of ['notch', 'repeated', 'small-deltas', 'reverse', 'mid-list', 'keyboard', 'reveal', 'resize']) {
      if (process.argv.includes('--regressions-only')) continue;
      if (arg('only') && !new RegExp(arg('only')).test(`${inputMode}/${reducedMotion}/${scenario}`)) continue;
      if (inputMode === 'pixel-wheel' && ['keyboard', 'reveal'].includes(scenario)) continue;
      await page.goto(`${origin}/deck/${info.decks.najeela}`);
      await page.locator('[data-deck-list] [data-oracle-id]').first().waitFor();
      await frames(page, 20);
      const box = await page.locator('[data-deck-list]').boundingBox();
      pointer = { x: Math.round(box.x + box.width / 2), y: Math.round(box.y + 80) };
      await page.mouse.move(pointer.x, pointer.y);
      if (['reverse', 'mid-list'].includes(scenario)) { await wheel(scenario === 'reverse' ? 360 : 650); await frames(page, 40); }
      await page.evaluate(() => {
        const page = document.querySelector('[data-deck-page]');
        const hero = page.querySelector('[data-deck-hero]');
        const compact = page.querySelector('[data-deck-compact]');
        const list = page.querySelector('[data-deck-list]');
        const toolbar = page.querySelector('[aria-label="Deck view controls"]');
        const rows = [...list.querySelectorAll('[data-oracle-id]')];
        window.headerSamples = [];
        window.headerLayout = rows.map(el => { const b = el.getBoundingClientRect(); return { x: b.left, y: b.top }; });
        window.headerWheelInputs = [];
        list.addEventListener('wheel', e => window.headerWheelInputs.push({ t: performance.now(), delta: e.deltaY }), { passive: true });
        window.headerSampling = true;
        const sample = t => {
          if (!window.headerSampling) return;
          const r = list.getBoundingClientRect();
          const visible = rows.find(el => { const b = el.getBoundingClientRect(); return b.bottom > r.top && b.top < r.bottom; });
          const row = rows[0].getBoundingClientRect();
          const first = visible?.getBoundingClientRect();
          window.headerSamples.push({ t, header: hero.getBoundingClientRect().top, compactOpacity: +getComputedStyle(compact).opacity, compactVisibility: getComputedStyle(compact).visibility, toolbar: toolbar.getBoundingClientRect().top, list: list.scrollTop, row: row.top, rowX: row.left, first: first?.top, firstId: visible?.dataset.oracleId, page: page.scrollTop, range: page.scrollHeight - page.clientHeight, listRange: list.scrollHeight - list.clientHeight, listTop: r.top, listBottom: r.bottom });
          requestAnimationFrame(sample);
        };
        requestAnimationFrame(sample);
      });
      await frames(page, 3);
      const perfBefore = Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m => [m.name, m.value]));
      if (scenario === 'notch' || scenario === 'mid-list') await wheel(120);
      if (scenario === 'repeated') for (let i = 0; i < 6; i++) { await wheel(120); await frames(page, 3); }
      if (scenario === 'small-deltas') await page.evaluate(() => new Promise(done => {
        let sent = 0;
        const send = () => {
          void window.sendHeaderWheel();
          if (++sent < 90) requestAnimationFrame(send); else done();
        };
        requestAnimationFrame(send);
      }));
      if (scenario === 'reverse') for (let i = 0; i < 4; i++) { await wheel(-120); await frames(page, 3); }
      if (scenario === 'keyboard') { await page.locator('[data-deck-list] [data-oracle-id]').first().focus(); await page.keyboard.press('PageDown'); }
      if (scenario === 'reveal') await page.evaluate(() => {
        const rows = [...document.querySelectorAll('[data-deck-list] button[data-oracle-id]')];
        rows.sort((a, b) => b.getBoundingClientRect().top - a.getBoundingClientRect().top)[0].focus();
      });
      if (scenario === 'resize') { await wheel(360); await frames(page, 4); await page.setViewportSize({ width: 1280, height: 800 }); }
      await frames(page, 45);
      const perfAfter = Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m => [m.name, m.value]));
      const samples = await page.evaluate(() => { window.headerSampling = false; return window.headerSamples; });
      const layout = await page.evaluate(() => {
        const rows = [...document.querySelectorAll('[data-deck-list] [data-oracle-id]')];
        const boxes = rows.map(el => el.getBoundingClientRect());
        const shift = boxes[0].top - window.headerLayout[0].y;
        return { maxColumnShift: Math.max(...boxes.map((b, i) => Math.abs(b.left - window.headerLayout[i].x))), maxRowReflow: Math.max(...boxes.map((b, i) => Math.abs(b.top - window.headerLayout[i].y - shift))), wheelEvents: window.headerWheelInputs.length,
          meanInputMs: window.headerWheelInputs.length > 1 ? (window.headerWheelInputs.at(-1).t - window.headerWheelInputs[0].t) / (window.headerWheelInputs.length - 1) : null };
      });
      const delta = samples.slice(1).map((s, i) => ({ row: samples[i].row - s.row, header: samples[i].header - s.header, toolbar: samples[i].toolbar - s.toolbar, dt: s.t - samples[i].t }));
      const direction = scenario === 'reverse' ? -1 : 1;
      const moving = delta.map((d, i) => Math.abs(d.row) > .1 ? i : -1).filter(i => i >= 0);
      const first = moving[0], last = moving.at(-1);
      const travel = samples[0].range;
      const crossings = samples.slice(1).flatMap((s, i) => (samples[i].page < travel - 1 && s.page >= travel - 1) || (samples[i].page >= travel - 1 && s.page < travel - 1) ? [{ frame: i + 1, before: delta.slice(Math.max(0, i - 2), i + 4).map(d => +d.row.toFixed(2)) }] : []);
      const summary = {
        inputMode, reducedMotion, scenario, ...layout,
        layoutCount: perfAfter.LayoutCount - perfBefore.LayoutCount, layoutMs: (perfAfter.LayoutDuration - perfBefore.LayoutDuration) * 1000, scriptMs: (perfAfter.ScriptDuration - perfBefore.ScriptDuration) * 1000, maxRowFrame: Math.max(...delta.map(d => Math.abs(d.row))), maxHeaderFrame: Math.max(...delta.map(d => Math.abs(d.header))), maxToolbarFrame: Math.max(...delta.map(d => Math.abs(d.toolbar))),
        backwardFrames: delta.filter(d => d.row * direction < -.5).length,
        interiorStalls: first === undefined ? 0 : delta.slice(first, last + 1).filter(d => Math.abs(d.row) < .1).length,
        movingFrames: moving.length, totalRowTravel: samples[0].row - samples.at(-1).row, rowXDrift: Math.max(...samples.map(s => Math.abs(s.rowX - samples[0].rowX))),
        listRangeDrift: Math.max(...samples.map(s => s.listRange)) - Math.min(...samples.map(s => s.listRange)),
        maxFrameMs: Math.max(...delta.map(d => d.dt)), crossings, final: samples.at(-1),
      };
      results.push({ ...summary, samples });
      console.log(JSON.stringify(summary));
      if (scenario === 'resize') await page.setViewportSize({ width: 1440, height: 900 });
    }
    if (inputMode === 'precise' && reducedMotion === 'no-preference' && !arg('only') && !process.argv.includes('--skip-regressions')) {
      const open = async path => {
        await page.goto(`${origin}${path}`);
        await page.locator('[data-deck-list] button[data-oracle-id]').first().waitFor();
        await frames(page, 20);
      };
      const position = () => page.evaluate(() => {
        const root = document.querySelector('[data-deck-page]');
        const list = root.querySelector('[data-deck-list]');
        const row = list.querySelector('button[data-oracle-id]').getBoundingClientRect();
        return { header: root.scrollTop, list: list.scrollTop, row: row.top, x: row.left, compact: root.dataset.compact };
      });
      await open(`/deck/${info.decks.najeela}`);
      let box = await page.locator('[data-deck-list]').boundingBox();
      await page.mouse.move(box.x + 80, box.y + 80);
      await page.mouse.wheel(0, 120);
      await frames(page, 20);
      const before = await position();
      const panel = await page.locator('[data-deck-panel]').boundingBox();
      await page.mouse.move(panel.x + panel.width / 2, panel.y + 100);
      await page.mouse.wheel(0, 360);
      await frames(page, 20);
      assert.deepEqual(await position(), before, 'panel scroll must leave header and list alone');
      await page.locator('[data-deck-list] button[data-oracle-id]').first().click();
      await frames(page, 3);
      assert.deepEqual(await position(), before, 'opening card details must preserve all list geometry');
      const cardName = await page.locator('[data-deck-list] button[data-oracle-id] > span:nth-child(2) > span').first().textContent();
      const find = page.getByRole('searchbox', { name: 'Search this deck', exact: true });
      await find.fill(cardName);
      await frames(page, 5);
      assert.equal((await position()).compact, 'true', 'find reveals the full workspace');
      await find.fill('');
      await frames(page, 5);
      assert.deepEqual(await position(), before, 'clearing find must restore header, rows and list');
      await open(`/deck/${info.decks.najeela}/review`);
      for (let i = 0; i < 5; i++) { await page.keyboard.press('j'); await frames(page, 2); }
      const reviewVisible = await page.evaluate(() => {
        const list = document.querySelector('[data-deck-list]');
        const box = list.getBoundingClientRect();
        return [...list.querySelectorAll('[data-current="true"]')].some(el => { const r = el.getBoundingClientRect(); return r.top >= box.top && r.bottom <= box.bottom; });
      });
      assert.ok(reviewVisible, 'keyboard review must land a current row inside the list');
      await open(`/deck/${info.decks.najeela}`);
      box = await page.locator('[data-deck-list]').boundingBox();
      await page.mouse.move(box.x + box.width - 5, box.y + 30);
      await page.mouse.down();
      for (let i = 1; i <= 15; i++) {
        await page.mouse.move(box.x + box.width - 5, box.y + 30 + i * 15);
        await frames(page, 1);
      }
      await page.mouse.up();
      await frames(page, 5);
      assert.ok((await position()).list > 184, 'native scrollbar drag scrolls through the header into the list');
      console.log('Regressions passed: panel-only scroll, card opening/aria-current, find and clear restoration, keyboard review, native scrollbar drag.');
    }
    await context.close();
  }
  const out = process.argv.includes('--out') ? arg('out') : '/tmp/header-scroll.json';
  writeFileSync(out, JSON.stringify(results, null, 2) + '\n');
  console.log(`Frame samples: ${out}`);
} finally {
  await browser?.close();
  server.kill('SIGTERM');
}
