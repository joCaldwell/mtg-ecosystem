// Synthetic integration only: no listener, native executable, browser or host.
import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, lstatSync, chmodSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { createPrivateDiagnostics } from "./product-session-fixture.ts";
import { createGameCoordinator } from "../../src/games.ts";
import type { GameDiagnosticEvent, NativeGameAdapter } from "../../src/contracts.ts";
import { testDb, newDeck, add } from "../../test/helpers.ts";
import { OWNER } from "../../test/actors.ts";
import { nativeStatus } from "../../../multiverse/src/native-status.ts";
import { nativeDiagnostics } from "../../../multiverse/src/native-diagnostics.ts";
import { readPrivate } from "../../../multiverse/src/native-private.ts";

type Stored = GameDiagnosticEvent & { sequence: number; at_ms: number };
const parent = () => mkdtempSync(join(tmpdir(), "private-product-diagnostics-"));
const read = (path: string) => JSON.parse(readFileSync(path, "utf8")) as { version: number; events: Stored[] };

test("fixture artifact remains bounded/private and retains initial failure plus latest cleanup after event flood", () => {
  const directory = parent(), sink = createPrivateDiagnostics(directory), game_id = "a".repeat(32);
  sink.diagnostic({ source: "coordinator", game_id, stage: "status", phase: "operation", cause: "deadline" });
  for (let n = 0; n < 1000; n++) sink.diagnostic({ source: "adapter", game_id, stage: "status", cause: "token_mismatch", attempt: (n % 4 + 1) as 1 | 2 | 3 | 4 });
  sink.diagnostic({ source: "coordinator", game_id, stage: "stop", phase: "cleanup", cause: "adapter_failure" });
  const saved = read(sink.path);
  assert.equal(saved.version, 1); assert.equal(saved.events.length, 10);
  assert.equal(saved.events[0]!.cause, "deadline"); assert.equal(saved.events.at(-1)!.cause, "adapter_failure");
  assert.equal(lstatSync(join(directory, "diagnostics")).mode & 0o777, 0o700);
  assert.equal(lstatSync(sink.path).mode & 0o777, 0o600); assert.ok(lstatSync(sink.path).size <= 262_144);
  assert.deepEqual(readdirSync(join(directory, "diagnostics")), ["lifecycle.json"]);
  assert.equal(saved.events.every((row, i, all) => Number.isSafeInteger(row.at_ms) && (i === 0 || row.sequence > all[i - 1]!.sequence)), true);
  // Unknown fields are discarded, never serialized; unknown strings are refused.
  sink.diagnostic({ source: "adapter", game_id, stage: "status", cause: "private_file", error: "unsafe raw body", headers: "unsafe", token: "unsafe", url: "unsafe" } as GameDiagnosticEvent);
  sink.diagnostic({ source: "adapter", game_id, stage: "unsafe", cause: "unsafe" } as unknown as GameDiagnosticEvent);
  sink.diagnostic({ source: "adapter", game_id: "unsafe", stage: "status", cause: "private_file" });
  assert.equal(readFileSync(sink.path, "utf8").includes("unsafe"), false);
  assert.deepEqual(Object.keys(read(sink.path).events.at(-1)!).sort(), ["at_ms", "cause", "game_id", "sequence", "source", "stage"]);
  // A failed disk/privacy check is inert and does not repair/write a public file.
  chmodSync(sink.path, 0o644);
  const before = readFileSync(sink.path, "utf8");
  assert.doesNotThrow(() => sink.diagnostic({ source: "coordinator", game_id, stage: "stop", phase: "cleanup", cause: "deadline" }));
  assert.equal(readFileSync(sink.path, "utf8"), before);
});

test("real coordinator and adapter status boundary persist deadline, mismatch, child exit and stop failure separately", async t => {
  for (const scenario of ["deadline", "mismatch", "child_exit", "stop_failure"] as const) {
    const db = testDb(), deck = newDeck(db); add(db, deck, "id-atraxa", { role: "commander" });
    const directory = parent(), sink = createPrivateDiagnostics(directory);
    const identity = { profile: "fixture", catalog_sha256: "d".repeat(64), engine_build_id: "e".repeat(64), server_build_id: "f".repeat(64) };
    const unsafe = "unsafe private token header observation https://private.invalid";
    let currentId = "", requests = 0, stops = 0;
    const statusFile = join(directory, "status.json"), seatFile = join(directory, "seat.json");
    writeFileSync(statusFile, JSON.stringify({ version: 1, human_waiting_since: null, human_decision_token: scenario === "mismatch" ? unsafe : null }), { mode: 0o600 });
    const fetcher = t.mock.method(globalThis, "fetch", async () => {
      requests++;
      if (scenario === "deadline") return new Promise<Response>(() => {}); // Deliberately ignores abort.
      if (scenario === "stop_failure") throw Error(unsafe);
      return new Response(JSON.stringify({ protocol_version: 1, match_id: currentId, viewer: 0,
        engine_build_id: identity.engine_build_id, server_build_id: identity.server_build_id, catalog_sha256: identity.catalog_sha256,
        next: "0", state: { status: "pending" }, lifecycle: null, outcome: null }), { headers: { "content-type": "application/json" } });
    });
    const native: NativeGameAdapter = {
      identity, lifecycle_qualified: true,
      async admit(deck) { return { state: "supported", identity, deck_sha256: deck.sha256, launch_ready: true }; },
      async ensure(game) {
        currentId = game.id;
        writeFileSync(seatFile, JSON.stringify({ protocol_version: 1, bootstrap_version: 1, match_id: game.id, viewer: 0,
          engine_build_id: identity.engine_build_id, server_build_id: identity.server_build_id, catalog_sha256: identity.catalog_sha256,
          server_origin: "http://127.0.0.1:18741", capability: "c".repeat(64) }), { mode: 0o600 });
      },
      status: (game, signal) => nativeStatus(game, signal, { identity, state: { child: { exitCode: scenario === "child_exit" ? 1 : null, signalCode: null }, stopping: false,
        origin: "http://127.0.0.1:18741", bots: { status: () => ["running", "running", "running"] } },
        bootstrap: () => readPrivate(seatFile), readStatus: () => readPrivate(statusFile), diagnostic: nativeDiagnostics(sink.diagnostic) }),
      async stop() { stops++; if (scenario === "stop_failure") throw Error(unsafe); },
      async humanBootstrap() { return "unused"; }, validateHumanBootstrap: () => false,
    };
    const coordinator = createGameCoordinator(db, { native, diagnostic: sink.diagnostic, browser_session_qualified: true, operation_timeout_ms: scenario === "mismatch" ? 2000 : 30 });
    try {
      const result = await coordinator.launch(OWNER, deck, 1, "diagnostic_fixture_01");
      assert.equal(result.game!.pause, "native_unavailable"); assert.equal(result.game!.state, "paused");
      assert.equal(result.game!.outcome, null); assert.equal(result.game!.consumers_stopped, scenario !== "stop_failure");
      assert.equal(stops, 1);
      const events = read(sink.path).events;
      assert.equal(events[0]!.source, scenario === "deadline" ? "coordinator" : "adapter");
      assert.equal(events[0]!.cause, { deadline: "deadline", mismatch: "token_mismatch", child_exit: "child_already_exited", stop_failure: "transport_failure" }[scenario]);
      assert.equal(events.at(-1)!.cause, scenario === "stop_failure" ? "adapter_failure" : "succeeded");
      if (scenario === "deadline") assert.equal(events[0]!.stage, "status");
      if (scenario === "mismatch") {
        assert.equal(requests, 4);
        assert.deepEqual(events.filter(event => event.cause === "token_mismatch").map(event => "attempt" in event ? event.attempt : null), [1, 2, 3, 4]);
        assert.equal(events[4]!.cause, "token_mismatch_exhausted");
      }
      assert.equal(readFileSync(sink.path, "utf8").includes("unsafe"), false);
      const persisted = (db.prepare("SELECT record_json FROM game_launches").get() as { record_json: string }).record_json;
      assert.equal(persisted.includes("diagnostic"), false); assert.equal(persisted.includes("unsafe"), false);
      if (scenario === "stop_failure") {
        native.stop = async () => { stops++; };
        await coordinator.tick();
        assert.equal((await coordinator.get(OWNER, result.game!.id)).consumers_stopped, true);
      }
    } finally { await coordinator.close(); db.close(); fetcher.mock.restore(); }
  }
});
