import test from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { chromium } from "playwright-core";
import { ObserverAudit, finishOwnedChild, deadline, discoverWithRateRecovery, waitForUiRead, boundedRetryAfter } from "./product-session-safety.ts";

test("actual checker wait allows a 35s read cooldown but never treats it as command consumption", async () => {
  for (const [rate, consumedAt, end, failure] of [
    [true, 36_000, 100_000, ""], [false, 36_000, 100_000, "still_unknown"],
    [true, Infinity, 100_000, "still_unknown"], [true, 36_000, 30_000, "still_unknown"],
  ] as const) {
    let now = 0, extended = 0;
    const work = waitForUiRead({ end, code: "still_unknown", now: () => now, healthy: () => {},
      ratePending: () => rate && now >= 1_000, onCooldown: () => { extended++; },
      // This simulates only arrival of the observed consumed receipt. A saved
      // journal stays unresolved until that explicit predicate becomes true.
      predicate: async () => now >= consumedAt, wait: async () => { now += 1_000; },
    });
    if (failure) { await assert.rejects(work, new RegExp(failure)); assert.equal(now, Math.min(end, rate ? 90_000 : 20_000)); }
    else { assert.equal(await work, true); assert.equal(now, consumedAt); }
    assert.equal(extended, rate ? 1 : 0);
  }
});

test("checker discards a stale menu and captures a new context only after real-UI recovery", async () => {
  const counts = { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 };
  let context = "original", calls = 0, returned = false, release!: () => void;
  const gate = new Promise<void>(done => { release = done; });
  const running = discoverWithRateRecovery({ allowed: true, end: Date.now() + 2_000, now: Date.now,
    healthy: async () => {}, capture: () => context, rateCount: () => 1,
    discover: async captured => { calls++; return calls === 1 ? { phase: "stale", value: `unsafe:${captured}` } : { phase: "complete", value: `fresh:${captured}` }; },
    recovered: async () => { await gate; context = "new-original"; return 1; },
  }, counts).then(value => { returned = true; return value; });
  await new Promise<void>(done => setImmediate(done));
  assert.equal(calls, 1); assert.equal(returned, false); assert.equal(counts.started, 1);
  release(); const result = await running;
  assert.deepEqual(result, { context: "new-original", value: "fresh:new-original", recovered: true });
  assert.deepEqual(counts, { handled: 1, started: 1, ready: 1, freshDiscoveries: 1 });
});

test("checker never retries a non-rate stale menu, other phase or unsafe journal/protocol state", async () => {
  for (const scenario of ["no429", "not-natural", "unavailable", "unknown_journal", "protocol_failure"]) {
    let recovery = 0, captures = 0;
    await assert.rejects(discoverWithRateRecovery({ allowed: scenario !== "not-natural", end: Date.now() + 1_000, now: Date.now,
      healthy: async () => { if (["unknown_journal", "protocol_failure"].includes(scenario)) throw Error(scenario); },
      capture: () => ++captures, rateCount: () => scenario === "no429" ? 0 : 1,
      discover: async () => ({ phase: scenario === "unavailable" ? "unavailable" : "stale", value: "unsafe" }),
      recovered: async () => { recovery++; return 1; },
    }, { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 }));
    assert.equal(recovery, 0); assert.equal(captures, ["unknown_journal", "protocol_failure"].includes(scenario) ? 0 : 1);
  }
  for (const failure of ["late_unknown_journal", "late_protocol_failure"]) {
    let unsafe = false, captures = 0;
    await assert.rejects(discoverWithRateRecovery({ allowed: true, end: Date.now() + 1_000, now: Date.now,
      healthy: async () => { if (unsafe) throw Error(failure); }, capture: () => ++captures, rateCount: () => 1,
      discover: async () => ({ phase: "stale", value: "unsafe" }), recovered: async () => { unsafe = true; return 1; },
    }, { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 }), new RegExp(failure));
    assert.equal(captures, 1);
  }
});

test("checker recovery has a deadline and finite rediscoveries even under repeated real rate evidence", async () => {
  let rates = 0, calls = 0;
  const counts = { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 };
  await assert.rejects(discoverWithRateRecovery({ allowed: true, end: Date.now() + 1_000, now: Date.now,
    healthy: async () => {}, capture: () => "context", rateCount: () => rates,
    discover: async () => { rates++; calls++; return { phase: "stale", value: "unsafe" }; }, recovered: async () => rates,
  }, counts), /persistent_rate_recovery/);
  assert.equal(calls, 3); assert.equal(counts.started, 2);
  const stalled = { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 };
  await assert.rejects(discoverWithRateRecovery({ allowed: true, end: Date.now() + 20, now: Date.now,
    healthy: async () => {}, capture: () => "context", rateCount: () => 1,
    discover: async () => ({ phase: "stale", value: "unsafe" }), recovered: async () => new Promise<number>(() => {}),
  }, stalled), /rate_recovery_deadline/);
  assert.equal(stalled.started, 1); assert.equal(stalled.ready, 0); assert.equal(stalled.freshDiscoveries, 0);
});

test("Retry-After receipts retain only bounded seconds, never raw header contents", () => {
  const now = Date.parse("2026-11-15T00:00:00Z");
  assert.equal(boundedRetryAfter("35", now), 35);
  assert.equal(boundedRetryAfter("Sun, 15 Nov 2026 00:00:35 GMT", now), 35);
  for (const value of [null, "private header contents", "61", "-1", "999999999999999999999", "Sun, 15 Nov 2026 00:02:00 GMT"]) assert.equal(boundedRetryAfter(value, now), null);
});

test("stalled owned host ignores stop/TERM but KILL exits, IPC disconnects and known leases are verified", async () => {
  const child = spawn(process.execPath, ["--input-type=module", "-e", "process.on('SIGTERM',()=>{});process.on('message',()=>{});process.send({ready:true});setInterval(()=>{},1000)"], { stdio: ["ignore", "ignore", "ignore", "ipc"] });
  try {
    await deadline(new Promise<void>(done => child.once("message", () => done())), 2_000);
    let joined = false;
    const result = await finishOwnedChild(child, async () => { joined = true; return 3; }, { graceful: 20, term: 20, kill: 2_000, join: 20 });
    assert.equal(result.exited && result.escalated && result.ipcDisconnected && result.verified && joined, true);
    assert.equal(result.leases, 3); assert.equal(result.complete, false);
  } finally { child.kill("SIGKILL"); if (child.connected) child.disconnect(); }
});

test("stalled lease verification is bounded and cannot claim complete cleanup", async () => {
  const child = spawn(process.execPath, ["-e", ""], { stdio: "ignore" });
  const result = await finishOwnedChild(child, async () => new Promise<never>(() => {}), { graceful: 2_000, term: 20, kill: 100, join: 20 });
  assert.equal(result.exited, true); assert.equal(result.verified, false); assert.equal(result.complete, false);
});

test("late context/schema failures, nested observers and page errors defeat final acceptance", async () => {
  const audit = new ObserverAudit();
  let release!: () => void;
  const held = new Promise<void>(done => { release = done; });
  audit.track(async () => { await held; audit.track(async () => { throw Error("late_context"); }); });
  assert.equal(audit.verdict("", true).passed, true);
  audit.closing = true; release(); await audit.drain(1_000);
  assert.equal(audit.verdict("", true).passed, false);
  const schema = new ObserverAudit(); schema.closing = true;
  schema.track(async () => { await schema.json({ json: async () => { throw new SyntaxError("late_schema"); }, request: () => ({ failure: () => ({ errorText: "net::ERR_ABORTED" }) }) }); });
  await schema.drain(1_000); assert.equal(schema.protocolFailed, true);
  const page = new ObserverAudit(); page.closing = true;
  page.track(async () => { page.pageErrors++; }); await page.drain(1_000);
  assert.equal(page.verdict("", true).failure, "page_error");
});

test("only evidenced shutdown body cancellation is excluded, never a context assertion", async () => {
  const audit = new ObserverAudit(); audit.closing = true;
  audit.track(async () => { await audit.json({ json: async () => { throw Error("body_closed"); }, request: () => ({ failure: () => ({ errorText: "NS_BINDING_ABORTED" }) }) }); });
  await audit.drain(1_000);
  assert.equal(audit.expectedCancellations, 1); assert.equal(audit.verdict("", true).passed, true);
  audit.track(async () => { throw Error("identity_mismatch"); }); await audit.drain(1_000);
  assert.equal(audit.verdict("", true).passed, false);
});

test("actual browser late malformed body and page exception fail after the preliminary checkpoint", async () => {
  // A separate unauthenticated transport fault probe, not a game-host response
  // substitute. Plain playwright-core creates no failure captures.
  let finishBody!: () => void;
  const server = createServer((request, response) => {
    if (request.url === "/late") {
      response.writeHead(200, { "Content-Type": "application/json" }); response.flushHeaders();
      finishBody = () => response.end("{invalid");
    } else { response.writeHead(200, { "Content-Type": "text/html" }); response.end("<!doctype html><title>Observer fault probe</title>"); }
  });
  await new Promise<void>(done => server.listen(0, "127.0.0.1", done));
  const address = server.address(); assert.equal(address !== null && typeof address !== "string", true);
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage(); const audit = new ObserverAudit();
    page.on("pageerror", () => { audit.pageErrors++; });
    page.on("response", response => { if (new URL(response.url()).pathname === "/late") audit.track(async () => { await audit.json(response); }); });
    await page.goto(`http://127.0.0.1:${(address as { port: number }).port}`);
    const arrived = page.waitForResponse(response => new URL(response.url()).pathname === "/late");
    await page.evaluate(() => { void fetch("/late").catch(() => {}); }); await arrived;
    assert.equal(audit.verdict("", true).passed, true);
    audit.closing = true; finishBody(); await audit.drain(2_000);
    assert.equal(audit.protocolFailed, true); assert.equal(audit.verdict("", true).passed, false);
    const error = new Promise<void>(done => page.once("pageerror", () => done()));
    await page.evaluate(() => { setTimeout(() => { throw Error("late_fixture_exception"); }, 0); });
    await deadline(error, 2_000); await browser.close(); await audit.drain(2_000);
    assert.equal(audit.pageErrors, 1); assert.equal(audit.verdict("", true).passed, false);
  } finally {
    await browser.close(); server.closeAllConnections(); await new Promise<void>(done => server.close(() => done()));
  }
});
