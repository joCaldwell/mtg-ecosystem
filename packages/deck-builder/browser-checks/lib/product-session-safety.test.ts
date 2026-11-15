import test from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { chromium } from "playwright-core";
import { ObserverAudit, finishOwnedChild, deadline } from "./product-session-safety.ts";

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
