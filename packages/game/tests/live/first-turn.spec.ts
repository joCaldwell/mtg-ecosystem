import { expect, test, type Page } from '@playwright/test';
import { createHash, randomUUID } from 'node:crypto';
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { closeSync, mkdirSync, openSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { resolve } from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import {
  GameClient, parsePreparedCommand, parseSeatBootstrap, prepareCommand,
  type Action, type Answer, type Observation,
  type SeatResume, type StreamPage,
} from '@multiverse/game-server-sdk/browser';
import { OBSERVATION_VERSION } from '../../web/src/session/read-state.ts';

const qualification = JSON.parse(readFileSync(new URL('../../sdk-qualification.json', import.meta.url), 'utf8'));

// Opt in separately from the read-only smoke suite. Preparation supplies a checked
// production bundle and offline presentation; this suite never builds Cargo/SDK.
const root = process.env.MTG_FIRST_TURN_EVIDENCE;
const native = process.env.MTG_FIRST_TURN_NATIVE;
// Playwright's failure helper can otherwise write an authenticated DOM snapshot.
process.env.PLAYWRIGHT_NO_COPY_PROMPT = '1';
test.use({ trace: 'off', screenshot: 'off', video: 'off' });
const hash = (path: string) => createHash('sha256').update(readFileSync(path)).digest('hex');
const delay = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));

function assertCompleteResponse(actual: Answer, expected: Answer): void {
  expect(isDeepStrictEqual(actual, expected)).toBe(true);
}

// Bound even an operation that ignores its signal. SDK fetches additionally get
// the shared shutdown signal and a per-call deadline, including response bodies.
async function driverCall<T>(shutdown: AbortSignal, operation: (signal: AbortSignal) => Promise<T>, timeoutMs = 4_000): Promise<T> {
  const deadline = new AbortController();
  const signal = AbortSignal.any([shutdown, deadline.signal]);
  let rejectAbort!: (error: Error) => void;
  const aborted = new Promise<never>((_, reject) => { rejectAbort = reject; });
  const onAbort = () => rejectAbort(Error('Test driver stopped or exceeded deadline'));
  signal.addEventListener('abort', onAbort, { once: true });
  const timer = setTimeout(() => deadline.abort(), timeoutMs);
  try {
    if (signal.aborted) onAbort();
    return await Promise.race([aborted, Promise.resolve().then(() => {
      signal.throwIfAborted();
      return operation(signal);
    })]);
  } finally { clearTimeout(timer); signal.removeEventListener('abort', onAbort); }
}

async function finishOwnedHost(cleanup: () => Promise<unknown>, shutdown: () => Promise<void>, timeoutMs = 2_000): Promise<void> {
  try { await driverCall(new AbortController().signal, cleanup, timeoutMs); }
  finally { await shutdown(); }
}

test('qualification guards compare whole actions and shut down after stalled driver cleanup', async () => {
  const selected: Answer = {answer:'pay',pay:{step:'finish',undo_on_failure:true}};
  assertCompleteResponse(selected,{answer:'pay',pay:{step:'finish',undo_on_failure:true}});
  expect(()=>assertCompleteResponse({answer:'pay',pay:{step:'finish',undo_on_failure:false}},selected)).toThrow();
  const stalled = () => new Promise<never>(() => {});
  const abort = new AbortController();
  let started!: () => void;
  const arrival = new Promise<void>(resolve => { started = resolve; });
  const pending = driverCall(abort.signal, async signal => {
    expect(signal.aborted).toBe(false); started(); return stalled();
  });
  const rejected = expect(pending).rejects.toThrow('Test driver stopped or exceeded deadline');
  await arrival; abort.abort(); await rejected;
  await expect(driverCall(new AbortController().signal, stalled, 20)).rejects.toThrow('Test driver stopped or exceeded deadline');
  for (const cleanup of [stalled, async () => { throw Error('cleanup failed'); }]) {
    let stopped = false;
    await expect(finishOwnedHost(cleanup, async () => { stopped = true; }, 20)).rejects.toThrow();
    expect(stopped).toBe(true);
  }
});

async function freePort(): Promise<number> {
  const listener = createServer();
  await new Promise<void>((resolve, reject) => { listener.once('error', reject); listener.listen(0, '127.0.0.1', resolve); });
  const address = listener.address();
  if (!address || typeof address === 'string') throw Error('Expected numeric loopback');
  await new Promise<void>(resolve => listener.close(() => resolve()));
  return address.port;
}

async function journal(page: Page, exactBody?: string): Promise<boolean> {
  return page.evaluate(async body => {
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open('multiverse-command-journal', 1);
      request.onsuccess = () => resolve(request.result); request.onerror = () => reject(Error('journal open'));
    });
    try {
      return await new Promise<boolean>((resolve, reject) => {
        const tx = db.transaction('commands', 'readonly');
        const request = tx.objectStore('commands').getAll();
        // Resolve only on transaction completion. Never return private records.
        tx.oncomplete = () => resolve(body === undefined ? request.result.length === 0
          : request.result.length === 1 && request.result[0].exactBody === body
            && Object.keys(request.result[0]).sort().join() === 'binding,exactBody,version');
        tx.onerror = tx.onabort = () => reject(Error('journal read'));
      });
    } finally { db.close(); }
  }, exactBody);
}

test('native first turn: App keeps, discovers, pays, resolves commander and reimports', async ({ page, browser }, info) => {
  test.skip(!root || !native, 'Set MTG_FIRST_TURN_EVIDENCE and MTG_FIRST_TURN_NATIVE for disposable real-native qualification');
  test.setTimeout(180_000);
  process.umask(0o077);
  const parent = resolve(root!, 'matches', `${info.project.name}-${randomUUID()}`);
  mkdirSync(parent, { mode: 0o700 });
  expect(statSync(parent).mode & 0o777).toBe(0o700);
  const port = await freePort(), origin = `http://127.0.0.1:${port}`;
  const bundle = resolve(root!, 'bundle'), presentation = resolve(root!, 'presentation');
  const identity = JSON.parse(readFileSync(resolve(native!, 'native-info.json'), 'utf8'));
  expect(identity.engine_build_id === qualification.engine_build_id).toBe(true);
  expect(identity.server_build_id === qualification.server_build_id).toBe(true);
  expect(hash(resolve(native!, 'protocol.schema.json')) === qualification.protocol_schema_sha256).toBe(true);
  const binaryHashes = JSON.parse(readFileSync(resolve(native!, 'final-binaries.json'), 'utf8')) as Record<string, string>;
  for (const file of ['mtg-server', 'provision_client', 'native-info.json']) expect(hash(resolve(native!, file)) === binaryHashes[file]).toBe(true);
  execFileSync(resolve(native!, 'provision_client'), [resolve(parent, 'registry'), resolve(parent, 'seats'), origin], { stdio: ['ignore', 'pipe', 'pipe'] });
  const seats = Array.from({ length: 4 }, (_, seat) => {
    const path = resolve(parent, 'seats', `seat-${seat}.json`);
    expect(statSync(path).mode & 0o777).toBe(0o600);
    const bootstrap = parseSeatBootstrap(readFileSync(path, 'utf8'));
    expect(bootstrap.viewer === seat && bootstrap.server_origin === origin
      && bootstrap.engine_build_id === identity.engine_build_id && bootstrap.server_build_id === identity.server_build_id).toBe(true);
    return { path, bootstrap };
  });
  const log = openSync(resolve(parent, 'host.log'), 'wx', 0o600);
  const host = spawn(resolve(native!, 'mtg-server'), ['serve-client', resolve(parent, 'registry'), `127.0.0.1:${port}`,
    bundle, hash(resolve(bundle, 'client-assets.json')), presentation, hash(resolve(presentation, 'artifacts.json'))], { stdio: ['ignore', log, log] });
  closeSync(log);
  const exited = once(host, 'exit');
  let stop = false, driverFailure = false, driverCommands = 0;
  const driverAbort = new AbortController();
  const stopDriver = () => { stop = true; driverAbort.abort(); };
  let driver: Promise<void> = Promise.resolve();
  let observation: Observation | null = null, revision = '';
  let observationVersion: number | null = null;
  let errors = 0, journalBeforeSend = 0, originalContexts = 0, completeResponses = 0, resumes = 0;
  let cleanupComplete = false;
  let recovered = false, commanderResolved = false, landPlayed = false, conditionalReversal = false;
  const sent = new Map<string, string>(), accepted = new Set<string>(), consumed = new Set<string>();
  const kinds: string[] = [];
  const responseJobs = new Set<Promise<void>>();
  let protocolFailure = false;
  const record = (name: string, value: unknown) => writeFileSync(resolve(parent, name), JSON.stringify(value, null, 2)+'\n', { mode: 0o600, flag: 'wx' });
  try {
    await expect.poll(async () => {
      try { return (await fetch(`${origin}/client-config.json`, { signal: AbortSignal.timeout(2_000) })).status; } catch { return 0; }
    }).toBe(200);
    page.on('pageerror', () => { errors++; });
    page.on('response', response => {
      const job = (async () => {
        const path = new URL(response.url()).pathname;
        if (!path.startsWith('/v1/') || response.status() !== 200) return;
        const value = await response.json();
        if (path.endsWith('/resume')) {
          const read = value as SeatResume;
          if (read.viewer !== 0 || read.match_id !== seats[0]!.bootstrap.match_id || read.catalog_sha256 !== seats[0]!.bootstrap.catalog_sha256
            || read.engine_build_id !== identity.engine_build_id || read.server_build_id !== identity.server_build_id) throw Error('identity');
          resumes++;
          revision = read.next;
          if (read.state.status === 'available') observation = read.state.observation;
        } else if (path.endsWith('/events')) {
          const read = value as StreamPage;
          for (const entry of read.entries) if (entry.publication.kind === 'view_replaced') observation = entry.publication.observation;
          revision = read.next;
        } else if (/\/commands$/.test(path)) {
          if (value.status === 'accepted') accepted.add(value.command_id);
          else throw Error('human command rejected');
        } else if (/\/commands\/[^/]+$/.test(path) && value.status.status === 'consumed_by_rules') consumed.add(value.command_id);
      })().catch(() => { protocolFailure = true; });
      responseJobs.add(job); void job.finally(() => responseJobs.delete(job));
    });
    // Intercept only the outgoing human command, assert durable exact bytes, and
    // continue its original request. No response mocking or second human poller.
    await page.route('**/v1/matches/*/commands', async route => {
      const exactBody = route.request().postData()!;
      const envelope = parsePreparedCommand(exactBody).envelope;
      const valid = await journal(page, exactBody);
      if (!valid) { protocolFailure = true; await route.abort(); return; }
      journalBeforeSend++;
      sent.set(envelope.command_id, exactBody);
      if (envelope.command.kind === 'decision') kinds.push(envelope.command.answer.answer);
      await route.continue();
    });
    const join = async () => {
      await page.goto(`${origin}/join`);
      await page.getByLabel('Single-seat bootstrap file').setInputFiles(seats[0]!.path);
      await expect(page.getByRole('heading', { name: 'Connected · Seat 1', exact: true })).toBeVisible();
      await page.getByRole('link', { name: 'Open live table' }).click();
    };
    const current = (): Observation => {
      if (!observation) throw Error('No App-published observation');
      return observation;
    };
    const settle = async (action: () => Promise<unknown>, expectedResponse: Answer, original = { revision, token: current().decision!.token }) => {
      const before = sent.size;
      await action();
      await expect.poll(() => sent.size).toBe(before + 1);
      const id = [...sent.keys()].at(-1)!;
      const envelope = parsePreparedCommand(sent.get(id)!).envelope;
      expect(envelope.expected_revision === original.revision && envelope.command.kind === 'decision' && envelope.command.token === original.token).toBe(true);
      originalContexts++;
      if (envelope.command.kind !== 'decision') throw Error('Expected human decision command');
      assertCompleteResponse(envelope.command.answer, expectedResponse);
      completeResponses++;
      await expect.poll(() => accepted.has(id) && consumed.has(id) && !protocolFailure).toBe(true);
      await expect.poll(() => journal(page)).toBe(true);
      await expect(page.getByRole('region', { name: 'Choice status' })).toContainText('The table used your choice.');
    };
    const confirm=()=>page.getByRole('button',{name:'Confirm choice',exact:true}).click();
    const choose = async (predicate:(action:Action)=>boolean) => {
      const d=current().decision!;
      const index=d.candidates.findIndex(c=>c.candidate==='action' && predicate(c.action));
      expect(index).toBeGreaterThanOrEqual(0);
      const answer:Answer=d.shape.shape==='pay' ? {answer:'pay',pay:{step:'activate',source:index}} : {answer:'pick',picks:[[index]]};
      await page.locator('input[type=radio][value="'+index+'"]').check();
      await settle(confirm,answer);
    };
    await join();
    observationVersion = current().observation_version;
    expect(observationVersion).toBe(OBSERVATION_VERSION);
    const commander = current().view.objects.find(object => object.owner === 0 && object.zone.zone === 'command');
    expect(Boolean(commander)).toBe(true);
    const commanderOracle = commander!.oracle_id;
    // Explicitly bounded test-only keep/pass driver. Each other seat has its own
    // private exact-body journal and authoritative execution receipt. No policy
    // examples, hidden state, strategic decisions, or commands for human seat 0.
    const clients = seats.slice(1).map(({ bootstrap }) => new GameClient({ baseURL: origin, matchId: bootstrap.match_id, capability: bootstrap.capability }));
    driver = (async () => {
      for (let rounds = 0; !stop && rounds < 900; rounds++) {
        for (const [index, client] of clients.entries()) {
          if (stop) break;
          const seat = index + 1, read = await driverCall(driverAbort.signal, signal => client.resume({ signal }));
          expect(read.viewer === seat && read.engine_build_id === identity.engine_build_id && read.server_build_id === identity.server_build_id).toBe(true);
          if (read.state.status !== 'available' || !read.state.observation.decision) continue;
          const request = read.state.observation.decision;
          const i=request.candidates.findIndex(c=>c.candidate==='flag' && c.flag==='keep' || c.candidate==='action' && c.action.action==='pass');
          if (i<0) throw Error('Test driver reached a choice beyond keep/pass');
          const answer:Answer={answer:'pick',picks:[[i]]};
          const command = prepareCommand({ protocol_version: read.protocol_version, command_id: randomUUID().replaceAll('-', ''), expected_revision: read.next, command: { kind: 'decision', token: request.token, answer } });
          const name = `driver-seat-${seat}-${driverCommands++}`;
          writeFileSync(resolve(parent, `${name}.exact.json`), command.exactBody, { mode: 0o600, flag: 'wx' });
          const submission = await driverCall(driverAbort.signal, signal => client.submitExact(command.exactBody, { signal }));
          expect(submission.status === 'accepted').toBe(true);
          let status = await driverCall(driverAbort.signal, signal => client.execution(command.envelope.command_id, { signal }));
          for (let retry = 0; !stop && status.status.status === 'pending' && retry < 100; retry++) {
            await delay(30);
            if (stop) break;
            status = await driverCall(driverAbort.signal, signal => client.execution(command.envelope.command_id, { signal }));
          }
          record(`${name}.status.json`, { seat, submission, execution: status });
          expect(status.status.status === 'consumed_by_rules').toBe(true);
        }
        await delay(50);
      }
    })().catch(() => { if (!driverAbort.signal.aborted) driverFailure = true; });
    const openingContext = { revision, token: current().decision!.token };
    await page.getByRole('radio', { name: 'Keep', exact: true }).check();
    await settle(() => page.getByRole('button', { name: 'Confirm choice', exact: true }).click(), { answer:'pick',picks:[[current().decision!.candidates.findIndex(c=>c.candidate==='flag' && c.flag==='keep')]] }, openingContext);
    for (let attempts = 0; attempts < 12; attempts++) {
      await expect.poll(() => current().decision?.reason.reason === 'priority').toBe(true);
      if (current().view.turn.step === 'precombat_main') break;
      await choose(action => action.action === 'pass');
    }
    expect(current().view.turn.step === 'precombat_main' && current().view.turn.number === 1).toBe(true);
    await choose(action => action.action === 'play_land');
    expect(current().view.turn.lands_played).toBe(1);
    expect(current().view.objects.filter(object => object.owner === 0 && object.zone.zone === 'battlefield').length).toBe(1);
    await expect(page.getByRole('region', { name: 'Battlefield', exact: true }).getByRole('button')).toHaveCount(1);
    landPlayed = true;
    await choose(action => action.action==='cast' && action.card===commander!.handle);
    await expect.poll(()=>current().decision?.reason.reason).toBe('mana');
    await choose(action=>action.action==='activate_mana' && current().view.objects.some(o=>o.handle===action.object && o.zone.zone==='battlefield'));
    await page.locator('input[value="finish-undo"]').check();
    await settle(confirm,{answer:'pay',pay:{step:'finish',undo_on_failure:true}});
    conditionalReversal=true;
    await expect.poll(()=>current().decision?.reason.reason).toBe('spend_mana');
    const decision=current().decision!;
    const payment=decision.shape;
    if(payment.shape!=='pay' || payment.step.step!=='spend') throw Error('Expected explicit payment');
    expect(payment.cost.generic === '0' && Boolean(current().decision_context?.source)).toBe(true);
    const paymentContext = { revision, token: decision.token };
    const colors = { W: 'White', U: 'Blue', B: 'Black', R: 'Red', G: 'Green', C: 'Colorless' };
    const spending: Record<string,string> = {};
    for (const color of Object.keys(payment.pool.unrestricted) as (keyof typeof colors)[]) {
      const amount = payment.cost.colored.filter(required => required === color).length.toString();
      if (amount !== '0') spending[color] = amount;
      await page.getByRole('textbox', { name: new RegExp(`^${color} Available`) }).fill(amount);
    }
    await settle(() => page.getByRole('button', { name: 'Confirm choice', exact: true }).click(), {answer:'pay',pay:{step:'spend',unrestricted:spending}}, paymentContext);
    for (let attempts = 0; attempts < 4; attempts++) {
      if (current().view.objects.some(object => object.owner === 0 && object.oracle_id === commanderOracle && object.zone.zone === 'battlefield')) break;
      await expect.poll(() => current().decision?.reason.reason === 'priority' || current().view.objects.some(object => object.owner === 0 && object.oracle_id === commanderOracle && object.zone.zone === 'battlefield')).toBe(true);
      if (!current().view.stack.length) break;
      await choose(action => action.action === 'pass');
    }
    await expect.poll(() => current().view.objects.some(object => object.owner === 0 && object.oracle_id === commanderOracle && object.zone.zone === 'battlefield')).toBe(true);
    await expect(page.getByRole('region', { name: 'Battlefield', exact: true }).getByRole('button')).toHaveCount(2);
    commanderResolved = true;
    stopDriver(); await driverCall(new AbortController().signal, () => driver);
    expect(driverFailure).toBe(false);
    // Meaningful resolved-command recovery: lose memory authorization, reimport
    // the original seat, and prove the consumed command and changed board persist.
    const beforeReimport = resumes;
    await page.reload(); observation = null;
    await expect(page.getByRole('heading', { name: 'Not connected', exact: true })).toBeVisible();
    await join();
    expect(resumes > beforeReimport).toBe(true);
    await expect.poll(() => current().view.objects.some(object => object.owner === 0 && object.oracle_id === commanderOracle && object.zone.zone === 'battlefield')).toBe(true);
    await expect(page.getByRole('region', { name: 'Battlefield', exact: true }).getByRole('button')).toHaveCount(2);
    expect(await journal(page)).toBe(true); recovered = true;
    expect(await page.evaluate(() => localStorage.length === 0 && sessionStorage.length === 0 && location.search === '' && location.hash === '')).toBe(true);
    expect(errors === 0 && !protocolFailure && journalBeforeSend === sent.size && originalContexts === sent.size && completeResponses === sent.size && accepted.size === sent.size && consumed.size === sent.size).toBe(true);
    expect(['pick','pay'].every(kind => kinds.includes(kind))).toBe(true);
    await page.getByRole('button', { name: 'Release seat and clear view' }).click();
  } finally {
    stopDriver();
    await finishOwnedHost(async () => {
      await Promise.allSettled([driver, page.close(), ...responseJobs]);
      cleanupComplete = true;
    }, async () => {
      // This separate finally path runs even if driver/browser cleanup fails or
      // ignores cancellation. No unbounded cleanup may precede owned-host SIGINT.
      host.kill('SIGINT');
      const termination = await Promise.race([exited, delay(10_000).then(() => null)]);
      const units = execFileSync('systemctl', ['--user', 'list-units', 'mtg-p8-*.service', '--all', '--plain', '--no-legend', '--no-pager'], { encoding: 'utf8' });
      const ownedUnitsReleased = !units.split('\n').some(line => line.includes(`mtg-p8-${host.pid}-`) || line.includes(parent));
      // Never kill an unowned port/process. Retain every private registry/receipt.
      const released = await new Promise<boolean>(resolve => {
        const listener = createServer(); listener.once('error', () => resolve(false));
        listener.listen(port, '127.0.0.1', () => listener.close(() => resolve(true)));
      });
      record('browser-receipt.json', { browser: info.project.name, version: browser.version(), origin, engine_build_id: identity.engine_build_id, server_build_id: identity.server_build_id, observation_version: observationVersion,
        human_commands: sent.size, journal_before_send: journalBeforeSend, original_contexts: originalContexts, complete_responses: completeResponses, accepted: accepted.size, consumed: consumed.size, command_kinds: kinds, driver_commands: driverCommands, driver_failed: driverFailure, cleanup_complete: cleanupComplete,
        land_played: landPlayed, commander_resolved: commanderResolved, conditional_reversal_wording: conditionalReversal, recovered, page_errors: errors, protocol_failed: protocolFailure, sigint_exit: termination, port_released: released, owned_units_released: ownedUnitsReleased,
        scope: 'Disposable foundations first turn; seats 1–3 test-only keep/pass; no full game, strategy, generic permissions, newer native or Safari acceptance.' });
      expect(termination !== null && termination[0] === 0 && released && ownedUnitsReleased).toBe(true);
    });
  }
});
