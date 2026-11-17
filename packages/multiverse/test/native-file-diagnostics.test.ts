// Synthetic filesystem/transport only. No listener, child, browser or native host.
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { syncBuiltinESMExports } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { GameDiagnosticEvent, GameDiagnosticSink, GameRecord } from "@mtg-ecosystem/deck-builder/contracts";
import { readPrivate } from "../src/native-private.ts";
import { readNativeBootstrap } from "../src/native-bootstrap.ts";
import { nativeDiagnostics, type NativeFileDetail } from "../src/native-diagnostics.ts";
import { nativeStatus } from "../src/native-status.ts";

const identity = { catalog_sha256: "d".repeat(64), profile: "fixture", engine_build_id: "e".repeat(64), server_build_id: "f".repeat(64) };
const game: GameRecord = { id: "a".repeat(32), identity, deck: { deck_id: 1, revision: 1, cards: [], sha256: "b".repeat(64) }, state: "active", pause: null, outcome: null, created_at: 1, updated_at: 1, human_waiting_since: null, consumers_stopped: false };
const lstat = fs.lstatSync, readFile = fs.readFileSync;
function fixture(sink?: GameDiagnosticSink) {
  const directory = fs.mkdtempSync(join(tmpdir(), "private-file-details-"));
  const state = { session: "session-" + "1".repeat(32), stopping: false, child: { exitCode: null as number | null, signalCode: null as string | null },
    origin: "http://127.0.0.1:18741", bots: { status: () => ["running", "running", "running"] as const } };
  fs.mkdirSync(join(directory, state.session), { mode: 0o700 });
  const seat = join(directory, state.session, "seat-0.json"), status = join(directory, "status.json");
  fs.writeFileSync(seat, JSON.stringify({ protocol_version: 1, bootstrap_version: 1, match_id: game.id, viewer: 0,
    engine_build_id: identity.engine_build_id, server_build_id: identity.server_build_id, catalog_sha256: identity.catalog_sha256,
    server_origin: state.origin, capability: "c".repeat(64) }), { mode: 0o600 });
  fs.writeFileSync(status, JSON.stringify({ version: 1, human_waiting_since: null, human_decision_token: null }), { mode: 0o600 });
  const events: GameDiagnosticEvent[] = [];
  const input = { identity, state,
    bootstrap: (observe?: Parameters<typeof readPrivate>[2]) => readNativeBootstrap(game.identity, identity, () => state, () => directory, observe),
    readStatus: (observe?: Parameters<typeof readPrivate>[2]) => readPrivate(status, 65_536, observe),
    diagnostic: nativeDiagnostics(sink ?? (event => { events.push(event); })) };
  return { directory, seat, status, state, events, input, read: () => nativeStatus(game, new AbortController().signal, input) };
}
const response = () => new Response(JSON.stringify({ protocol_version: 1, match_id: game.id, viewer: 0,
  engine_build_id: identity.engine_build_id, server_build_id: identity.server_build_id, catalog_sha256: identity.catalog_sha256,
  next: "0", state: { status: "pending" }, lifecycle: null, outcome: null }), { headers: { "content-type": "application/json" } });
const details = (event: GameDiagnosticEvent): NativeFileDetail => {
  assert.equal(event.source, "adapter");
  const { file_scope, file_step, file_check, file_links } = event as Extract<GameDiagnosticEvent, { source: "adapter" }>;
  return { file_scope, file_step, file_check, file_links };
};

test("actual status/private read classifies first failed metadata predicate and link bucket without reading", async t => {
  let requests = 0;
  t.mock.method(globalThis, "fetch", async () => { requests++; return response(); });
  const cases = [
    { change: { nlink: 0 }, check: "links", links: "zero" },
    { change: { nlink: 2 }, check: "links", links: "multiple" },
    ...[NaN, -1, 1.5, Number.MAX_SAFE_INTEGER + 1].map(nlink => ({ change: { nlink }, check: "links", links: "invalid" })),
    { change: { uid: (process.getuid?.() ?? 0) + 1 }, check: "owner", links: "one" },
    { change: { uid: (process.getuid?.() ?? 0) + 1, nlink: 0 }, check: "owner", links: "zero" },
    { change: { isFile: () => false }, check: "type", links: "one" },
    { change: { mode: 0o100644 }, check: "mode", links: "one" },
    { change: { size: 65_537 }, check: "size", links: "one" },
  ] as const;
  for (const scope of ["bootstrap_seat", "status"] as const) {
    for (const scenario of cases) {
      const f = fixture(), path = scope === "status" ? f.status : f.seat;
      let reads = 0, stats = 0;
      const before = requests;
      const statMock = t.mock.method(fs, "lstatSync", ((target: fs.PathLike) => {
        const info = lstat(target);
        if (target !== path) return info;
        stats++;
        return Object.assign(Object.create(info), scenario.change, scenario.check === "size" && scope === "bootstrap_seat" ? { size: 4097 } : {});
      }) as typeof fs.lstatSync);
      const readMock = t.mock.method(fs, "readFileSync", ((target: fs.PathOrFileDescriptor, ...args: unknown[]) => {
        if (target === path) reads++;
        return Reflect.apply(readFile, fs, [target, ...args]);
      }) as typeof fs.readFileSync);
      syncBuiltinESMExports();
      try {
        await assert.rejects(f.read(), { message: "Invalid private file" });
        assert.equal(stats, 1); assert.equal(reads, 0);
        assert.equal(requests - before, scope === "status" ? 1 : 0);
        assert.deepEqual(f.events, [{ source: "adapter", game_id: game.id, stage: "status", cause: "private_file",
          file_scope: scope, file_step: "metadata", file_check: scenario.check, file_links: scenario.links }]);
      } finally { statMock.mock.restore(); readMock.mock.restore(); syncBuiltinESMExports(); }
    }
  }
});

test("lstat/read exceptions remain identical at seat and status boundaries; successful files produce no observation", async t => {
  t.mock.method(globalThis, "fetch", async () => response());
  const healthy = fixture();
  assert.deepEqual(await healthy.read(), { identity, consumers: ["running", "running", "running"], human_waiting_since: null, outcome: null });
  assert.deepEqual(healthy.events, []);
  for (const scope of ["bootstrap_seat", "status"] as const) {
    for (const step of ["lstat", "read"] as const) {
      const f = fixture(), path = scope === "status" ? f.status : f.seat;
      const failure = new Error("unsafe path errno token URL contents"), calls: string[] = [];
      const statMock = t.mock.method(fs, "lstatSync", ((target: fs.PathLike) => {
        if (target === path) { calls.push("lstat"); if (step === "lstat") throw failure; }
        return lstat(target);
      }) as typeof fs.lstatSync);
      const readMock = t.mock.method(fs, "readFileSync", ((target: fs.PathOrFileDescriptor, ...args: unknown[]) => {
        if (target === path) { calls.push("read"); throw failure; }
        return Reflect.apply(readFile, fs, [target, ...args]);
      }) as typeof fs.readFileSync);
      syncBuiltinESMExports();
      try {
        await assert.rejects(f.read(), error => error === failure);
        assert.deepEqual(calls, step === "lstat" ? ["lstat"] : ["lstat", "read"]);
        assert.deepEqual(details(f.events[0]!), { file_scope: scope, file_step: step, file_check: undefined, file_links: step === "read" ? "one" : undefined });
        assert.equal(JSON.stringify(f.events).includes("unsafe"), false);
      } finally { statMock.mock.restore(); readMock.mock.restore(); syncBuiltinESMExports(); }
    }
  }
});

test("bootstrap guard remains distinct from seat/status and preserves lazy guard order", async t => {
  let requests = 0;
  t.mock.method(globalThis, "fetch", async () => { requests++; return response(); });
  for (const guard of ["identity", "missing", "session", "stopping", "exit", "signal", "root"] as const) {
    const f = fixture(), calls: string[] = [], failure = new Error("unsafe original root error");
    const state = { ...f.state, child: { ...f.state.child } };
    if (guard === "session") state.session = "";
    if (guard === "stopping") state.stopping = true;
    if (guard === "exit") state.child.exitCode = 0;
    if (guard === "signal") state.child.signalCode = "SIGKILL";
    f.input.bootstrap = observe => readNativeBootstrap(game.identity, guard === "identity" ? { ...identity, profile: "other" } : identity,
      () => { calls.push("state"); return guard === "missing" ? undefined : state; },
      () => { calls.push("root"); throw failure; }, observe);
    const statMock = t.mock.method(fs, "lstatSync", (() => { assert.fail("guard must precede private file metadata"); }) as typeof fs.lstatSync);
    syncBuiltinESMExports();
    try {
      await assert.rejects(f.read(), error => guard === "root" ? error === failure : error instanceof Error && error.message === "Native lifecycle unavailable");
      assert.deepEqual(calls, guard === "identity" ? [] : guard === "root" ? ["state", "root"] : ["state"]);
      assert.deepEqual(f.events, [{ source: "adapter", game_id: game.id, stage: "status", cause: "private_file",
        file_scope: "bootstrap_guard", file_step: "guard", file_check: guard === "identity" || guard === "root" ? "identity" : "lifecycle" }]);
    } finally { statMock.mock.restore(); syncBuiltinESMExports(); }
  }
  assert.equal(requests, 0);
});

test("readPrivate retains size boundaries, real file guards and optional-observer behavior", () => {
  const f = fixture(), observed: NativeFileDetail[] = [];
  fs.writeFileSync(f.status, "x".repeat(65_536));
  assert.equal(readPrivate(f.status, undefined, detail => { observed.push(detail); }).length, 65_536);
  assert.equal(observed.length, 0);
  fs.writeFileSync(f.seat, "x".repeat(4096));
  assert.equal(readPrivate(f.seat, 4096).length, 4096);
  fs.appendFileSync(f.seat, "x");
  assert.throws(() => readPrivate(f.seat, 4096), { message: "Invalid private file" });
  fs.linkSync(f.status, join(f.directory, "hardlink"));
  assert.throws(() => readPrivate(f.status, undefined, detail => { observed.push(detail); }), { message: "Invalid private file" });
  assert.deepEqual(observed.pop(), { file_step: "metadata", file_check: "links", file_links: "multiple" });
  const symlink = join(f.directory, "symlink"); fs.symlinkSync(f.seat, symlink);
  assert.throws(() => readPrivate(symlink, undefined, detail => { observed.push(detail); }), { message: "Invalid private file" });
  assert.deepEqual(observed.pop(), { file_step: "metadata", file_check: "type", file_links: "one" });
});

test("throwing/rejecting/unsettled file observers and sinks cannot replace errors or delay failure", { timeout: 2000 }, async t => {
  const failure = new Error("unsafe original filesystem failure"), f = fixture();
  const statMock = t.mock.method(fs, "lstatSync", (() => { throw failure; }) as typeof fs.lstatSync);
  syncBuiltinESMExports();
  try {
    for (const callback of [undefined, () => { throw Error("unsafe callback"); }, async () => { throw Error("unsafe callback"); }, () => new Promise<void>(() => {})]) {
      assert.throws(() => readPrivate(f.status, undefined, callback), error => error === failure);
      assert.throws(() => readNativeBootstrap(identity, identity, () => undefined, () => f.directory, callback), { message: "Native lifecycle unavailable" });
      const current = fixtureWithoutFiles(callback);
      await assert.rejects(current(), error => error === failure);
    }
  } finally { statMock.mock.restore(); syncBuiltinESMExports(); }
  function fixtureWithoutFiles(sink?: GameDiagnosticSink) {
    return () => nativeStatus(game, new AbortController().signal, { ...f.input, diagnostic: nativeDiagnostics(sink) });
  }
});

test("closed detail reconstruction and legacy missing-detail callbacks preserve earlier event fields", async () => {
  const events: GameDiagnosticEvent[] = [], diagnostic = nativeDiagnostics(event => { events.push(event); });
  diagnostic(game.id, "status", "private_file");
  diagnostic(game.id, "status", "token_mismatch", 2);
  diagnostic(game.id, "status", "private_file", undefined, { file_scope: "status", file_step: "metadata", file_check: "links", file_links: "zero", path: "unsafe", error: "unsafe" } as NativeFileDetail);
  diagnostic(game.id, "status", "private_file", undefined, { file_scope: "unsafe", file_step: "unsafe", file_check: "unsafe", file_links: 0 } as unknown as NativeFileDetail);
  diagnostic(game.id, "status", "status_decode", undefined, { file_scope: "status", file_links: "one" });
  assert.deepEqual(events[0], { source: "adapter", game_id: game.id, stage: "status", cause: "private_file" });
  assert.deepEqual(events[1], { source: "adapter", game_id: game.id, stage: "status", cause: "token_mismatch", attempt: 2 });
  assert.deepEqual(details(events[2]!), { file_scope: "status", file_step: "metadata", file_check: "links", file_links: "zero" });
  assert.deepEqual(events[3], events[0]);
  assert.equal("file_scope" in events[4]!, false); assert.equal(JSON.stringify(events).includes("unsafe"), false);
  const f = fixture(), failure = new Error("legacy callback");
  f.input.bootstrap = () => { throw failure; };
  await assert.rejects(f.read(), error => error === failure);
  assert.deepEqual(f.events, [events[0]]);
});

test("status preflight guards retain their original causes before bootstrap access", async t => {
  t.mock.method(globalThis, "fetch", async () => { assert.fail("preflight must precede transport"); });
  for (const scenario of ["missing", "stopping", "bots", "identity", "child"] as const) {
    const f = fixture();
    const input: Parameters<typeof nativeStatus>[2] = { ...f.input, state: { ...f.state } };
    input.bootstrap = () => { assert.fail("preflight must precede bootstrap"); };
    if (scenario === "missing") input.state = undefined;
    if (scenario === "stopping") input.state!.stopping = true;
    if (scenario === "bots") input.state!.bots = undefined;
    if (scenario === "identity") input.identity = { ...identity, profile: "other" };
    if (scenario === "child") input.state!.child = { exitCode: 0, signalCode: null };
    await assert.rejects(nativeStatus(game, new AbortController().signal, input), { message: "Native lifecycle unavailable" });
    assert.deepEqual(f.events, [{ source: "adapter", game_id: game.id, stage: "status",
      cause: scenario === "identity" ? "identity_failure" : scenario === "child" ? "child_already_exited" : "state_unavailable" }]);
  }
});

test("native detail sanitation reads each closed field once and never reads arbitrary extra accessors", () => {
  const events: GameDiagnosticEvent[] = [], diagnostic = nativeDiagnostics(event => { events.push(event); });
  let scopeReads = 0;
  const detail = Object.defineProperties({}, {
    file_scope: { get: () => ++scopeReads === 1 ? "status" : "unsafe" },
    message: { get: () => { assert.fail("must not inspect arbitrary fields"); } },
  });
  diagnostic(game.id, "status", "private_file", undefined, detail);
  assert.equal(scopeReads, 1);
  assert.deepEqual(events, [{ source: "adapter", game_id: game.id, stage: "status", cause: "private_file", file_scope: "status" }]);
});
