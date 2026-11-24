import test from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { chromium } from "playwright-core";
import { prepareCommand, parsePreparedCommand } from "@multiverse/game-server-sdk";
import { ObserverAudit, SubmissionAudit, finishOwnedChild, deadline, discoverWithRateRecovery, waitForUiRead, readCooldownPending, boundedRetryAfter, queryRateInvalidated } from "./product-session-safety.ts";

test("captured query429 invalidation waits for a genuinely later product read before a fresh query", async () => {
  let calls = 0, finished = false, successfulRead = 5, release!: () => void;
  const readAtRate = 5, gate = new Promise<void>(done => { release = done; });
  const work = discoverWithRateRecovery({ allowed: true, end: Date.now() + 2_000, now: Date.now,
    healthy: async () => {}, capture: () => successfulRead, rateCount: () => 0, recovered: async () => 0,
    normalStale: { counts: freshStaleCounts(), ready: async () => {
      assert.equal(readCooldownPending(1, readAtRate, successfulRead), true); await gate;
      assert.equal(readCooldownPending(1, readAtRate, successfulRead), false);
    }, changed: (a, b) => ({ token: false, cursor: a !== b }) },
    discover: async () => { calls++; const raw = calls === 1 ? "unavailable" : "complete";
      return { phase: queryRateInvalidated(raw, calls === 1 ? 429 : 200, true) ? "stale" : raw, value: calls }; },
  }, { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 }).then(value => { finished = true; return value; });
  await new Promise<void>(done => setImmediate(done)); assert.equal(calls, 1); assert.equal(finished, false);
  // Completion of the old in-flight request does not establish fresh admission.
  successfulRead = 5; await new Promise<void>(done => setImmediate(done)); assert.equal(calls, 1);
  successfulRead = 6; release(); const result = await work;
  assert.equal(result.value, 2); assert.equal(result.context, 6); assert.equal(result.rediscovered, true);
});

test("query429 exception cannot swallow native unsupported, auth, other HTTP failures or uncertainty", () => {
  assert.equal(queryRateInvalidated("unavailable", 429, true), true);
  for (const [phase, status, hint] of [["unavailable", 200, true], ["unavailable", 401, true], ["unavailable", 403, true],
    ["unavailable", 503, true], ["uncertain", 429, true], ["limited", 429, true], ["unavailable", 429, false]] as const)
    assert.equal(queryRateInvalidated(phase, status, hint), false);
});

test("observed submit429/execution404 permits only the saved SDK bytes and resolves once after consumed", () => {
  const command = prepareCommand({ protocol_version: 2, command_id: "a".repeat(32), expected_revision: "7",
    command: { kind: "decision", token:"v0-d0",answer:{answer:"pay",pay:{step:"finish",undo_on_failure:true}} } });
  const original = { revision: "7", token: "v0-d0", binding: { match: "private-fixture", seat: "0" }, response: {answer:"pay",pay:{step:"finish",undo_on_failure:true}} };
  const ledger = new SubmissionAudit<typeof original>();
  const id = command.envelope.command_id;
  assert.equal(ledger.attempt(id, command.exactBody, original).replay, false);
  original.token = "must-not-replace-original";
  ledger.rateLimited(id, 10);
  assert.throws(() => ledger.resolve(id), /unproven_command_resolution/);
  ledger.missing(id); // Missing is permission for identical delivery, never settlement.
  assert.equal(ledger.resolutions, 0); assert.equal(ledger.active, id);
  const replay = ledger.attempt(id, command.exactBody);
  assert.equal(replay.replay, true); assert.equal(replay.original.token, "v0-d0");
  assert.equal(parsePreparedCommand(command.exactBody).envelope.expected_revision, replay.original.revision);
  assert.deepEqual(replay.original.response, {answer:"pay",pay:{step:"finish",undo_on_failure:true}});
  assert.deepEqual(replay.original.binding, { match: "private-fixture", seat: "0" });
  ledger.accepted(id); assert.throws(() => ledger.resolve(id), /unproven_command_resolution/);
  ledger.consumed(id); ledger.consumed(id); ledger.resolve(id);
  assert.equal(ledger.records.size, 1); assert.equal(ledger.attempts, 2); assert.equal(ledger.replays, 1);
  assert.equal(ledger.resolutions, 1); assert.equal(ledger.recovered, 1); assert.equal(ledger.active, null);
  assert.throws(() => ledger.resolve(id), /unproven_command_resolution/);
});

test("submission replay rejects changed bytes/ID, missing causal evidence and post-resolution duplicates", () => {
  for (const fault of ["changed_bytes", "new_id", "no429", "no404", "new_intent", "reused404", "consumed"]) {
    const ledger = new SubmissionAudit<{ token: string }>(); ledger.attempt("original", "exact-body", { token: "original-context" });
    if (fault !== "no429") ledger.rateLimited("original", 0);
    if (fault !== "no404") ledger.missing("original");
    if (fault === "reused404") ledger.attempt("original", "exact-body");
    if (fault === "consumed") { ledger.accepted("original"); ledger.consumed("original"); ledger.resolve("original"); }
    assert.throws(() => ledger.attempt(fault === "new_id" ? "replacement" : "original",
      fault === "changed_bytes" ? "different-context-or-payload" : "exact-body", fault === "new_intent" || fault === "new_id" ? { token: "new" } : undefined));
  }
  const bounded = new SubmissionAudit<object>(); bounded.attempt("id", "bytes", {});
  for (let attempt = 1; attempt < 5; attempt++) { bounded.rateLimited("id", 0); bounded.missing("id"); bounded.attempt("id", "bytes"); }
  bounded.rateLimited("id", 0); bounded.missing("id");
  assert.throws(() => bounded.attempt("id", "bytes"), /submission_attempt_bound/);
});

const freshStaleCounts = () => ({ native: 0, local: 0, started: 0, ready: 0, freshGenerations: 0,
  contextChanges: 0, tokenChanges: 0, cursorChanges: 0, maxClicks: 0, exhausted: 0 });

test("late no-send UI invalidation re-discovers; a newly arriving429 is not retired as old evidence", async () => {
  const counts = freshStaleCounts(); let calls = 0, rates = 0, ready = false;
  const dispatched: string[] = [];
  const rate = { handled: 0, retired: 0, started: 0, ready: 0, freshDiscoveries: 0 };
  const result = await discoverWithRateRecovery({ allowed: true, end: Date.now() + 1_000, now: Date.now,
    healthy: async () => {}, capture: () => ready ? "new-context" : "old-context", rateCount: () => rates,
    normalStale: { counts, ready: async () => { throw Error("expected_new_rate_lane"); }, changed: (a, b) => ({ token: a !== b, cursor: a !== b }) },
    discover: async () => ({ phase: "complete", value: ++calls === 1 ? "old-menu" : "fresh-menu" }),
    use: async (_context, value) => { if (value === "old-menu") { rates++; return false; } dispatched.push(value); return true; },
    recovered: async () => { ready = true; return rates; },
  }, rate);
  assert.deepEqual(dispatched, ["fresh-menu"]); assert.equal(result.context, "new-context");
  assert.equal(calls, 2); assert.equal(rate.started, 1); assert.equal(rate.retired, 0); assert.equal(counts.local, 1);
  let retries = 0;
  await assert.rejects(discoverWithRateRecovery({ allowed: true, end: Date.now() + 1_000, now: Date.now,
    healthy: async () => {}, capture: () => "context", rateCount: () => 0, recovered: async () => 0,
    normalStale: { counts: freshStaleCounts(), ready: async () => { retries++; }, changed: () => ({ token: false, cursor: false }) },
    discover: async () => ({ phase: "complete", value: "candidate" }), use: async () => { throw Error("unknown_journal"); },
  }, { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 }), /unknown_journal/);
  assert.equal(retries, 0);
});

test("actual browser disabled-click invalidation never dispatches the old candidate", async () => {
  // Unauthenticated driver fault probe only; no product/native responses or
  // metadata are substituted. Plain Playwright captures no artifacts.
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage(); let queries = 0; const clicked: string[] = [];
    const result = await discoverWithRateRecovery({ allowed: true, end: Date.now() + 2_000, now: Date.now,
      healthy: async () => {}, capture: () => queries, rateCount: () => 0, recovered: async () => 0,
      normalStale: { counts: freshStaleCounts(), ready: async () => {}, changed: (a, b) => ({ token: a !== b, cursor: false }) },
      discover: async () => {
        const candidate = ++queries === 1 ? "old" : "fresh";
        await page.setContent(`<button ${candidate === "old" ? "disabled" : ""} onclick="this.dataset.clicked='true'">${candidate}</button>`);
        return { phase: "complete", value: candidate };
      },
      use: async (_context, candidate) => {
        const button = page.getByRole("button", { name: candidate, exact: true });
        try { await button.click({ timeout: candidate === "old" ? 50 : 1_000 }); }
        catch (error) { assert.equal((error as Error).name, "TimeoutError"); assert.equal(await button.getAttribute("data-clicked"), null); return false; }
        assert.equal(await button.getAttribute("data-clicked"), "true"); clicked.push(candidate); return true;
      },
    }, { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 });
    assert.equal(queries, 2); assert.deepEqual(clicked, ["fresh"]); assert.equal(result.value, "fresh");
  } finally { await browser.close(); }
});

test("normal native stale uses a fresh ready-context query and never returns the old selection", async () => {
  const counts = freshStaleCounts(); let ready = false, calls = 0;
  const originals: string[] = [];
  const result = await discoverWithRateRecovery({ allowed: true, end: Date.now() + 1_000, now: Date.now,
    healthy: async () => {}, capture: () => ready ? "new-token-and-cursor" : "original",
    rateCount: () => 0, recovered: async () => { throw Error("no_rate_lane"); },
    normalStale: { counts, ready: async () => { ready = true; }, changed: (a, b) => ({ token: a !== b, cursor: a !== b }) },
    discover: async original => { originals.push(original); calls++; return calls === 1
      ? { phase: "stale", value: ["old-selection"], nativeStale: true } : { phase: "complete", value: ["new-only-candidate"] }; },
  }, { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 });
  assert.deepEqual(originals, ["original", "new-token-and-cursor"]);
  assert.deepEqual(result.value, ["new-only-candidate"]); assert.equal(result.context, "new-token-and-cursor");
  assert.equal(result.rediscovered, true); assert.equal(result.recovered, false);
  assert.deepEqual(counts, { native: 1, local: 0, started: 1, ready: 1, freshGenerations: 1,
    contextChanges: 1, tokenChanges: 1, cursorChanges: 1, maxClicks: 2, exhausted: 0 });
});

test("normal stale and rate recovery share three total UI discovery clicks, then fail typed", async () => {
  for (const withRate of [false, true]) {
    const counts = freshStaleCounts(); let calls = 0, ready = 0;
    const rate = { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 };
    await assert.rejects(discoverWithRateRecovery({ allowed: true, end: Date.now() + 1_000, now: Date.now,
      healthy: async () => {}, capture: () => calls, rateCount: () => withRate ? 1 : 0, recovered: async () => 1,
      normalStale: { counts, ready: async () => { ready++; }, changed: (a, b) => ({ token: a !== b, cursor: false }) },
      discover: async () => { calls++; return { phase: "stale", value: "never-return", nativeStale: true }; },
    }, rate), /persistent_discovery_stale/);
    assert.equal(calls, 3); assert.equal(counts.exhausted, 1); assert.equal(counts.maxClicks, 3);
    assert.equal(counts.native, 3); assert.equal(ready, withRate ? 1 : 2); assert.equal(rate.started, withRate ? 1 : 0);
  }
});

test("normal stale never retries auth, protocol, unsupported or unknown-journal failures", async () => {
  for (const failure of ["unauthorized", "forbidden", "protocol", "unknown_journal", "unavailable"]) {
    let retries = 0;
    await assert.rejects(discoverWithRateRecovery({ allowed: true, end: Date.now() + 1_000, now: Date.now,
      healthy: async () => { if (failure === "unknown_journal") throw Error(failure); }, capture: () => "context",
      rateCount: () => 0, recovered: async () => { throw Error("no_rate_lane"); },
      normalStale: { counts: freshStaleCounts(), ready: async () => { retries++; }, changed: () => ({ token: false, cursor: false }) },
      discover: async () => { if (failure !== "unavailable") throw Error(failure); return { phase: "unavailable", value: "unsupported" }; },
    }, { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 }), new RegExp(failure));
    assert.equal(retries, 0);
  }
});

test("ordinary local invalidation does not reuse retired429 evidence or its menu", async () => {
  const counts = freshStaleCounts(); let calls = 0, ready = false;
  const rate = { handled: 1, started: 0, ready: 0, freshDiscoveries: 0 };
  const result = await discoverWithRateRecovery({ allowed: true, end: Date.now() + 1_000, now: Date.now,
    healthy: async () => {}, capture: () => ready ? "fresh-cursor" : "old-cursor", rateCount: () => 1,
    recovered: async () => { throw Error("retired_cooldown_used"); },
    normalStale: { counts, ready: async () => { ready = true; }, changed: (a, b) => ({ token: false, cursor: a !== b }) },
    discover: async () => { calls++; return { phase: "complete", value: calls === 1 ? "discard" : "fresh" }; }, usable: async () => ready,
  }, rate);
  assert.equal(result.value, "fresh"); assert.equal(result.rediscovered, true); assert.equal(result.recovered, false);
  assert.equal(counts.local, 1); assert.equal(counts.native, 0); assert.equal(counts.cursorChanges, 1);
  assert.equal(rate.started, 0); assert.equal(rate.freshDiscoveries, 0);
});

test("a successful post429 resume retires the cooldown allowance for unrelated later waits", async () => {
  let now = 0, resumes = 10, cooldowns = 0;
  const rateCount = 1, resumesAtRate = 10;
  const effects = { end: 200_000, code: "still_unknown", now: () => now, healthy: () => {},
    ratePending: () => readCooldownPending(rateCount, resumesAtRate, resumes), onCooldown: () => { cooldowns++; },
    wait: async () => { now += 1_000; if (now === 35_000) resumes++; },
  };
  assert.equal(await waitForUiRead({ ...effects, predicate: async () => now >= 36_000 }), true);
  assert.equal(now, 36_000); assert.equal(cooldowns, 1);
  // No new429: an unrelated missing receipt must fail at its normal20s bound,
  // rather than extending again or counting the old cooldown a second time.
  await assert.rejects(waitForUiRead({ ...effects, predicate: async () => false }), /still_unknown/);
  assert.equal(now, 56_000); assert.equal(cooldowns, 1);
});

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

test("coherent discoveries retire unused 429 attribution before an unrelated later stale menu", async () => {
  const counts = { handled: 0, started: 0, ready: 0, freshDiscoveries: 0, retired: 0 };
  let now = 0, phase = "complete", rediscoveries = 0;
  const effects = { allowed: true, end: 1_000_000, now: () => now,
    healthy: async () => {}, capture: () => "current-original", rateCount: () => 1,
    discover: async () => ({ phase, value: "current-menu" }),
    recovered: async () => { rediscoveries++; return 1; },
  };
  for (let menu = 0; menu < 3; menu++) {
    assert.equal((await discoverWithRateRecovery(effects, counts)).recovered, false);
  }
  assert.equal(counts.handled, 1); assert.equal(counts.retired, 1);
  now = 565_000; phase = "stale";
  await assert.rejects(discoverWithRateRecovery(effects, counts), /discovery_stale/);
  assert.equal(rediscoveries, 0);
  assert.equal(counts.started, 0); assert.equal(counts.ready, 0);
});

test("complete discovery invalidated by concurrent read429 cannot escape as a usable menu", async () => {
  let recovered = false, calls = 0;
  const contexts: string[] = [];
  const result = await discoverWithRateRecovery({ allowed: true, end: Date.now() + 1_000, now: Date.now,
    healthy: async () => {}, capture: () => recovered ? "fresh-original" : "old-original", rateCount: () => 1,
    discover: async context => { contexts.push(context); calls++; return { phase: "complete", value: calls === 1 ? "discard-this-menu" : "fresh-menu" }; },
    usable: async () => recovered, recovered: async () => { recovered = true; return 1; },
  }, { handled: 0, started: 0, ready: 0, freshDiscoveries: 0 });
  assert.deepEqual(contexts, ["old-original", "fresh-original"]);
  assert.deepEqual(result, { context: "fresh-original", value: "fresh-menu", recovered: true });
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

test("natural verdict rejects operational pauses and consumed prefixes without a completed visible outcome", () => {
  const audit = new ObserverAudit();
  const evidence = { recordState: "paused", recordedOutcome: null, observedOutcome: null, outcomeVisible: false, journalUnresolved: false };
  // The actual 139-command run had complete command proofs, empty journal,
  // paused/native_unavailable and no result. Those facts cannot pass a game.
  assert.equal(audit.verdict("", true).passed, true); // Prefix mode retains its contract.
  for (const state of ["paused", "interrupted", "active", "provisioning", ""]) {
    assert.deepEqual(audit.verdict("", true, { ...evidence, recordState: state }), { passed: false, failure: "natural_game_not_completed" });
  }
  assert.equal(audit.verdict("", true, { ...evidence, recordState: "completed" }).passed, false);
  assert.equal(audit.verdict("", true, { ...evidence, observedOutcome: { outcome: "win", player: 0 }, outcomeVisible: true }).passed, false);
});

test("natural verdict requires matching typed result, visible owner outcome and resolved journal after observer drain", async () => {
  const audit = new ObserverAudit();
  const evidence = { recordState: "completed", recordedOutcome: { kind: "win" as const, winner: 0 },
    observedOutcome: { outcome: "win" as const, player: 0 }, outcomeVisible: true, journalUnresolved: false };
  assert.equal(audit.verdict("", true, evidence).passed, true);
  assert.equal(audit.verdict("", true, { ...evidence, observedOutcome: null }).passed, true); // Durable owner result survives native shutdown.
  assert.equal(audit.verdict("", true, { ...evidence, recordedOutcome: { kind: "draw" }, observedOutcome: { outcome: "draw" } }).passed, true);
  for (const patch of [{ outcomeVisible: false }, { journalUnresolved: true }, { observedOutcome: { outcome: "draw" as const } },
    { observedOutcome: { outcome: "win" as const, player: 1 } }, { recordedOutcome: { kind: "win" as const, winner: 4 } }]) {
    assert.equal(audit.verdict("", true, { ...evidence, ...patch }).passed, false);
  }
  assert.equal(audit.verdict("cleanup_failed", true, evidence).failure, "cleanup_failed");
  assert.equal(audit.verdict("", false, evidence).failure, "all_command_proofs");
  audit.track(async () => { audit.pageErrors++; }); await audit.drain(1_000);
  assert.equal(audit.verdict("", true, evidence).failure, "page_error");
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
