import { gameApplication } from "../server/index.ts";
import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, readFileSync, readdirSync, lstatSync, existsSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { execFileSync, spawn } from "node:child_process";
import { randomBytes, createHash } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import { createServer } from "node:net";
import { request as httpRequest } from "node:http";
import { DatabaseSync } from "node:sqlite";
import { createHost } from "@mtg-ecosystem/deck-builder/host";
import type { GameRecord, GameIdentity } from "../shared/contracts.ts";
import { parseSeatBootstrap, parseClientConfig, GameClient, prepareCommand, PROTOCOL_VERSION } from "@multiverse/game-server-sdk";
import { createNativeLifecycle } from "../server/native-lifecycle.ts";
import { privateDirectory, readPrivate } from "../server/native-private.ts";

// Driver remains private; a fresh native child explicitly uses 0022 to prove
// that SQLite/credentials are 0600 independently of a convenient parent umask.
process.umask(0o077);
const executable = process.env.MULTIVERSE_TEST_NATIVE_EXECUTABLE;
const signal = () => AbortSignal.timeout(150_000);
async function until(predicate: () => boolean, ms = 5000) {
  const end = Date.now() + ms;
  while (!predicate()) { if (Date.now() >= end) throw new Error("Lifecycle condition deadline"); await delay(25); }
}
function nativeFixture() {
  const source = JSON.parse(execFileSync(executable!, ["lifecycle-profile", "commander-foundations-v1"], { encoding: "utf8" }));
  const identity: GameIdentity = { profile: source.profile, catalog_sha256: source.catalog_sha256, engine_build_id: source.engine_build_id, server_build_id: source.server_build_id };
  const directory = mkdtempSync(join(process.env.MULTIVERSE_TEST_NATIVE_EVIDENCE_DIRECTORY ?? tmpdir(), "multiverse-native-lifecycle-"));
  const selected = source.decks[0] as { commander: string; cards: { oracle_id: string; count: number }[] };
  const capture = { deck_id: 1, revision: 1, cards: selected.cards.map(c => ({ oracle_id: c.oracle_id, quantity: c.count, role: c.oracle_id === selected.commander ? "commander" as const : "card" as const })).sort((a, b) => a.oracle_id.localeCompare(b.oracle_id)) };
  const game: GameRecord = { id: randomBytes(16).toString("hex"), deck: { ...capture, sha256: createHash("sha256").update(JSON.stringify(capture)).digest("hex") }, identity, state: "provisioning", pause: null, outcome: null, created_at: Date.now(), updated_at: Date.now(), human_waiting_since: null, consumers_stopped: false };
  const wrapper = join(directory, "native-0022");
  const quoted = "'" + executable!.replaceAll("'", String.fromCharCode(39, 92, 39, 39)) + "'";
  writeFileSync(wrapper, ["#!/bin/sh", "umask 0022", `exec ${quoted} "$@"`, ""].join("\n"), { mode: 0o700 });
  const options = { executable: wrapper, directory, identity, humanOrigin: "http://127.0.0.1:8787" };
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

test("logical human origin accepts explicit numeric default port and rejects alternate destinations", () => {
  const directory = mkdtempSync(join(tmpdir(), "multiverse-origin-boundary-"));
  const identity = { profile: "fixture", catalog_sha256: "a".repeat(64), engine_build_id: "engine", server_build_id: "server" };
  assert.doesNotThrow(() => createNativeLifecycle({ executable: "/unused", directory, identity, humanOrigin: "http://127.0.0.1:80" }));
  for (const humanOrigin of ["http://localhost:8787", "https://127.0.0.1:8787", "http://127.0.0.1:8787/path", "http://127.0.0.1:8787?other=1", "http://user@127.0.0.1:8787"]) assert.throws(() => createNativeLifecycle({ executable: "/unused", directory, identity, humanOrigin }));
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
    assert.equal(adapter.validateHumanBootstrap(JSON.stringify({ ...first, capability: seats[1]!.capability }), f.game), false);
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
    void creating.catch(() => {});
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
  const host = await createHost({ mode: "local", port: productPort, databasePath: join(f.options.directory, "actor.sqlite"), dataDirectory: f.options.directory, backgroundJobs: false, application: gameApplication({ native: { ...adapter, lifecycle_qualified: true }, browser_session_qualified: true, operation_timeout_ms: 150_000, supervision_interval_ms: 1_000_000 }) });
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
    const metadata = await client.presentation();
    assert.equal(metadata.descriptor.catalog_sha256, f.game.identity.catalog_sha256);
    const manifest = await client.manifest();
    const { producePresentation } = await import("@multiverse/game-server-sdk/presentation-producer");
    const expected = await producePresentation(manifest, { match_id: f.game.id, ...f.game.identity });
    const artifactURL = endpoint + metadata.descriptor.path;
    const artifactResponse = await fetch(artifactURL); // Actor gate, no native bearer.
    assert.equal(artifactResponse.status, 200);
    assert.equal(artifactResponse.headers.get("cache-control"), "no-store");
    assert.equal(await artifactResponse.text(), new TextDecoder().decode(expected.bytes));
    assert.ok(metadata.artifact.cards.every(card => card.faces.every(face => face.name.length > 0)));
    assert.equal((await fetch(`${endpoint}/v1/matches/${f.game.id}/presentation`)).status, 401);
    assert.equal((await fetch(`${endpoint}/v1/matches/${f.game.id}/presentation`, { headers: { Authorization: `Bearer ${"f".repeat(64)}` } })).status, 503);
    for (const tail of [metadata.descriptor.path + "?query=1", "/presentation/" + "0".repeat(64) + ".json"]) assert.equal((await fetch(endpoint + tail)).status, 503);
    for (const tail of [metadata.descriptor.path + "/other", "/presentation/not-a-digest.json"]) assert.equal((await fetch(endpoint + tail)).status, 404);
    assert.equal((await fetch(artifactURL, { method: "POST", headers: { Authorization: `Bearer ${seat.capability}` } })).status, 503);
    const resume = await client.resume(); assert.equal(resume.viewer, 0);
    assert.equal((await fetch(`${endpoint}/v1/matches/${f.game.id}/resume`)).status, 401);
    assert.equal((await fetch(`${endpoint}/v1/matches/${"f".repeat(32)}/resume`, { headers: { Authorization: `Bearer ${seat.capability}` } })).status, 503);
    // Native request paths bypass URL normalization to verify aliases are
    // rejected at the authenticated proxy, before any upstream request.
    for (const tail of ["commands/../resume", "%72esume"]) {
      const status = await new Promise<number | undefined>((resolve, reject) => {
        const request = httpRequest({ hostname: "127.0.0.1", port, path: `/api/games/${f.game.id}/native/v1/matches/${f.game.id}/${tail}`, headers: { Authorization: `Bearer ${seat.capability}` } }, response => { response.resume(); response.once("end", () => resolve(response.statusCode)); });
        request.once("error", reject); request.end();
      });
      assert.equal(status, 400);
    }
    for (const tail of [metadata.descriptor.path.replace("/presentation/", "/presentation/../presentation/"), metadata.descriptor.path.replace("presentation", "%70resentation")]) {
      const status = await new Promise<number | undefined>((resolve, reject) => {
        const request = httpRequest({ hostname: "127.0.0.1", port, path: `/api/games/${f.game.id}/native${tail}` }, response => { response.resume(); response.once("end", () => resolve(response.statusCode)); });
        request.once("error", reject); request.end();
      });
      assert.ok(status === 400 || status === 404);
    }
    if (resume.state.status !== "available" || !resume.state.observation.decision) throw new Error("Expected human opening decision");
    const prepared = prepareCommand({ protocol_version: PROTOCOL_VERSION, command_id: randomBytes(16).toString("hex"), expected_revision: resume.next, command: { kind: "decision", token: resume.state.observation.decision.token, answer:{answer:'pick',picks:[[resume.state.observation.decision.candidates.findIndex(c=>c.candidate==='flag'&&c.flag==='keep')]]} } });
    const exact = `  ${prepared.exactBody}\n`;
    assert.equal((await client.submitExact(exact)).status, "accepted");
    assert.equal((await client.submitExact(exact)).status, "accepted");
    const records = await (await fetch(`${base}/api/games`)).text();
    assert.equal(records.includes(seat.capability), false);
    assert.equal(records.includes("operator"), false);
  } finally {
    await host.close();
    // This disposable witness owns only this host and its three consumer leases.
    execFileSync(executable!, ["lifecycle-join", f.root], { timeout: 5000 });
    for (const lease of ["owner.lock", ...[1, 2, 3].map(seat => `bot-${seat}/lease`)]) {
      execFileSync("flock", ["-n", join(f.root, lease), "true"], { timeout: 5000 });
    }
  }
});

test("SIGKILL of product parent releases native worker and all three bot lifetime leases", { skip: !executable, timeout: 240_000 }, async () => {
  const f = nativeFixture();
  const code = `import {createNativeLifecycle} from ${JSON.stringify(new URL("../server/native-lifecycle.ts", import.meta.url).href)}; const chunks=[]; for await(const c of process.stdin) chunks.push(c); const f=JSON.parse(Buffer.concat(chunks)); const adapter=createNativeLifecycle(f.options); await adapter.ensure(f.game,AbortSignal.timeout(150000)); console.log('ready');`;
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
