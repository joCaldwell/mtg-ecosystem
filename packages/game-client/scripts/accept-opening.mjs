// Explicit production-App acceptance. No Cargo, source-module injection, fake
// execution status, card policy, traces/HAR/video or capability-bearing output.
// Every automatic input below is a named finite opening-prefix TEST action.
import fs from 'node:fs';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { request } from 'node:http';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import { chromium, firefox, expect } from '@playwright/test';
import { GameClient } from '../../game-server/sdk/dist/browser.js';

const [binary, provisioner, nativeInfoPath, sourcePath, bundle, presentation, output, portText] = process.argv.slice(2);
assert(output && portText && !fs.existsSync(output), 'Supply retained server/provisioner/info/source, bundle, presentation, NEW output and explicit first native port');
for (const input of [binary, provisioner, nativeInfoPath, sourcePath, bundle, presentation, output]) assert(path.isAbsolute(input), 'Absolute inputs required');
const firstPort = Number(portText); assert(Number.isInteger(firstPort) && firstPort > 1024 && firstPort < 65000);
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const pins = JSON.parse(fs.readFileSync('packages/game-client/sdk-qualification.json'));
const nativeInfo = JSON.parse(fs.readFileSync(nativeInfoPath));
const bundleHash = sha(fs.readFileSync(path.join(bundle, 'client-assets.json')));
const presentationHash = sha(fs.readFileSync(path.join(presentation, 'artifacts.json')));
fs.mkdirSync(output, { mode: 0o700 });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const results = [];
let nextPort = firstPort;
async function waitFor(check, message, attempts = 300) {
  for (let n = 0; n < attempts; n++) { if (await check()) return; await delay(50); }
  throw Error(message);
}
async function stop(child) {
  if (child.exitCode !== null) return;
  child.kill('SIGINT'); const bound = setTimeout(() => child.kill('SIGKILL'), 8000);
  try { await once(child, 'exit'); } finally { clearTimeout(bound); }
}
async function nativeCase(name, work) {
  const dir = path.join(output, name); fs.mkdirSync(dir, { mode: 0o700 });
  const nativePort = nextPort++, origin = `http://127.0.0.1:${nativePort}`;
  const registry = path.join(dir, 'registry'), privateDir = path.join(dir, 'private');
  const provision = spawnSync(provisioner, [registry, privateDir, origin], { encoding: 'utf8' });
  assert(provision.status === 0, 'Native provisioning failed; private output omitted');
  const files = Array.from({ length: 4 }, (_, seat) => path.join(privateDir, `seat-${seat}.json`));
  const seats = files.map(file => JSON.parse(fs.readFileSync(file)));
  for (const seat of seats) assert(seat.engine_build_id === nativeInfo.engine_build_id && seat.server_build_id === nativeInfo.server_build_id, 'Producer identity mismatch');
  const clients = seats.map(seat => new GameClient({ baseURL: origin, matchId: seat.match_id, capability: seat.capability }));
  const log = fs.openSync(path.join(dir, 'host.log'), 'wx', 0o600);
  const host = spawn(binary, ['serve-client', registry, `127.0.0.1:${nativePort}`, bundle, bundleHash, presentation, presentationHash], { stdio: ['ignore', log, log] });
  try {
    await waitFor(async () => { assert(host.exitCode === null, 'Host exited'); try { const r = await clients[0].resume(); return r.state.status === 'available' && r.state.observation.decision?.request === 'mulligan'; } catch { return false; } }, 'Native opening not ready');
    await work({ origin, files, seats, clients, dir });
    results.push({ case: name, status: 'passed', port: nativePort });
    console.log(JSON.stringify({ case: name, status: 'passed' }));
  } finally { await stop(host); fs.closeSync(log); }
}
async function join(page, origin, file) {
  await page.goto(`${origin}/join`);
  await page.getByLabel('Single-seat bootstrap file').setInputFiles(file);
  await expect(page.getByRole('heading', { name: /Connected · Seat/, exact: false })).toBeVisible({ timeout: 15000 });
  await page.getByRole('link', { name: 'Open live table', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Commander table', exact: true })).toBeVisible();
}
const choiceStatus = page => page.getByRole('region', { name: 'Choice status', exact: true });
const confirm = page => page.getByRole('button', { name: 'Confirm opening choice', exact: true });
async function choose(page, choice, duplicate = false) {
  const radio = page.getByRole('radio', { name: choice, exact: true });
  await expect(radio).toBeEnabled({ timeout: 15000 });
  await radio.focus(); await page.keyboard.press('Space');
  await expect(confirm(page)).toBeEnabled();
  if (duplicate) await confirm(page).evaluate(button => { button.click(); button.click(); });
  else { await confirm(page).focus(); await page.keyboard.press('Enter'); }
}
async function records(page) {
  return page.evaluate(() => new Promise((resolve, reject) => {
    const open = indexedDB.open('multiverse-command-journal', 1);
    open.onerror = () => reject(Error('IDB read failed'));
    open.onsuccess = () => { const db = open.result; const tx = db.transaction('commands'); const get = tx.objectStore('commands').getAll();
      get.onsuccess = () => resolve(get.result); tx.oncomplete = () => db.close(); };
  }));
}
async function countRecords(page, count) { await waitFor(async () => (await records(page)).length === count, 'Unexpected durable slot count'); }
async function freshDecision(client, oldToken, kind) {
  let read;
  await waitFor(async () => { read = await client.resume(); return read.state.status === 'available' && read.state.observation.decision?.token !== oldToken && (!kind || read.state.observation.decision?.request === kind); }, 'Decision did not progress');
  return read;
}
async function explicitSDKChoice(client, take) {
  const read = await client.resume();
  assert(read.state.status === 'available' && read.state.observation.decision?.request === 'mulligan', 'SDK test seat must have a mulligan request');
  const token = read.state.observation.decision.token;
  await client.submit({ protocol_version: 1, command_id: crypto.randomUUID().replaceAll('-', ''), expected_revision: read.next, command: { kind: 'decision', token, response: { response: 'mulligan', take } } });
  return freshDecision(client, token);
}
function watchPrivacy(page, seats) {
  let errors = 0, posts = 0;
  page.on('pageerror', () => errors++);
  page.on('request', req => {
    assert(seats.every(seat => !req.url().includes(seat.capability)), 'Capability leaked into URL');
    if (req.method() === 'POST') { assert(req.url().endsWith('/commands'), 'Unexpected live write route'); posts++; }
  });
  return { errors: () => errors, posts: () => posts };
}
async function security(origin) {
  const raw = (target, method = 'GET', headers = {}, body = '') => new Promise((resolve, reject) => {
    const req = request(origin + target, { method, headers }, res => { res.resume(); res.on('end', () => resolve({ status: res.statusCode, cors: res.headers['access-control-allow-origin'] })); });
    req.on('error', reject); req.end(body);
  });
  const cases = [
    ['/join', 'GET', {}, 200], ['/join', 'GET', { host: 'evil.test' }, 403],
    ['/join', 'GET', { origin: 'http://evil.test' }, 403], ['/join', 'GET', { 'sec-fetch-site': 'cross-site' }, 403],
    ['/generated/private.json', 'GET', {}, 404], ['/client-assets.json', 'GET', {}, 404], ['/private/seat-0.json', 'GET', {}, 404],
    ['/v1/matches/a1/commands', 'POST', { 'content-type': 'application/json' }, 401],
    [`/v1/matches/a1/commands/${'a'.repeat(32)}`, 'GET', {}, 401],
    [`/v1/matches/a1/commands/${'a'.repeat(32)}/cancel`, 'POST', {}, 401],
  ];
  for (const [target, method, headers, status] of cases) { const response = await raw(target, method, headers); assert(response.status === status && response.cors === undefined, 'Native security response mismatch'); }
  return cases.length;
}

for (const [browserName, launcher] of Object.entries({ chromium, firefox })) {
  const browser = await launcher.launch();
  results.push({ browser: browserName, version: browser.version() });
  try {
    await nativeCase(`${browserName}-prefix`, async ({ origin, files, seats, clients }) => {
      const contexts = [], pages = [], audits = [];
      const progress = []; const recordProgress = value => { progress.push(value); fs.writeFileSync(path.join(output, `${browserName}-prefix-progress.json`), JSON.stringify(progress, null, 2)); };
      try {
        for (let seat = 0; seat < 4; seat++) {
          const context = await browser.newContext({ serviceWorkers: 'block' }); contexts.push(context);
          const page = await context.newPage(); pages.push(page); audits.push(watchPrivacy(page, seats));
          page.on('response', async response => { if (response.url().includes('/commands')) { try { const body = await response.json(); recordProgress({ seat, status: response.status(), result: body.status, reason: body.reason, execution: body.status?.status }); } catch {} } });
          await join(page, origin, files[seat]);
          const r = await clients[seat].resume();
          assert(r.state.status === 'available' && r.viewer === seat && r.state.observation.view.objects.filter(o => o.zone.zone === 'hand').every(o => o.owner === seat), 'Permitted hand isolation failed');
        }
        const duplicate = await contexts[0].newPage(); await duplicate.goto(`${origin}/join`);
        await duplicate.getByLabel('Single-seat bootstrap file').setInputFiles(files[0]);
        await expect(duplicate.getByRole('heading', { name: 'Seat open in another tab', exact: true })).toBeVisible(); await duplicate.close();
        // Automated finite opening only: seat0 takes three permitted mulligans;
        // seats1-3 explicitly keep, then seat0 chooses two exact ordered handles.
        const initial = await clients[0].resume(); const token = initial.state.observation.decision.token;
        await choose(pages[0], 'Take mulligan');
        for (let seat = 1; seat < 4; seat++) await choose(pages[seat], 'Keep hand', seat === 3);
        let read = await freshDecision(clients[0], token, 'mulligan');
        for (let n = 1; n < 3; n++) {
          await expect(pages[0].getByText(`Mulligans taken: ${n}`, { exact: true })).toBeVisible();
          await choose(pages[0], 'Take mulligan');
          await waitFor(async () => { read = await clients[0].resume(); return read.state.status === 'available' && read.state.observation.decision?.request === 'bottom_opening'; }, 'Expected supplied native bottom request');
          const bottom = read.state.observation.decision;
          assert(bottom.count === n && bottom.candidates.length >= bottom.count, 'Unexpected supplied bottom count');
          const boxes = pages[0].getByRole('group', { name: 'Opening hand actions', exact: true }).getByRole('checkbox');
          await expect(boxes).toHaveCount(bottom.candidates.length);
          for (const handle of bottom.candidates.slice(0, bottom.count)) {
            const box = pages[0].locator(`input[type=checkbox][value="${handle}"]`); await box.focus(); await pages[0].keyboard.press('Space');
          }
          const expected = bottom.candidates.slice(0, bottom.count);
          if (bottom.count > 1) {
            const move = pages[0].getByRole('button', { name: `Move ${bottom.candidates[1]} earlier`, exact: true });
            await move.focus(); await pages[0].keyboard.press('Enter'); await expect(move).toBeFocused();
            [expected[0], expected[1]] = [expected[1], expected[0]];
          }
          const posted = pages[0].waitForRequest(req => req.method() === 'POST' && req.url().endsWith('/commands'));
          await confirm(pages[0]).focus(); await pages[0].keyboard.press('Enter');
          const body = JSON.parse((await posted).postData());
          assert(JSON.stringify(body.command.response.objects) === JSON.stringify(expected), 'Bottom count/order differed from explicit choices');
          await countRecords(pages[0], 0);
          read = await freshDecision(clients[0], bottom.token, 'mulligan');
        }
        await expect(pages[0].getByText('Mulligans taken: 3', { exact: true })).toBeVisible();
        await choose(pages[0], 'Keep hand');
        await countRecords(pages[0], 0);
        // Finish any supplied zero-count bottom decisions through actual UI.
        for (let n = 0; n < 20; n++) {
          const reads = await Promise.all(clients.map(client => client.resume()));
          const index = reads.findIndex(r => r.state.status === 'available' && r.state.observation.decision?.request === 'bottom_opening');
          if (index < 0) break;
          assert(reads[index].state.observation.decision.count === 0, 'Unscripted opening count');
          await expect(confirm(pages[index])).toBeEnabled(); await confirm(pages[index]).click(); await countRecords(pages[index], 0);
        }
        await waitFor(async () => (await clients[0].resume()).state.observation.view.setup == null, 'Opening prefix did not complete');
        let active = -1;
        await waitFor(async () => { const reads = await Promise.all(clients.map(client => client.resume())); active = reads.findIndex(r => r.state.status === 'available' && r.state.observation.decision?.request === 'priority'); return active >= 0; }, 'Expected unsupported post-opening priority');
        await expect(pages[active].getByText('Opening actions complete or unavailable', { exact: true })).toBeVisible();
        await expect(confirm(pages[active])).toHaveCount(0);
        const before = audits.reduce((n, a) => n + a.posts(), 0); await delay(400); assert(before === audits.reduce((n, a) => n + a.posts(), 0), 'Unsupported decision sent automatically');
        assert(audits[3].posts() === 1, 'Duplicate click sent more than one command');
        for (let seat = 0; seat < 4; seat++) { assert(audits[seat].errors() === 0, 'Production page error'); const saved = await records(pages[seat]); assert(!JSON.stringify(saved).includes(seats[seat].capability), 'Capability in IDB'); }
        assert(await security(origin) === 10);
        results.push({ proof: `${browserName}-opening`, browserContexts: 4, mulligans: 3, orderedBottom: 2, duplicateTabBlocked: true, keyboard: true, postOpeningBlocked: true, nativeSecurityChecks: 10 });
      } finally { for (const context of contexts) await context.close(); }
    });

    await nativeCase(`${browserName}-loss`, async ({ origin, files, seats, clients }) => {
      const context = await browser.newContext();
      try {
        const page = await context.newPage(); const audit = watchPrivacy(page, seats);
        let posts = [], blockStatus = true;
        await page.route('**/v1/matches/*/commands/*', route => blockStatus ? route.abort() : route.continue());
        await page.route('**/v1/matches/*/commands', async route => {
          const response = await route.fetch(); assert(response.status() === 200, 'Native commit acknowledgment missing');
          posts.push(route.request().postData()); await route.abort();
        });
        await join(page, origin, files[0]);
        const token = (await clients[0].resume()).state.observation.decision.token;
        await choose(page, 'Keep hand');
        await expect(choiceStatus(page).getByText(/result is uncertain/)).toBeVisible();
        const saved = await records(page); assert(saved.length === 1, 'Lost response must retain IDB record');
        assert(posts.length === 1 && posts[0] === saved[0].exactBody, 'Persisted bytes differ from actual native command');
        const envelope = JSON.parse(saved[0].exactBody);
        await waitFor(async () => (await clients[0].execution(envelope.command_id)).status.status === 'consumed_by_rules', 'Native consumption not observed');
        await freshDecision(clients[0], token); assert((await records(page)).length === 1, 'New observation resolved uncertainty');
        await page.getByRole('button', { name: 'Release seat and clear view', exact: true }).click();
        assert((await records(page)).length === 1, 'Stop cleared unresolved choice');
        blockStatus = false; await page.unrouteAll(); await page.reload();
        await expect(page.getByText('Import a single-seat file to open your permitted view.', { exact: true })).toBeVisible();
        await join(page, origin, files[0]); await expect(choiceStatus(page).getByText('The table used your choice. Review the next decision.', { exact: true })).toBeVisible();
        await countRecords(page, 0); assert(audit.posts() === 1, 'Consumed recovery replayed a choice');
        results.push({ proof: `${browserName}-loss`, persistedBeforeNativePost: true, injectedLossAfterNative200: true, realConsumedStatus: true, stopPreserved: true, refreshReimportRecovered: true, body_sha256: sha(posts[0]) });
      } finally { await context.close(); }
    });

    await nativeCase(`${browserName}-exact-retry`, async ({ origin, files, seats }) => {
      const context = await browser.newContext();
      try {
        const page = await context.newPage(); watchPrivacy(page, seats);
        const bodies = []; let accepted = false, permitStatus = false, nativeMissing = 0;
        await page.route('**/v1/matches/*/commands/*', async route => {
          if (accepted && !permitStatus) { await route.abort(); return; }
          const response = await route.fetch(); if (response.status() === 404) nativeMissing++;
          await route.fulfill({ response });
        });
        await page.route('**/v1/matches/*/commands', async route => {
          bodies.push(route.request().postData());
          const saved = await records(page);
          assert(saved.length === 1 && saved[0].exactBody === bodies.at(-1), 'Retry was not persisted exact bytes');
          if (bodies.length === 1) { await route.abort(); return; } // no native commit for first attempt
          const response = await route.fetch();
          const result = await response.json(); assert(response.status() === 200 && result.status === 'accepted', 'Exact retry not accepted by native host');
          accepted = true; await route.fulfill({ response });
        });
        await join(page, origin, files[0]); await choose(page, 'Keep hand');
        await expect(choiceStatus(page).getByText(/result is uncertain/)).toBeVisible();
        await page.getByRole('button', { name: 'Check or retry saved choice', exact: true }).click();
        await waitFor(() => accepted, 'Expected retry after real native missing execution row');
        assert(nativeMissing >= 1 && bodies.length === 2 && bodies[0] === bodies[1], 'Retry changed original ID, revision, token or bytes');
        assert((await records(page)).length === 1, 'Accepted acknowledgment cleared unresolved slot');
        await expect(confirm(page)).toHaveCount(0);
        // The injected status-read failure may replace the accepted/pending copy
        // immediately; neither state clears the durable slot. Let it settle.
        await delay(150);
        permitStatus = true;
        await page.getByRole('button', { name: 'Check or retry saved choice', exact: true }).click();
        await countRecords(page, 0);
        assert(bodies.length === 2, 'Consumption reconciliation created another command');
        results.push({ proof: `${browserName}-exact-retry`, beforeCommitLoss: true, nativeMissingRows: nativeMissing, exactAttempts: 2, sameBodyIdRevisionToken: true, acceptedRetained: true, realConsumedRecovery: true });
      } finally { await context.close(); }
    });

    await nativeCase(`${browserName}-rejection`, async ({ origin, files, seats, clients }) => {
      const context = await browser.newContext();
      let release;
      try {
        const page = await context.newPage(); watchPrivacy(page, seats);
        let arrived; const arrival = new Promise(resolve => { arrived = resolve; }); const gate = new Promise(resolve => { release = resolve; });
        let originalBody, nativeResult;
        await page.route('**/v1/matches/*/commands', async route => { originalBody = route.request().postData(); arrived(); await gate; const response = await route.fetch(); nativeResult = await response.json(); await route.fulfill({ response }); });
        await join(page, origin, files[0]); await choose(page, 'Keep hand'); await arrival;
        assert((await records(page)).length === 1, 'In-flight choice was not durably saved');
        // Explicit same-seat collision TEST, not a hidden policy driving play:
        // another SDK takes a mulligan while original UI intent is delayed.
        await explicitSDKChoice(clients[0], true);
        for (let seat = 1; seat < 4; seat++) await explicitSDKChoice(clients[seat], false);
        await freshDecision(clients[0], JSON.parse(originalBody).command.token, 'mulligan');
        const focus = page.getByRole('button', { name: 'Jump to decision' }); await focus.focus();
        await delay(300); await expect(focus).toBeFocused();
        release();
        await expect(choiceStatus(page).getByText(/rejected the previous choice/)).toBeVisible({ timeout: 15000 });
        assert(nativeResult?.status === 'rejected', 'Expected authoritative native rejection');
        assert((await records(page)).length === 0, 'Rejected record remained unresolved after coherent refresh');
        await expect(page.getByText('Mulligans taken: 1', { exact: true })).toBeVisible();
        const before = JSON.parse(originalBody); const authoritative = await clients[0].resume();
        assert(before.command.token !== authoritative.state.observation.decision?.token, 'Original token did not become stale');
        await expect(confirm(page)).toBeDisabled(); // new explicit selection required
        await delay(250); assert(nativeResult.command_id === before.command_id, 'Rejection identity mismatch');
        results.push({ proof: `${browserName}-rejection`, originalContextRetained: true, authoritativeRejection: true, refreshedBeforeNewConfirmation: true, remoteFocusPreserved: true });
      } finally { release?.(); await context.close(); }
    });

    await nativeCase(`${browserName}-storage`, async ({ origin, files, seats }) => {
      for (const fault of ['denied', 'corrupt']) {
        const context = await browser.newContext();
        try {
          const page = await context.newPage(); const audit = watchPrivacy(page, seats);
          if (fault === 'denied') await page.addInitScript(() => Object.defineProperty(window, 'indexedDB', { get() { throw new DOMException('Denied', 'SecurityError'); } }));
          else {
            await page.goto(`${origin}/join`);
            await page.evaluate(key => new Promise((resolve, reject) => { const open = indexedDB.open('multiverse-command-journal', 1); open.onupgradeneeded = () => open.result.createObjectStore('commands'); open.onerror = reject; open.onsuccess = () => { const db = open.result, tx = db.transaction('commands', 'readwrite'); tx.objectStore('commands').put({ version: 1, corrupt: true }, key); tx.oncomplete = () => { db.close(); resolve(); }; }; }), JSON.stringify([seats[0].server_origin, seats[0].match_id, '0']));
          }
          await join(page, origin, files[0]);
          await expect(choiceStatus(page).getByText(/Safe choice storage is unavailable or damaged/)).toBeVisible();
          await expect(confirm(page)).toHaveCount(0); assert(audit.posts() === 0, 'Storage fault allowed a send');
          results.push({ proof: `${browserName}-${fault}`, faultInjection: 'browser storage only', sends: 0 });
        } finally { await context.close(); }
      }
    });
  } finally { await browser.close(); }
}
const report = { status: 'passed', scope: 'Automated finite opening prefix and recovery through actual production App/native serve-client. No full game, hidden human-seat policy, synthetic execution, or human usability claim.', source_sha256: sha(fs.readFileSync(sourcePath)), binary_sha256: sha(fs.readFileSync(binary)), nativeInfo, protocol_sha256: pins.protocol_schema_sha256, sdk_build_sha256: sha(fs.readFileSync('packages/game-server/sdk/dist/build.json')), harness_sha256: sha(fs.readFileSync(new URL(import.meta.url))), bundle_sha256: bundleHash, presentation_sha256: presentationHash, results };
fs.writeFileSync(path.join(output, 'report.json'), JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ status: 'passed', cases: results.length, report: path.join(output, 'report.json') }));
