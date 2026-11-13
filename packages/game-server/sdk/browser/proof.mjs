import { GameClient, ProtocolError, HttpError, TransportError, counter } from '/browser.js';

const results = [];
const cspViolations = [];
addEventListener('securitypolicyviolation', event => cspViolations.push({ directive: event.effectiveDirective, blocked: event.blockedURI }));
const assert = (value, message) => { if (!value) throw new Error(message); };
const reject = async (operation, check) => {
  try { await operation(); } catch (error) { assert(check(error), 'wrong error boundary'); return; }
  throw new Error('expected explicit rejection');
};
const test = async (name, run) => { await run(); results.push(name); };
const json = value => new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } });
const empty = next => ({ protocol_version: 1, entries: [], next, has_more: false });
const capability = 'synthetic-browser-test-capability';
const commandId = 'a'.repeat(32);
const envelope = response => ({ protocol_version: 1, command_id: commandId, expected_revision: '0', command: { kind: 'decision', token: 'synthetic-token', response } });
const client = (fetch, maximumResponseBytes) => new GameClient({ baseURL: location.origin, matchId: 'a', capability,
  ...(fetch ? { fetch } : {}), ...(maximumResponseBytes ? { maximumResponseBytes } : {}) });

try {
  await test('ESM loads without Node globals or runtime code generation', () => {
    assert(typeof globalThis.process === 'undefined' && typeof globalThis.Buffer === 'undefined' && typeof globalThis.require === 'undefined', 'Node global present');
    assert(counter('18446744073709551615') === '18446744073709551615', 'u64 precision');
  });
  await test('CSP actually blocks dynamic evaluation', () => {
    let blocked = false;
    try { Function('return 1')(); } catch (error) { blocked = error.name === 'EvalError'; }
    assert(blocked, 'CSP did not block unsafe-eval');
  });
  await test('capability stays header-only and exact command string survives serialization', async () => {
    let calls = 0;
    const sdk = client(async (url, init) => {
      calls++;
      assert(!url.includes(capability) && init.headers.Authorization === `Bearer ${capability}`, 'capability handling');
      assert(init.credentials === 'omit' && init.redirect === 'error' && init.cache === 'no-store', 'fetch policy');
      const sent = JSON.parse(init.body);
      assert(sent.command.response.assignments[0][1] === '90071992547409931234567890', 'quantity rounded');
      return json({ status: 'accepted', command_id: commandId });
    });
    await sdk.submit(envelope({ response: 'damage', assignments: [['h', '90071992547409931234567890']] }));
    assert(calls === 1 && JSON.stringify(sdk) === '{}', 'send count or credential exposure');
  });
  await test('exact journal commands and bounded action queries use generated validators under CSP', async () => {
    const exact=' \n'+JSON.stringify(envelope({response:'pass'}),null,1)+'\n';
    let calls=0;
    const sdk=client(async (_url,init)=>{
      calls++;
      if(calls===1) { assert(init.body===exact,'exact journal bytes changed'); return json({status:'accepted',command_id:commandId}); }
      return json({protocol_version:1,match_id:'a',viewer:0,outcome:{status:'incomplete',page:{token:'synthetic-token',scope:{scope:'window'},context:{window:'priority'},beginnings:[]},required_minimum:1,cursor:'a'.repeat(64)}});
    });
    await sdk.submitExact(exact);
    const result=await sdk.queryActions({operation:'start',token:'synthetic-token',scope:{scope:'window'},page_size:1,work:1});
    assert(result.outcome.status==='incomplete' && calls===2,'query status/send count');
    await reject(()=>sdk.queryActions({operation:'start',token:'synthetic-token',scope:{scope:'window'},page_size:33,work:1}),error=>error instanceof ProtocolError);
    assert(calls===2,'invalid query sent');
  });
  await test('custom formats and tuples reject before transport', async () => {
    let calls = 0;
    const sdk = client(async () => { calls++; return json({}); });
    for (const response of [{ response: 'modes', mode: 4_294_967_296 }, { response: 'player_targets', players: [256] },
      { response: 'damage', assignments: [['h', '3', 'extra']] }, { response: 'damage', assignments: [['h', '01']] }]) {
      await reject(() => sdk.submit(envelope(response)), error => error instanceof ProtocolError);
    }
    assert(calls === 0, 'invalid command sent');
  });
  await test('raw JSON scanner rejects duplicate keys, rounded lexemes, surrogates and depth', async () => {
    const valid = JSON.stringify(empty('0'));
    for (const raw of [
      valid.replace('"protocol_version":1', '"protocol_version":1,"\\u0070rotocol_version":1'),
      valid.replace('"protocol_version":1', '"protocol_version":1e0'),
      valid.replace('"protocol_version":1', '"protocol_version":1.00000000000000001'),
      valid.replace('"protocol_version":1', '"protocol_version":9007199254740991.1'),
      valid.replace('"0"', '"\\ud800"'), '['.repeat(129) + '0' + ']'.repeat(129),
    ]) {
      await reject(() => client(async () => new Response(raw, { headers: { 'content-type': 'application/json' } })).poll({ after: '0' }), error => error instanceof ProtocolError);
    }
  });
  await test('response byte bounds cancel streams before unbounded buffering', async () => {
    let cancelled = false;
    const body = new ReadableStream({ pull(controller) { controller.enqueue(new Uint8Array(65)); }, cancel() { cancelled = true; } });
    await reject(() => client(async () => new Response(body, { headers: { 'content-type': 'application/json' } }), 64).poll({ after: '0' }), error => error instanceof ProtocolError);
    assert(cancelled, 'over-limit stream not cancelled');
    let errorCancelled = false;
    const errorBody = new ReadableStream({ cancel() { errorCancelled = true; } });
    await reject(() => client(async () => new Response(errorBody, { status: 503 })).poll({ after: '0' }), error => error instanceof HttpError);
    assert(errorCancelled, 'HTTP error body not cancelled');
  });
  await test('chunked decoding, fatal UTF-8 errors and lifecycle cursor validation', async () => {
    const page = { protocol_version: 1, entries: [{ cursor: '1', publication: { kind: 'lifecycle', status: { status: 'running' } } }], next: '1', has_more: false };
    const bytes = new TextEncoder().encode(JSON.stringify(page));
    const body = new ReadableStream({ start(controller) { for (const byte of bytes) controller.enqueue(Uint8Array.of(byte)); controller.close(); } });
    assert((await client(async () => new Response(body, { headers: { 'content-type': 'application/json' } })).poll({ after: '0' })).next === '1', 'split decoding');
    await reject(() => client(async () => new Response(Uint8Array.of(0xff), { headers: { 'content-type': 'application/json' } })).poll({ after: '0' }), error => error instanceof ProtocolError);
    await reject(() => client(async () => json({ ...page, next: '2' })).poll({ after: '0' }), error => error instanceof ProtocolError);
  });
  await test('ambiguous failures have no implicit retries or secret error details', async () => {
    let calls = 0;
    await reject(() => client(async () => { calls++; throw new Error(capability); }).submit(envelope({ response: 'pass' })),
      error => error instanceof TransportError && !String(error).includes(capability));
    assert(calls === 1, 'unexpected retry');
  });
  await test('native browser fetch reaches same-origin HTTP with exact u64 cursor', async () => {
    const page = await client().poll({ after: '18446744073709551615', waitMs: 0 });
    assert(page.next === '18446744073709551615', 'HTTP exact counter');
  });
  await test('explicitly injected native fetch retains its browser receiver', async () => {
    assert((await client(globalThis.fetch).poll({ after: '0', waitMs: 0 })).next === '0', 'injected native fetch receiver');
  });
  await test('native fetch abort during a pending HTTP response', async () => {
    const controller = new AbortController();
    const pending = client().poll({ after: '0', waitMs: 20000, signal: controller.signal });
    setTimeout(() => controller.abort(), 40);
    await reject(() => pending, error => error.name === 'AbortError');
  });
  await test('Web Locks and IndexedDB are available in loopback browser context', async () => {
    assert(typeof navigator.locks?.request === 'function', 'Web Locks unavailable');
    await navigator.locks.request('sdk-feasibility', { ifAvailable: true }, lock => assert(lock, 'lock unavailable'));
    const db = await new Promise((resolve, reject) => {
      const request = indexedDB.open('sdk-feasibility', 1);
      request.onupgradeneeded = () => request.result.createObjectStore('probe');
      request.onerror = () => reject(new Error('IndexedDB unavailable'));
      request.onsuccess = () => resolve(request.result);
    });
    await new Promise((resolve, reject) => {
      const tx = db.transaction('probe', 'readwrite'); tx.objectStore('probe').put('18446744073709551615', 'cursor');
      tx.oncomplete = resolve; tx.onerror = () => reject(new Error('IndexedDB write failed'));
    });
    db.close(); indexedDB.deleteDatabase('sdk-feasibility');
  });
  // CSP reports arrive asynchronously. The only expected violation is our
  // explicit negative control above, not any SDK module load or validation.
  await new Promise(resolve => setTimeout(resolve, 50));
  assert(cspViolations.length === 1 && cspViolations[0].blocked === 'eval', 'unexpected or missing CSP violation');
  const output = document.getElementById('result');
  output.textContent = JSON.stringify({ status: 'passed', checks: results, csp: cspViolations }, null, 2);
  output.dataset.status = 'passed';
} catch (error) {
  const output = document.getElementById('result');
  output.textContent = JSON.stringify({ status: 'failed', completed: results, error: String(error) }, null, 2);
  output.dataset.status = 'failed';
}
