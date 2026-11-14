import assert from "node:assert/strict";
import test from "node:test";
import { createGameCoordinator } from "../src/games.ts";
import type { GameDeck, GameIdentity, GameRecord, NativeGameAdapter, NativeGameStatus } from "../src/game-contracts.ts";
import { newDeck, testDb, add } from "./helpers.ts";
import { OWNER, agent } from "./actors.ts";
import { exportAccount, accountInfo, accountScopes, deleteAccount } from "../src/accounts.ts";

const identity: GameIdentity = { catalog_sha256: "b".repeat(64), profile: "fixture", engine_build_id: "fixture-engine", server_build_id: "fixture-server" };
function fixture() {
  const db = testDb(), id = newDeck(db);
  add(db, id, "id-atraxa", { role: "commander" }); add(db, id, "id-solring");
  const revision = () => (db.prepare("SELECT revision FROM decks WHERE id=?").get(id) as { revision: number }).revision;
  const matches = new Set<string>(), ensured: GameRecord[] = [], stops: string[] = [];
  let status: NativeGameStatus = { identity, consumers: ["running", "running", "running"], human_waiting_since: null, outcome: null };
  let admitHook: (() => void | Promise<void>) | undefined;
  let ensureHook: (() => void | Promise<void>) | undefined;
  const adapter: NativeGameAdapter = {
    identity, lifecycle_qualified: true,
    async admit(deck) { await admitHook?.(); return { state: "supported", identity, deck_sha256: deck.sha256, launch_ready: true }; },
    async ensure(game) { ensured.push(game); matches.add(game.id); await ensureHook?.(); },
    async status() { return status; },
    async stop(_game, reason) { stops.push(reason); },
    async humanBootstrap() { return "human-seat-only"; },
    validateHumanBootstrap: bytes => bytes === "human-seat-only",
  };
  const coordinator = createGameCoordinator(db, { native: adapter, browser_session_qualified: true, operation_timeout_ms: 50, human_think_time_ms: 100, game_lifetime_ms: 1000, now: () => 500 });
  return { db, id, revision, adapter, coordinator, matches, ensured, stops, setStatus: (s: Partial<NativeGameStatus>) => { status = { ...status, ...s }; }, setAdmit: (fn: typeof admitHook) => { admitHook = fn; }, setEnsure: (fn: typeof ensureHook) => { ensureHook = fn; } };
}
const key = "launch_request_0001";

test("owner and UI authority are enforced below HTTP, foreign IDs stay 404", async () => {
  const f = fixture();
  try {
    const foreign = { ...OWNER, user_id: 999 };
    await assert.rejects(f.coordinator.admission(foreign, f.id, f.revision()), { status: 404 });
    await assert.rejects(f.coordinator.launch(agent("owner-agent"), f.id, f.revision(), key), { status: 403 });
    await assert.rejects(f.coordinator.admission(null!, f.id, f.revision()), { status: 401 });
    assert.equal(f.matches.size, 0);
  } finally { await f.coordinator.close(); f.db.close(); }
});

test("capture preserves Oracle roles/quantities only, excludes maybes, freezes deck across library edits", async () => {
  const f = fixture();
  try {
    f.db.prepare("UPDATE deck_cards SET quantity=3 WHERE oracle_id='id-solring'").run();
    add(f.db, f.id, "id-llanowar"); f.db.prepare("UPDATE deck_cards SET maybe=1,maybe_note='private note' WHERE oracle_id='id-llanowar'").run();
    const result = await f.coordinator.launch(OWNER, f.id, f.revision(), key);
    assert.equal(result.game?.state, "active");
    assert.deepEqual(result.game?.deck.cards, [{ oracle_id: "id-atraxa", role: "commander", quantity: 1 }, { oracle_id: "id-solring", role: "card", quantity: 3 }]);
    assert.equal(JSON.stringify(result).includes("private note"), false);
    const before = result.game!.deck;
    add(f.db, f.id, "id-seedborn");
    const resume = await f.coordinator.resume(OWNER, result.game!.id);
    assert.deepEqual(resume.game!.deck, before);
    assert.ok(accountScopes(f.db).some(s => s.table === "game_launches"));
    assert.equal(JSON.stringify(exportAccount(f.db, OWNER)).includes("human-seat-only"), false);
    const grant = await f.coordinator.humanSeat(OWNER, result.game!.id);
    assert.equal(grant.bootstrap_file, "human-seat-only");
    assert.equal(JSON.stringify(await f.coordinator.list(OWNER)).includes("human-seat-only"), false);
  } finally { await f.coordinator.close(); f.db.close(); }
});

test("stale revision before admission and while native I/O runs never provisions", async () => {
  const f = fixture();
  try {
    assert.equal((await f.coordinator.admission(OWNER, f.id, 0)).state, "stale_revision");
    f.setAdmit(() => { add(f.db, f.id, "id-seedborn"); });
    assert.equal((await f.coordinator.launch(OWNER, f.id, f.revision(), key)).admission.state, "stale_revision");
    assert.equal(f.matches.size, 0);
  } finally { await f.coordinator.close(); f.db.close(); }
});

test("request retries and concurrent retries deduplicate; changed input conflicts", async () => {
  const f = fixture();
  try {
    const revision = f.revision();
    const [a, b] = await Promise.all([f.coordinator.launch(OWNER, f.id, revision, key), f.coordinator.launch(OWNER, f.id, revision, key)]);
    assert.equal(a.game!.id, b.game!.id); assert.equal(f.matches.size, 1);
    await assert.rejects(f.coordinator.launch(OWNER, f.id, revision + 1, key), { status: 409 });
    await assert.rejects(f.coordinator.launch(OWNER, f.id, revision, key + "other"), { status: 409 });
    // Original receipt remains valid after a mutable deck revision changes.
    add(f.db, f.id, "id-seedborn");
    assert.equal((await f.coordinator.launch(OWNER, f.id, revision, key)).game!.id, a.game!.id);
  } finally { await f.coordinator.close(); f.db.close(); }
});

test("uncertain create is durable and recovers the same key across coordinator restart", async () => {
  const f = fixture();
  let successor: ReturnType<typeof createGameCoordinator> | undefined;
  try {
    f.setEnsure(() => { throw new Error("operator-secret"); });
    const first = await f.coordinator.launch(OWNER, f.id, f.revision(), key);
    assert.equal(first.game!.state, "paused"); assert.equal(first.game!.pause, "native_unavailable");
    assert.equal(JSON.stringify(first).includes("operator-secret"), false);
    await f.coordinator.close(); f.setEnsure(undefined);
    successor = createGameCoordinator(f.db, { native: f.adapter, browser_session_qualified: true, now: () => 500 });
    const resumed = await successor.resume(OWNER, first.game!.id);
    assert.equal(resumed.game!.state, "active"); assert.equal(f.matches.size, 1);
    assert.equal(f.ensured.every(game => game.id === first.game!.id), true);
  } finally { await successor?.close(); await f.coordinator.close(); f.db.close(); }
});

test("concurrent conflicting launch inputs remain bound to the original stable request key", async () => {
  const f = fixture();
  try {
    const other = newDeck(f.db); add(f.db, other, "id-atraxa", { role: "commander" });
    const revision = (f.db.prepare("SELECT revision FROM decks WHERE id=?").get(other) as { revision: number }).revision;
    const releases: (() => void)[] = [];
    let entered!: () => void;
    const bothEntered = new Promise<void>(resolve => { entered = resolve; });
    f.setAdmit(() => new Promise<void>(resolve => { releases.push(resolve); if (releases.length === 2) entered(); }));
    const a = f.coordinator.launch(OWNER, f.id, f.revision(), key);
    const b = f.coordinator.launch(OWNER, other, revision, key);
    await bothEntered; for (const release of releases) release();
    const results = await Promise.allSettled([a, b]);
    assert.equal(results.filter(result => result.status === "fulfilled").length, 1);
    const rejected = results.find(result => result.status === "rejected");
    assert.ok(rejected?.status === "rejected"); assert.equal(rejected.reason.status, 409);
    assert.equal(f.matches.size, 1);
  } finally { await f.coordinator.close(); f.db.close(); }
});

test("consumer failure and human timeout stop bots without fabricating a game result", async () => {
  for (const scenario of ["consumer_failed", "human_timeout"] as const) {
    const f = fixture();
    try {
      const first = await f.coordinator.launch(OWNER, f.id, f.revision(), key);
      f.setStatus(scenario === "consumer_failed" ? { consumers: ["running", "failed", "running"] } : { human_waiting_since: 100 });
      await f.coordinator.tick();
      const record = await f.coordinator.get(OWNER, first.game!.id);
      assert.equal(record.state, scenario === "human_timeout" ? "interrupted" : "paused"); assert.equal(record.pause, scenario); assert.equal(record.outcome, null); assert.ok(f.stops.includes(scenario));
      await assert.rejects(f.coordinator.humanSeat(OWNER, record.id), { status: 503 });
    } finally { await f.coordinator.close(); f.db.close(); }
  }
});

test("native typed completion persists and stops consumers; malformed status fails closed", async () => {
  const f = fixture();
  try {
    const game = (await f.coordinator.launch(OWNER, f.id, f.revision(), key)).game!;
    f.setStatus({ outcome: { kind: "win", winner: 2 }, consumers: ["stopped", "stopped", "stopped"] });
    await f.coordinator.tick();
    assert.deepEqual((await f.coordinator.get(OWNER, game.id)).outcome, { kind: "win", winner: 2 });
    assert.ok(f.stops.includes("completed"));
  } finally { await f.coordinator.close(); f.db.close(); }
  const g = fixture();
  try {
    g.setStatus({ consumers: ["running"] as unknown as NativeGameStatus["consumers"] });
    const game = (await g.coordinator.launch(OWNER, g.id, g.revision(), key)).game!;
    assert.equal(game.state, "paused"); assert.equal(game.pause, "identity_mismatch");
  } finally { await g.coordinator.close(); g.db.close(); }
});

test("default environment and incomplete browser/native gates never create game records", async () => {
  const f = fixture();
  try {
    const empty = createGameCoordinator(f.db);
    assert.deepEqual(await empty.admission(OWNER, f.id, f.revision()), { state: "unavailable", reason: "native_unconfigured" });
    await empty.close();
    const incomplete = createGameCoordinator(f.db, { native: f.adapter });
    const result = await incomplete.launch(OWNER, f.id, f.revision(), key);
    assert.equal(result.admission.state, "supported");
    if (result.admission.state === "supported") assert.equal(result.admission.launch_ready, false);
    assert.equal(result.game, undefined); assert.equal(f.matches.size, 0);
    await incomplete.close();
  } finally { await f.coordinator.close(); f.db.close(); }
});

test("deletion during native work cannot release human auth; identity mismatch cannot admit", async () => {
  const f = fixture();
  try {
    f.adapter.admit = async (deck: GameDeck) => ({ state: "supported", identity: { ...identity, catalog_sha256: "c".repeat(64) }, deck_sha256: deck.sha256, launch_ready: true });
    assert.equal((await f.coordinator.admission(OWNER, f.id, f.revision())).state, "unavailable");
    assert.equal(f.matches.size, 0);
  } finally { await f.coordinator.close(); f.db.close(); }
});

test("failed stop acknowledgments are retried after restart, including completed outcomes", async () => {
  const f = fixture(); let successor: ReturnType<typeof createGameCoordinator> | undefined;
  try {
    const game = (await f.coordinator.launch(OWNER, f.id, f.revision(), key)).game!;
    f.setStatus({ outcome: { kind: "draw" } });
    f.adapter.stop = async () => { throw Error("stop unavailable"); };
    await f.coordinator.tick();
    const record = await f.coordinator.get(OWNER, game.id);
    assert.equal(record.state, "completed"); assert.equal(record.consumers_stopped, false);
    await f.coordinator.close();
    let stopped = 0; f.adapter.stop = async () => { stopped++; };
    successor = createGameCoordinator(f.db, { native: f.adapter, browser_session_qualified: true, now: () => 500 });
    await successor.tick();
    assert.equal(stopped, 1); assert.equal((await successor.get(OWNER, game.id)).consumers_stopped, true);
    await successor.tick(); assert.equal(stopped, 1);
  } finally { await successor?.close(); await f.coordinator.close(); f.db.close(); }
});

test("bounded native waits pause uncertain launch; stop and account removal retain supervision", async () => {
  const f = fixture();
  try {
    f.setEnsure(() => new Promise<void>(() => {}));
    const game = (await f.coordinator.launch(OWNER, f.id, f.revision(), key)).game!;
    assert.equal(game.state, "paused"); assert.equal(game.pause, "native_unavailable");
    assert.ok(f.stops.includes("native_unavailable"));
    f.setEnsure(undefined);
    await f.coordinator.resume(OWNER, game.id);
    f.db.prepare("DELETE FROM game_launches WHERE id=?").run(game.id);
    await f.coordinator.tick(); assert.ok(f.stops.includes("owner_deleted"));
    await assert.rejects(f.coordinator.get(OWNER, game.id), { status: 404 });
  } finally { await f.coordinator.close(); f.db.close(); }
});

test("recovered failed stop survives real account deletion and a second coordinator restart with only minimal native identity", async () => {
  const f = fixture();
  let successor: ReturnType<typeof createGameCoordinator> | undefined;
  let recovered: ReturnType<typeof createGameCoordinator> | undefined;
  try {
    f.setEnsure(() => { throw Error("uncertain create"); });
    let attempts = 0;
    f.adapter.stop = async () => { attempts++; throw Error("stop unavailable"); };
    const game = (await f.coordinator.launch(OWNER, f.id, f.revision(), key)).game!;
    await f.coordinator.close();
    successor = createGameCoordinator(f.db, { native: f.adapter, browser_session_qualified: true, now: () => 500 });
    await successor.tick();
    const beforeDelete = attempts;
    f.db.prepare("UPDATE users SET is_admin=0 WHERE id=?").run(OWNER.user_id);
    deleteAccount(f.db, OWNER, accountInfo(f.db, OWNER).handle);
    assert.equal(f.db.prepare("SELECT id FROM game_launches WHERE id=?").get(game.id), undefined);
    const job = f.db.prepare("SELECT target_json FROM game_stop_jobs WHERE id=?").get(game.id) as { target_json: string };
    assert.deepEqual(JSON.parse(job.target_json), { id: game.id, identity });
    assert.equal(job.target_json.includes("deck"), false);
    assert.equal(accountScopes(f.db).some(scope => scope.table === "game_stop_jobs"), false, "stop handoff is explicitly global operational data");
    await successor.tick(); assert.ok(attempts > beforeDelete);
    await successor.close();
    const stopped: unknown[] = [];
    f.adapter.stop = async target => { stopped.push(target); };
    recovered = createGameCoordinator(f.db, { native: f.adapter, browser_session_qualified: true, now: () => 500 });
    await recovered.tick();
    assert.deepEqual(stopped, [{ id: game.id, identity }]);
    assert.equal(f.db.prepare("SELECT id FROM game_stop_jobs WHERE id=?").get(game.id), undefined);
  } finally { await recovered?.close(); await successor?.close(); await f.coordinator.close(); f.db.close(); }
});

test("native secret-bearing extras never enter admission and invalid human delivery is refused", async () => {
  const f = fixture();
  try {
    f.adapter.identity = { ...identity, operator: "operator-secret" } as GameIdentity;
    const game = (await f.coordinator.launch(OWNER, f.id, f.revision(), key)).game!;
    assert.equal(JSON.stringify(game).includes("operator-secret"), false);
    f.adapter.validateHumanBootstrap = () => false;
    await assert.rejects(f.coordinator.humanSeat(OWNER, game.id), { status: 503 });
    f.adapter.validateHumanBootstrap = () => true;
    f.adapter.humanBootstrap = async () => { f.db.prepare("DELETE FROM game_launches WHERE id=?").run(game.id); return "human-seat-only"; };
    await assert.rejects(f.coordinator.humanSeat(OWNER, game.id), { status: 404 });
  } finally { await f.coordinator.close(); f.db.close(); }
});

test("expired lifetime is an interrupted non-rules result and allows another game only after joined consumers", async () => {
  const f = fixture(); let successor: ReturnType<typeof createGameCoordinator> | undefined;
  try {
    const game = (await f.coordinator.launch(OWNER, f.id, f.revision(), key)).game!;
    await f.coordinator.close();
    successor = createGameCoordinator(f.db, { native: f.adapter, browser_session_qualified: true, now: () => 10_000, game_lifetime_ms: 100 });
    await successor.tick();
    const result = await successor.get(OWNER, game.id);
    assert.equal(result.state, "interrupted"); assert.equal(result.outcome, null); assert.equal(result.pause, "lifetime_exceeded"); assert.equal(result.consumers_stopped, true);
    const next = await successor.launch(OWNER, f.id, f.revision(), key + "new");
    assert.notEqual(next.game!.id, game.id);
  } finally { await successor?.close(); await f.coordinator.close(); f.db.close(); }
});

test("game HTTP endpoints require current human auth, reject cross-origin writes and release SQLite during native I/O", async () => {
  const { createApp } = await import("../src/app.ts");
  const { databaseGate } = await import("../src/auth-database.ts");
  const f = fixture();
  const savedOrigin = process.env.DECKBUILDER_PUBLIC_ORIGIN;
  let current: typeof OWNER | null = OWNER;
  let app: ReturnType<typeof createApp> | undefined;
  let release: (() => void) | undefined;
  try {
    process.env.DECKBUILDER_PUBLIC_ORIGIN = "http://127.0.0.1:8787";
    app = createApp(f.db, { mode: "hosted", resolveUiActor: () => current, games: { native: f.adapter, browser_session_qualified: false } });
    await new Promise<void>(resolve => app!.server.listen(0, "127.0.0.1", resolve));
    const address = app.server.address(); assert.ok(address && typeof address !== "string");
    const base = `http://127.0.0.1:${address.port}`; process.env.DECKBUILDER_PUBLIC_ORIGIN = base;
    const post = (path: string, origin?: string) => fetch(base + path, { method: "POST", headers: { "Content-Type": "application/json", ...(origin ? { Origin: origin } : {}) }, body: JSON.stringify({ deck_id: f.id, expected_revision: f.revision(), request_id: key }) });
    assert.equal((await post("/api/games/admission")).status, 403);
    assert.equal((await post("/api/games/admission", "http://foreign.invalid")).status, 403);
    current = null; assert.equal((await post("/api/games/admission", base)).status, 401);
    current = agent("agent"); assert.equal((await post("/api/games/admission", base)).status, 403);
    current = OWNER;
    let entered!: () => void;
    const started = new Promise<void>(resolve => { entered = resolve; });
    f.setAdmit(() => { entered(); return new Promise<void>(resolve => { release = resolve; }); });
    const pending = post("/api/games/admission", base);
    await started;
    // The shared auth query can run while the outbound native call is blocked.
    const count = await databaseGate(f.db).run(() => (f.db.prepare("SELECT count(*) n FROM users").get() as { n: number }).n);
    assert.ok(count > 0); release!();
    const response = await pending; assert.equal(response.status, 200); assert.equal(response.headers.get("cache-control"), "no-store");
    const result = await response.json(); assert.equal(result.admission.state, "supported"); assert.equal(result.admission.launch_ready, false);
    const games = await fetch(base + "/api/games").then(r => r.json()); assert.deepEqual(games.games, []);
    assert.equal((await post("/api/games/" + "a".repeat(32) + "/seat", base)).status, 404);
  } finally {
    release?.(); await app?.close(); await f.coordinator.close(); f.db.close();
    if (savedOrigin === undefined) delete process.env.DECKBUILDER_PUBLIC_ORIGIN; else process.env.DECKBUILDER_PUBLIC_ORIGIN = savedOrigin;
  }
});
