import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, lstatSync, existsSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { execFileSync, spawn } from "node:child_process";
import { randomBytes, createHash } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import { createServer } from "node:net";
import { DatabaseSync } from "node:sqlite";
import { createHost } from "@mtg-ecosystem/deck-builder/host";
import type { GameRecord, GameIdentity } from "@mtg-ecosystem/deck-builder/contracts";
import { parseSeatBootstrap, parseClientConfig, GameClient, prepareCommand, PROTOCOL_VERSION } from "@multiverse/game-server-sdk";
import { createNativeLifecycle } from "../src/native-lifecycle.ts";
import { privateDirectory, readPrivate } from "../src/native-private.ts";

const executable = process.env.MULTIVERSE_TEST_NATIVE_EXECUTABLE;
const signal = () => AbortSignal.timeout(150_000);
async function until(predicate: () => boolean, ms = 5000) {
  const end = Date.now() + ms;
  while (!predicate()) { if (Date.now() >= end) throw new Error("Lifecycle condition deadline"); await delay(25); }
}
function nativeFixture() {
  const source = JSON.parse(execFileSync(executable!, ["lifecycle-profile", "commander-foundations-v1"], { encoding: "utf8" }));
  const identity: GameIdentity = { profile: source.profile, catalog_sha256: source.catalog_sha256, engine_build_id: source.engine_build_id, server_build_id: source.server_build_id };
  const directory = mkdtempSync(join(tmpdir(), "multiverse-native-lifecycle-"));
  const selected = source.decks[0] as { commander: string; cards: { oracle_id: string; count: number }[] };
  const capture = { deck_id: 1, revision: 1, cards: selected.cards.map(c => ({ oracle_id: c.oracle_id, quantity: c.count, role: c.oracle_id === selected.commander ? "commander" as const : "card" as const })).sort((a, b) => a.oracle_id.localeCompare(b.oracle_id)) };
  const game: GameRecord = { id: randomBytes(16).toString("hex"), deck: { ...capture, sha256: createHash("sha256").update(JSON.stringify(capture)).digest("hex") }, identity, state: "provisioning", pause: null, outcome: null, created_at: Date.now(), updated_at: Date.now(), human_waiting_since: null, consumers_stopped: false };
  const options = { executable: executable!, directory, identity, humanOrigin: "http://127.0.0.1:8787" };
  return { game, options, root: join(directory, game.id) };
}
function privateTree(path: string) {
  for (const name of readdirSync(path)) {
    const file = join(path, name), info = lstatSync(file);
    assert.equal(info.isSymbolicLink(), false);
    assert.equal(info.mode & 0o777, info.isDirectory() ? 0o700 : 0o600, file);
    if (info.isDirectory()) privateTree(file);
  }
}

test("private lifecycle storage refuses public roots and symlink-free modes remain explicit", () => {
  assert.throws(() => privateDirectory(tmpdir()), /0700/);
});

test("real native lifecycle deduplicates, recovers journals/identity, isolates three consumers and fences uncertain ensure", { skip: !executable, timeout: 300_000 }, async () => {
  const f = nativeFixture();
  let adapter = createNativeLifecycle(f.options);
  try {
    await Promise.all([adapter.ensure(f.game, signal()), adapter.ensure(f.game, signal())]);
    const first = parseSeatBootstrap(await adapter.humanBootstrap(f.game, signal()));
    assert.equal(first.viewer, 0); assert.equal(first.server_origin, f.options.humanOrigin);
    assert.equal(adapter.validateHumanBootstrap(JSON.stringify({ ...first, operator: "forbidden" }), f.game), false);
    const session = readdirSync(f.root).find(n => n.startsWith("session-"))!;
    const seats = [0, 1, 2, 3].map(seat => parseSeatBootstrap(readPrivate(join(f.root, session, `seat-${seat}.json`))));
    assert.equal(new Set(seats.map(s => s.capability)).size, 4);
    for (const seat of seats.slice(1)) {
      const own = await new GameClient({ baseURL: seat.server_origin, matchId: seat.match_id, capability: seat.capability }).resume();
      assert.equal(own.viewer, seat.viewer);
      assert.equal(own.state.status === "available" ? own.state.observation.view.viewer : seat.viewer, seat.viewer);
    }
    assert.deepEqual((await adapter.status(f.game, signal())).consumers, ["running", "running", "running"]);
    const wait = (await adapter.status(f.game, signal())).human_waiting_since;
    assert.ok(wait);
    await delay(150); assert.equal((await adapter.status(f.game, signal())).human_waiting_since, wait);
    privateTree(f.root);
    const config = readFileSync(join(f.root, "registry", "matches", f.game.id, "config.json"));
    const operator = readFileSync(join(f.root, "operator.json"));
    // Simulate a caller losing the create result, then process-lifetime release.
    await adapter.stop({ id: f.game.id, identity: f.game.identity }, "shutdown", signal());
    adapter = createNativeLifecycle(f.options);
    await adapter.ensure(f.game, signal());
    assert.deepEqual(readFileSync(join(f.root, "registry", "matches", f.game.id, "config.json")), config);
    assert.deepEqual(readFileSync(join(f.root, "operator.json")), operator);
    assert.equal(parseSeatBootstrap(await adapter.humanBootstrap(f.game, signal())).capability, first.capability);
    assert.equal((await adapter.status(f.game, signal())).human_waiting_since, wait);
    await adapter.stop(f.game, "native_unavailable", signal());
    // Later authorized recovery owns the next generation of the same match.
    adapter = createNativeLifecycle(f.options);
    await adapter.ensure(f.game, signal());
    assert.deepEqual(readFileSync(join(f.root, "operator.json")), operator);
    await adapter.stop(f.game, "shutdown", signal());
    const uncertain = createNativeLifecycle(f.options), abort = new AbortController();
    const prior = new Set(readdirSync(f.root));
    const creating = uncertain.ensure(f.game, abort.signal);
    await until(() => readdirSync(f.root).some(n => n.startsWith("session-") && !prior.has(n)));
    abort.abort();
    const stopping = uncertain.stop(f.game, "owner_deleted", signal());
    await assert.rejects(creating); await stopping;
    const epoch = JSON.parse(readPrivate(join(f.root, "generation.json")));
    await delay(200); assert.equal(JSON.parse(readPrivate(join(f.root, "generation.json"))), epoch);
    execFileSync(executable!, ["lifecycle-join", f.root], { timeout: 5000 });
    for (const seat of [1, 2, 3]) execFileSync("flock", ["-n", join(f.root, `bot-${seat}`, "lease"), "true"], { timeout: 5000 });
    assert.equal(readdirSync(join(f.root, "registry", "matches")).filter(n => !n.startsWith(".")).length, 1);
  } finally { await adapter.stop(f.game, "shutdown", signal()); }
});

test("real public host delivers only human bootstrap and proxies exact bytes under disposable Actor DB", { skip: !executable, timeout: 240_000 }, async () => {
  const f = nativeFixture();
  const reservation = createServer();
  await new Promise<void>(resolve => reservation.listen(0, "127.0.0.1", resolve));
  const address = reservation.address(); if (!address || typeof address === "string") throw new Error("No test port");
  const productPort = address.port;
  await new Promise<void>(resolve => reservation.close(() => resolve()));
  f.options.humanOrigin = `http://127.0.0.1:${productPort}`;
  const adapter = createNativeLifecycle(f.options);
  const host = await createHost({ mode: "local", port: productPort, databasePath: join(f.options.directory, "actor.sqlite"), dataDirectory: f.options.directory, backgroundJobs: false, games: { native: { ...adapter, lifecycle_qualified: true }, browser_session_qualified: true, operation_timeout_ms: 150_000, supervision_interval_ms: 1_000_000 } });
  // Only a freshly created disposable database is seeded. The public resume door
  // must authorize and reconcile this persisted immutable capture itself.
  const db = new DatabaseSync(host.paths.database);
  db.prepare("INSERT INTO game_launches(id,user_id,request_id,input_hash,record_json) VALUES(?,1,?,?,?)").run(f.game.id, "native_fixture_request", "fixture", JSON.stringify(f.game)); db.close();
  try {
    const { port } = await host.start();
    const base = `http://127.0.0.1:${port}`;
    const resumed = await fetch(`${base}/api/games/${f.game.id}/resume`, { method: "POST", headers: { "Content-Type": "application/json" }, body: "{}" });
    assert.equal(resumed.status, 200); assert.equal((await resumed.json() as { game: GameRecord }).game.state, "active");
    const seatResponse = await fetch(`${base}/api/games/${f.game.id}/seat`, { method: "POST", headers: { "Content-Type": "application/json" }, body: "{}" });
    const seat = parseSeatBootstrap((await seatResponse.json() as { bootstrap_file: string }).bootstrap_file);
    const endpoint = `${base}/api/games/${f.game.id}/native`;
    const config = await fetch(`${endpoint}/client-config.json`);
    assert.equal(parseClientConfig(await config.text()).server_origin, f.options.humanOrigin);
    const client = new GameClient({ baseURL: endpoint, matchId: f.game.id, capability: seat.capability });
    const resume = await client.resume(); assert.equal(resume.viewer, 0);
    assert.equal((await fetch(`${endpoint}/v1/matches/${f.game.id}/resume`)).status, 401);
    assert.equal((await fetch(`${endpoint}/v1/matches/${"f".repeat(32)}/resume`, { headers: { Authorization: `Bearer ${seat.capability}` } })).status, 503);
    if (resume.state.status !== "available" || !resume.state.observation.decision) throw new Error("Expected human opening decision");
    const prepared = prepareCommand({ protocol_version: PROTOCOL_VERSION, command_id: randomBytes(16).toString("hex"), expected_revision: resume.next, command: { kind: "decision", token: resume.state.observation.decision.token, response: { response: "mulligan", take: false } } });
    const exact = `  ${prepared.exactBody}\n`;
    assert.equal((await client.submitExact(exact)).status, "accepted");
    assert.equal((await client.submitExact(exact)).status, "accepted");
    const records = await (await fetch(`${base}/api/games`)).text();
    assert.equal(records.includes(seat.capability), false);
    assert.equal(records.includes("operator"), false);
  } finally { await host.close(); }
});

test("SIGKILL of product parent releases native worker and all three bot lifetime leases", { skip: !executable, timeout: 240_000 }, async () => {
  const f = nativeFixture();
  const code = `import {createNativeLifecycle} from ${JSON.stringify(new URL("../src/native-lifecycle.ts", import.meta.url).href)}; const chunks=[]; for await(const c of process.stdin) chunks.push(c); const f=JSON.parse(Buffer.concat(chunks)); const adapter=createNativeLifecycle(f.options); await adapter.ensure(f.game,AbortSignal.timeout(150000)); console.log('ready');`;
  const child = spawn(process.execPath, ["--input-type=module", "-e", code], { stdio: ["pipe", "pipe", "ignore"] });
  try {
    const ready = new Promise<void>((resolve, reject) => { child.stdout.once("data", () => resolve()); child.once("exit", () => reject(new Error("Product fixture died before readiness"))); });
    child.stdin.end(JSON.stringify(f)); await ready;
    child.kill("SIGKILL");
    await new Promise<void>(resolve => child.once("close", () => resolve()));
    await until(() => {
      try {
        execFileSync("flock", ["-n", join(f.root, "owner.lock"), "true"], { timeout: 1000 });
        for (const seat of [1, 2, 3]) execFileSync("flock", ["-n", join(f.root, `bot-${seat}`, "lease"), "true"], { timeout: 1000 });
        return true;
      } catch { return false; }
    }, 15_000);
    const adapter = createNativeLifecycle(f.options);
    try { await adapter.ensure(f.game, signal()); assert.deepEqual((await adapter.status(f.game, signal())).consumers, ["running", "running", "running"]); }
    finally { await adapter.stop(f.game, "shutdown", signal()); }
  } finally { child.kill("SIGKILL"); }
});
