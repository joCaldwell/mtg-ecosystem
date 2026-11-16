import test from "node:test";
import assert from "node:assert/strict";
import { createGameCoordinator } from "../src/games.ts";
import type { GameDiagnosticEvent, GameDiagnosticSink, NativeGameAdapter } from "../src/contracts.ts";
import { newDeck, testDb, add } from "./helpers.ts";
import { OWNER } from "./actors.ts";

function fixture(diagnostic?: GameDiagnosticSink) {
  const db = testDb(), deck = newDeck(db);
  add(db, deck, "id-atraxa", { role: "commander" });
  const identity = { catalog_sha256: "b".repeat(64), profile: "fixture", engine_build_id: "engine", server_build_id: "server" };
  let stops = 0;
  const native: NativeGameAdapter = {
    identity, lifecycle_qualified: true,
    async admit(deck) { return { state: "supported", identity, deck_sha256: deck.sha256, launch_ready: true }; },
    async ensure() {},
    async status() { return { identity, consumers: ["running", "running", "running"], human_waiting_since: null, outcome: null }; },
    async stop() { stops++; },
    async humanBootstrap() { return "private-seat"; }, validateHumanBootstrap: () => true,
  };
  const coordinator = createGameCoordinator(db, { native, diagnostic, browser_session_qualified: true, operation_timeout_ms: 20 });
  const launch = () => coordinator.launch(OWNER, deck, 1, "diagnostic_request_01");
  return { db, native, coordinator, launch, stops: () => stops };
}

test("private diagnostics retain status deadline before cleanup even when adapter ignores abort", async () => {
  const events: GameDiagnosticEvent[] = [], f = fixture(event => { events.push(event); });
  let aborted = false;
  f.native.status = async (_game, signal) => { signal.addEventListener("abort", () => { aborted = true; }); return new Promise(() => {}); };
  try {
    const result = await f.launch();
    assert.equal(aborted, true);
    assert.equal(result.game!.state, "paused"); assert.equal(result.game!.pause, "native_unavailable");
    assert.equal(result.game!.outcome, null); assert.equal(result.game!.consumers_stopped, true);
    assert.deepEqual(events.map(({ stage, cause }) => [stage, cause]), [["status", "deadline"], ["stop", "started"], ["stop", "succeeded"]]);
    assert.equal(f.stops(), 1);
    assert.equal(JSON.stringify(await f.coordinator.list(OWNER)).includes("deadline"), false);
  } finally { await f.coordinator.close(); f.db.close(); }
});

test("private diagnostics distinguish initiating ensure/status failures and retained failed cleanup", async () => {
  for (const stage of ["ensure", "status"] as const) {
    const events: GameDiagnosticEvent[] = [], f = fixture(event => { events.push(event); });
    const unsafe = new Error("Bearer private-seat https://private.invalid/deck token raw observation");
    f.native[stage] = async () => { throw unsafe; };
    f.native.stop = async () => { throw unsafe; };
    try {
      const result = await f.launch();
      assert.equal(result.game!.pause, "native_unavailable"); assert.equal(result.game!.consumers_stopped, false);
      assert.deepEqual(events.map(({ stage, cause }) => [stage, cause]), [[stage, "adapter_failure"], ["stop", "started"], ["stop", "adapter_failure"]]);
      const bytes = JSON.stringify(events);
      for (const word of ["Bearer", "private-seat", "private.invalid", "observation", "stack"]) assert.equal(bytes.includes(word), false);
      await f.coordinator.tick();
      assert.equal((await f.coordinator.get(OWNER, result.game!.id)).consumers_stopped, false);
      f.native.stop = async () => {};
      await f.coordinator.tick();
      assert.equal((await f.coordinator.get(OWNER, result.game!.id)).consumers_stopped, true);
    } finally { await f.coordinator.close(); f.db.close(); }
  }
});

test("throwing, rejecting and never-settling diagnostic sinks cannot change pause or cleanup", async () => {
  for (const diagnostic of [() => { throw Error("unsafe sink"); }, async () => { throw Error("unsafe sink"); }, () => new Promise<void>(() => {})]) {
    const f = fixture(diagnostic);
    f.native.status = async () => { throw Error("unsafe adapter"); };
    try {
      const result = await f.launch();
      assert.equal(result.game!.state, "paused"); assert.equal(result.game!.pause, "native_unavailable");
      assert.equal(result.game!.consumers_stopped, true); assert.equal(f.stops(), 1);
      await f.coordinator.tick(); assert.equal(f.stops(), 1);
    } finally { await f.coordinator.close(); f.db.close(); }
  }
});
