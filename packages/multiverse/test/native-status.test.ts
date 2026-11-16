import test from "node:test";
import childProcesses from "node:child_process";
import { syncBuiltinESMExports } from "node:module";
import { EventEmitter } from "node:events";
import { PassThrough } from "node:stream";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, readFileSync, chmodSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { GameDiagnosticEvent, GameDiagnosticSink, GameRecord } from "@mtg-ecosystem/deck-builder/contracts";
import { nativeStatus } from "../src/native-status.ts";
import { nativeDiagnostics, nativeFailureCause } from "../src/native-diagnostics.ts";
import { readPrivate, privateDirectory } from "../src/native-private.ts";
import { createNativeLifecycle } from "../src/native-lifecycle.ts";

const identity = { catalog_sha256: "d".repeat(64), profile: "fixture", engine_build_id: "e".repeat(64), server_build_id: "f".repeat(64) };
const game: GameRecord = { id: "a".repeat(32), identity, deck: { deck_id: 1, revision: 1, cards: [], sha256: "b".repeat(64) }, state: "active", pause: null, outcome: null, created_at: 1, updated_at: 1, human_waiting_since: null, consumers_stopped: false };
const resume = () => ({ protocol_version: 1, match_id: game.id, viewer: 0, engine_build_id: identity.engine_build_id,
  server_build_id: identity.server_build_id, catalog_sha256: identity.catalog_sha256, next: "0", state: { status: "pending" }, lifecycle: null, outcome: null });
const json = (value: unknown) => new Response(JSON.stringify(value), { headers: { "content-type": "application/json" } });
function fixture(sink?: GameDiagnosticSink) {
  const directory = mkdtempSync(join(tmpdir(), "native-status-diagnostics-"));
  const seat = join(directory, "seat.json"), status = join(directory, "status.json");
  writeFileSync(seat, JSON.stringify({ protocol_version: 1, bootstrap_version: 1, match_id: game.id, viewer: 0,
    engine_build_id: identity.engine_build_id, server_build_id: identity.server_build_id, catalog_sha256: identity.catalog_sha256,
    server_origin: "http://127.0.0.1:18741", capability: "c".repeat(64) }), { mode: 0o600 });
  const setStatus = (human_decision_token: string | null) => writeFileSync(status, JSON.stringify({ version: 1, human_waiting_since: null, human_decision_token }), { mode: 0o600 });
  setStatus(null);
  const events: GameDiagnosticEvent[] = [];
  const input = { identity, state: { child: { exitCode: null as number | null, signalCode: null as string | null }, stopping: false,
    origin: "http://127.0.0.1:18741", bots: { status: () => ["running", "running", "running"] as const } },
    bootstrap: () => readPrivate(seat), readStatus: () => readPrivate(status),
    diagnostic: nativeDiagnostics(sink ?? (event => { events.push(event); })) };
  return { directory, seat, status, setStatus, events, input, read: (signal = new AbortController().signal) => nativeStatus(game, signal, input) };
}

test("status uses real SDK transport and private files, retaining all four mismatches without token values", async t => {
  const f = fixture(); let requests = 0;
  t.mock.method(globalThis, "fetch", async (url: string, init: RequestInit) => {
    requests++; assert.equal(url.endsWith(`/v1/matches/${game.id}/resume`), true);
    assert.equal((init.headers as Record<string, string>).Authorization, `Bearer ${"c".repeat(64)}`);
    return json(resume());
  });
  assert.deepEqual((await f.read()).consumers, ["running", "running", "running"]);
  assert.equal(f.events.length, 0);
  f.setStatus("private-decision-token");
  await assert.rejects(f.read());
  assert.equal(requests, 5);
  assert.deepEqual(f.events.map(event => [event.cause, "attempt" in event ? event.attempt : undefined]), [
    ["token_mismatch", 1], ["token_mismatch", 2], ["token_mismatch", 3], ["token_mismatch", 4], ["token_mismatch_exhausted", undefined],
  ]);
  assert.equal(JSON.stringify(f.events).includes("private-decision-token"), false);
  assert.equal(JSON.stringify(f.events).includes("c".repeat(64)), false);
});

test("status distinguishes HTTP/schema/transport/identity/private-file/decode/child-exit/abort failures", async t => {
  for (const scenario of ["http_failure", "schema_failure", "transport_failure", "identity_failure", "private_file", "status_decode", "child_already_exited", "aborted"] as const) {
    const f = fixture(), controller = new AbortController();
    const mock = t.mock.method(globalThis, "fetch", async () => {
      if (scenario === "http_failure") return new Response("unsafe raw private body", { status: 503 });
      if (scenario === "schema_failure") return json({ unsafe: "payload" });
      if (scenario === "transport_failure") throw Error("unsafe token https://private.invalid");
      return json(scenario === "identity_failure" ? { ...resume(), server_build_id: "a".repeat(64) } : resume());
    });
    if (scenario === "private_file") chmodSync(f.status, 0o644);
    if (scenario === "status_decode") writeFileSync(f.status, "unsafe invalid private JSON");
    if (scenario === "child_already_exited") { f.input.state.child.signalCode = "SIGKILL"; f.input.state.stopping = true; }
    if (scenario === "aborted") controller.abort(Error("unsafe aborted reason"));
    await assert.rejects(f.read(controller.signal));
    assert.deepEqual(f.events.map(event => [event.stage, event.cause]), [["status", scenario]]);
    const bytes = JSON.stringify(f.events);
    for (const unsafe of ["unsafe", "private.invalid", "SIGKILL", "private-capability"]) assert.equal(bytes.includes(unsafe), false);
    mock.mock.restore();
  }
  const missingSeat = fixture(); unlinkSync(missingSeat.seat);
  await assert.rejects(missingSeat.read());
  assert.equal(missingSeat.events[0]!.cause, "private_file");
});

test("failed diagnostic sinks cannot change status reconciliation or read arbitrary exception text", async t => {
  const mocked = t.mock.method(globalThis, "fetch", async () => json(resume()));
  for (const sink of [() => { throw Error("unsafe sink"); }, async () => { throw Error("unsafe sink"); }, () => new Promise<void>(() => {})]) {
    const f = fixture(sink); f.setStatus("private-old-token");
    let reads = 0;
    f.input.readStatus = () => { if (++reads === 2) f.setStatus(null); return readPrivate(f.status); };
    assert.equal((await f.read()).outcome, null); assert.equal(reads, 2);
  }
  mocked.mock.restore();
  let inspected = false;
  const error = Object.defineProperties(new Error(), Object.fromEntries(["message", "name", "stack"].map(key => [key, { get() { inspected = true; throw Error("unsafe"); } }])));
  assert.equal(nativeFailureCause(error, new AbortController().signal, "adapter_failure"), "adapter_failure");
  assert.equal(inspected, false);
});

test("product adapter reports early ensure identity and private stop binding failures without spawning", async () => {
  const f = fixture();
  const adapter = createNativeLifecycle({ executable: "/must-not-run", directory: f.directory, identity, humanOrigin: "http://127.0.0.1:18741", diagnostic: event => { f.events.push(event); } });
  await assert.rejects(adapter.ensure({ ...game, identity: { ...identity, profile: "unsafe-profile" } }, new AbortController().signal));
  const root = join(f.directory, game.id); privateDirectory(root);
  writeFileSync(join(root, "binding.json"), "unsafe private malformed binding", { mode: 0o600 });
  await assert.rejects(adapter.stop(game, "native_unavailable", new AbortController().signal));
  assert.deepEqual(f.events.map(event => [event.stage, event.cause]), [["ensure", "identity_failure"], ["stop", "private_file"]]);
  assert.equal(JSON.stringify(f.events).includes("unsafe"), false);
});


test("adapter retains initiating startup failure before cleanup SIGKILL and subsequent stop failure, even with failed sinks", async t => {
  for (const failingSink of [false, true]) {
    const events: GameDiagnosticEvent[] = [], directory = mkdtempSync(join(tmpdir(), "native-cleanup-diagnostics-"));
    const nativeIdentity = { ...identity, profile: "commander-foundations-v1" };
    const captured: GameRecord = { ...game, identity: nativeIdentity, deck: { ...game.deck, cards: [{ oracle_id: "synthetic-commander", role: "commander", quantity: 1 }] } };
    const gameRoot = join(directory, game.id); privateDirectory(gameRoot);
    const botRoot = join(gameRoot, "bot-1"); privateDirectory(botRoot);
    const journalPath = join(botRoot, "journal.json"), journalBytes = '{"binding":"private-synthetic-binding","exactBody":"private-synthetic-command"}';
    writeFileSync(journalPath, journalBytes, { mode: 0o600 });
    let spawns = 0;
    const mocked = t.mock.method(childProcesses, "spawn", (() => {
      spawns++;
      const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), exitCode: null, signalCode: null as string | null });
      if (spawns === 1) {
        child.stdin.once("finish", () => {
          child.signalCode = "SIGKILL";
          child.emit("exit", null, "SIGKILL"); child.emit("close", null, "SIGKILL");
        });
        queueMicrotask(() => child.stdout.write("unsafe private startup body\n"));
      } else queueMicrotask(() => child.emit("close", 1));
      return child;
    }) as unknown as typeof childProcesses.spawn);
    syncBuiltinESMExports();
    try {
      const adapter = createNativeLifecycle({ executable: "/must-not-run", directory, identity: nativeIdentity, humanOrigin: "http://127.0.0.1:18741",
        diagnostic: event => { events.push(event); if (failingSink) return Promise.reject(Error("unsafe sink failure")); } });
      await assert.rejects(adapter.ensure(captured, new AbortController().signal), /Native lifecycle unavailable/);
      assert.deepEqual(events.map(event => [event.stage, event.cause]), [
        ["ensure", "startup_protocol"], ["ensure_cleanup", "cleanup_started"], ["child", "child_exit_during_stop"], ["ensure_cleanup", "cleanup_succeeded"],
      ]);
      await assert.rejects(adapter.stop(captured, "native_unavailable", new AbortController().signal));
      assert.equal(events.at(-1)!.stage, "stop"); assert.equal(events.at(-1)!.cause, "adapter_failure");
      assert.equal(spawns, 2);
      assert.equal(readFileSync(journalPath, "utf8"), journalBytes);
      assert.equal(JSON.stringify(events).includes("unsafe"), false); assert.equal(JSON.stringify(events).includes("SIGKILL"), false);
    } finally { mocked.mock.restore(); syncBuiltinESMExports(); }
  }
});
