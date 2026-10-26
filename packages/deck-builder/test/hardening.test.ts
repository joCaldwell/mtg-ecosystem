import { removeIdentity } from "./legacy-db.ts";
import { existsSync } from "node:fs";
import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";
import type { AddressInfo } from "node:net";
import { OWNER } from "./actors.ts";
import { createApp } from "../src/app.ts";
import { backupDatabase, restoreDatabase } from "../src/backups.ts";
import { resolveCard } from "../src/cards.ts";
import { DATA_DIR, openDb, PKG_ROOT, SCHEMA_VERSION } from "../src/db.ts";
import { checkDeck } from "../src/deck/checks.ts";
import { markReviewed, writeLog } from "../src/deck/log.ts";
import { createProposal, getProposal } from "../src/deck/proposals.ts";
import { createDeck } from "../src/deck/service.ts";
import { ingestCards, ingestFromFile } from "../src/ingest.ts";
import { acquireJobLock, createJobs, LOCK_MAX_AGE, runJob } from "../src/jobs.ts";
import { remember, reviseMemory } from "../src/memory.ts";
import { callTool } from "../src/mcp/tools.ts";
import { acquireProcessLock } from "../src/process-lock.ts";
import { search } from "../src/search/index.ts";

import { FIXTURES } from "./fixtures.ts";
import { AGENT, add, newDeck, testDb } from "./helpers.ts";

const dir = mkdtempSync(join(tmpdir(), "db-hardening-"));
after(() => rmSync(dir, { recursive: true, force: true }));
const db = testDb();
const web = join(dir, "web");
mkdirSync(join(web, "assets"), { recursive: true });
writeFileSync(join(web, "index.html"), "<!doctype html><title>app</title>");
writeFileSync(join(web, "assets", "bundle.js"), "export {};");
writeFileSync(join(web, "assets", "font.woff2"), "font");
const app = createApp(db, { webDist: web });
let base: string;
before(async () => {
  await new Promise<void>(resolve => app.server.listen(0, "127.0.0.1", resolve));
  base = `http://127.0.0.1:${(app.server.address() as AddressInfo).port}`;
});
after(async () => { await app.close(); db.close(); });
const post = (path: string, body: unknown) => fetch(base + path, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });

test("HTTP rejects non-object JSON, oversized bodies and incorrect Content-Type", async () => {
  for (const value of [null, [], 42, "x", false]) assert.equal((await post("/api/decks", value)).status, 400);
  assert.equal((await post("/api/decks", { name: "x".repeat(1024 * 1024) })).status, 413);
  assert.equal((await fetch(base + "/api/decks", { method: "POST", headers: { "Content-Type": "text/plain" }, body: '{}' })).status, 415);
  assert.equal((await fetch(base + "/api/decks", { method: "POST", headers: { "Content-Type": "application/json" }, body: '{' })).status, 400);
});

test("HTTP rejects NaN, infinite, negative and fractional numeric inputs", async () => {
  for (const value of ["NaN", "Infinity", "", "1.5", "-1"])
    for (const path of ["/api/search?q=&limit=", "/api/activity?limit=", "/api/log?limit="])
      assert.equal((await fetch(base + path + value)).status, 400, path + value);
  assert.equal((await fetch(base + "/api/search?deck=NaN")).status, 400);
  const deck = newDeck(db);
  assert.equal((await post(`/api/decks/${deck}/cards`, { oracle_id: FIXTURES[0].oracle_id, quantity: "NaN" })).status, 400);
  assert.equal((await fetch(base + `/api/decks/${deck}`, { method: "PATCH", headers: { "Content-Type": "application/json" }, body: '{"parent_id":"invalid"}' })).status, 400);
});

test("items/rule requires integer arrays and rejects items and proposals from another deck", async () => {
  const a = newDeck(db), b = newDeck(db);
  const proposalId = createProposal(db, AGENT, b, { items: [{ action: "add", oracle_id: FIXTURES[0].oracle_id, reason: "Test" }] });
  const ids = getProposal(db, OWNER, proposalId)!.items.map(i => i.id);
  for (const item_ids of [null, "1", {}, ["1"], [1.5], []])
    assert.equal((await post(`/api/decks/${a}/items/rule`, { item_ids, outcome: "reject" })).status, 400);
  assert.equal((await post(`/api/decks/${a}/items/rule`, { item_ids: ids, outcome: "reject" })).status, 404);
  assert.equal((await post(`/api/decks/${a}/proposals/${proposalId}/rule`, { outcome: "reject" })).status, 404);
  assert.equal(getProposal(db, OWNER, proposalId)!.items[0].status, "pending");
  assert.equal((await post(`/api/decks/${b}/items/rule`, { item_ids: ids, outcome: "reject" })).status, 200);
});

test("health is plain JSON and static fallback preserves 404s and asset caching", async () => {
  const health = await (await fetch(base + "/api/health")).json();
  assert.deepEqual(Object.keys(health).sort(), ["ok", "schema_version", "cards", "last_ingest"].sort());
  assert.equal(health.ok, true); assert.equal(health.schema_version, SCHEMA_VERSION); assert.ok(health.cards > 0);
  for (const path of ["/api/missing", "/mcp-missing", "/.well-known/missing", "/assets/missing.js", "/missing.png", "/unknown", "/assets/"])
    assert.equal((await fetch(base + path)).status, 404, path);
  const index = await fetch(base + "/deck/1"); assert.equal(index.status, 200); assert.equal(index.headers.get("cache-control"), "no-cache");
  const asset = await fetch(base + "/assets/bundle.js"); assert.match(asset.headers.get("cache-control")!, /immutable/);
  assert.equal((await fetch(base + "/assets/font.woff2")).headers.get("content-type"), "font/woff2");
  const settings = await (await fetch(base + "/api/settings")).json();
  assert.equal(settings.agent.stdio_command, `node ${join(PKG_ROOT, "src", "mcp", "stdio.ts")}`);
  assert.ok(!JSON.stringify(settings).includes(DATA_DIR));
});

test("SSE disables proxy buffering and sends heartbeats", async t => {
  t.mock.timers.enable({ apis: ["setInterval"] });
  const controller = new AbortController();
  const response = await fetch(base + "/api/events", { signal: controller.signal });
  assert.equal(response.headers.get("x-accel-buffering"), "no");
  const reader = response.body!.getReader(); await reader.read();
  t.mock.timers.tick(25_000);
  assert.match(new TextDecoder().decode((await reader.read()).value), /: heartbeat\n\n/);
  await reader.cancel(); controller.abort();
});

test("streaming ingest stores top-level and front-face image URLs on all read interfaces", async () => {
  const cards = structuredClone(FIXTURES);
  const first = cards[0], faced = cards.find(c => c.card_faces?.length)!;
  first.image_uris = { small: "https://cards.scryfall.io/small/test.jpg", normal: "https://cards.scryfall.io/normal/test.jpg", art_crop: "https://cards.scryfall.io/art_crop/test.jpg" };
  faced.card_faces![0].image_uris = { normal: "https://cards.scryfall.io/normal/front.jpg" };
  const path = join(dir, "oracle.json"); writeFileSync(path, JSON.stringify(cards));
  const fresh = openDb(":memory:");
  try {
    await ingestFromFile(fresh, path);
    assert.equal(resolveCard(fresh, first.name).image_small, first.image_uris.small);
    assert.equal(resolveCard(fresh, faced.name).image_normal, faced.card_faces![0].image_uris.normal);
    assert.equal(search(fresh, OWNER, "cmc>=0", { limit: 1000 }).find(c => c.oracle_id === first.oracle_id)!.image_art_crop, first.image_uris.art_crop);
    const tool = callTool({ db: fresh, actor: AGENT }, "get_cards", { cards: [first.name] });
    assert.equal((tool.structuredContent!.cards as any[])[0].image_normal, first.image_uris.normal);
  } finally { fresh.close(); }
});

test("truncation and malformed streams leave live card facts and freshness unchanged", async () => {
  const fresh = testDb();
  try {
    const before = fresh.prepare("SELECT * FROM cards ORDER BY oracle_id").all();
    const marker = fresh.prepare("SELECT value FROM meta WHERE key = 'last_ingest'").get();
    assert.throws(() => ingestCards(fresh, FIXTURES.slice(0, 1)), /90%/);
    const path = join(dir, "broken.json"); writeFileSync(path, JSON.stringify(FIXTURES).slice(0, -1));
    await assert.rejects(ingestFromFile(fresh, path), /Incomplete/);
    assert.deepEqual(fresh.prepare("SELECT * FROM cards ORDER BY oracle_id").all(), before);
    assert.deepEqual(fresh.prepare("SELECT value FROM meta WHERE key = 'last_ingest'").get(), marker);
  } finally { fresh.close(); }
});

test("retired referenced cards remain resolvable, searchable only in their deck, and reported", () => {
  const fresh = testDb(), deck = newDeck(fresh);
  const card = FIXTURES[0]; add(fresh, deck, card.oracle_id);
  try {
    ingestCards(fresh, FIXTURES.filter(c => c.oracle_id !== card.oracle_id));
    assert.equal(resolveCard(fresh, card.name).retired, 1);
    assert.ok(!search(fresh, OWNER, "cmc>=0", { limit: 1000 }).some(c => c.oracle_id === card.oracle_id));
    assert.ok(search(fresh, OWNER, "cmc>=0", { deckId: deck, limit: 1000 }).some(c => c.oracle_id === card.oracle_id));
    assert.ok(checkDeck(fresh, OWNER, deck).some(f => f.key === `retired:${card.oracle_id}`));
    assert.deepEqual(fresh.prepare("PRAGMA foreign_key_check").all(), []);
    ingestCards(fresh, FIXTURES);
    assert.equal(resolveCard(fresh, card.name).retired, 0);
  } finally { fresh.close(); }
});

test("backup captures committed WAL, retains 14 files, and restores a verified snapshot", async () => {
  const target = join(dir, "restore.db"), data = join(dir, "backups-test");
  const source = openDb(target); createDeck(source, OWNER, "Before backup");
  const { file } = await backupDatabase(source, data);
  createDeck(source, OWNER, "After backup"); source.close();
  const held = acquireProcessLock(`${target}.server-lock`);
  await assert.rejects(restoreDatabase(file, target), /running/); held();
  assert.deepEqual(await restoreDatabase(file, target), { decks: 1 });
  const restored = openDb(target);
  assert.equal((restored.prepare("SELECT COUNT(*) n FROM decks").get() as { n: number }).n, 1);
  restored.close();
  const corrupt = join(dir, "not-a-db"); writeFileSync(corrupt, "bad");
  await assert.rejects(restoreDatabase(corrupt, target));
  const stillThere = openDb(target); assert.equal((stillThere.prepare("SELECT name FROM decks").get() as { name: string }).name, "Before backup");
  for (let day = 1; day <= 16; day++) await backupDatabase(stillThere, data, new Date(Date.UTC(2026, 0, day)));
  stillThere.close();
  assert.equal(readdirSync(join(data, "backups")).filter(f => f.endsWith(".db")).length, 14);
});

test("job locks exclude overlap, reclaim stale leases and keep newer holders safe", async () => {
  const fresh = openDb(":memory:"), now = Date.now();
  try {
    const old = acquireJobLock(fresh, now);
    assert.throws(() => acquireJobLock(fresh, now + 1), /running/);
    const current = acquireJobLock(fresh, now + LOCK_MAX_AGE + 1);
    old(); assert.throws(() => acquireJobLock(fresh, now + LOCK_MAX_AGE + 2), /running/);
    current();
    await assert.rejects(runJob(fresh, "refresh", async () => { throw new Error("example"); }), /example/);
    assert.equal(fresh.prepare("SELECT value FROM meta WHERE key = 'job:lock'").get(), undefined);
    const run = JSON.parse((fresh.prepare("SELECT value FROM meta WHERE key = 'job:refresh'").get() as { value: string }).value);
    assert.equal(run.result, "error"); assert.ok(run.finished_at);
  } finally { fresh.close(); }
});

test("scheduler runs one child at a time and coalesces queued refresh requests", async () => {
  const fresh = openDb(":memory:"), names: string[] = [];
  const jobs = createJobs(fresh, { spawnJob(name) { names.push(name); return spawn(process.execPath, ["-e", "setTimeout(()=>{}, 100)"], { stdio: "pipe" }); } });
  const service = createApp(fresh, { jobs });
  await new Promise<void>(resolve => service.server.listen(0, "127.0.0.1", resolve));
  try {
    const url = `http://127.0.0.1:${(service.server.address() as AddressInfo).port}/api/cards/refresh`;
    assert.equal((await fetch(url, { method: "POST" })).status, 200);
    jobs.enqueue("refresh"); jobs.enqueue("backup"); jobs.enqueue("backup");
    assert.deepEqual(names, ["refresh"]);
    assert.deepEqual(jobs.status().queued, ["backup"]);
    await new Promise(resolve => setTimeout(resolve, 350));
    assert.deepEqual(names, ["refresh", "backup"]);
  } finally { await service.close(); await jobs.close(); fresh.close(); }
});

test("scheduler retries failed refreshes within an hour and keeps successful runs daily", async t => {
  const now = Date.UTC(2026, 9, 25, 12);
  t.mock.method(Date, "now", () => now);
  const fresh = openDb(":memory:");
  const jobs = createJobs(fresh);
  const setMeta = fresh.prepare("INSERT INTO meta VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value");
  setMeta.run("price_stores_version", "1");
  try {
    for (const dataTime of [now - 48 * 60 * 60 * 1000, now]) {
      setMeta.run("last_ingest", new Date(dataTime).toISOString());
      setMeta.run("prices_as_of", new Date(dataTime).toISOString());
      const last = { last_run: new Date(now).toISOString(), finished_at: new Date(now).toISOString() };
      setMeta.run("job:refresh", JSON.stringify({ ...last, result: "error" }));
      const retryAt = Date.parse(jobs.status().refresh.next_run);
      assert.ok(retryAt > now && retryAt <= now + 60 * 60 * 1000);
      setMeta.run("job:refresh", JSON.stringify({ ...last, result: "success" }));
      assert.equal(Date.parse(jobs.status().refresh.next_run), now + 24 * 60 * 60 * 1000);
    }
  } finally { await jobs.close(); fresh.close(); }
});

test("review-through is deck-scoped and memory confirmation respects owner control", () => {
  const fresh = testDb(), a = newDeck(fresh), b = newDeck(fresh);
  try {
    const one = writeLog(fresh, OWNER, { deck_id: a, kind: "reject", summary: "One" });
    const two = writeLog(fresh, OWNER, { deck_id: b, kind: "reject", summary: "Two" });
    assert.equal(markReviewed(fresh, OWNER, { through: two }), 1);
    assert.equal((fresh.prepare("SELECT reviewed_at FROM log WHERE id = ?").get(one) as { reviewed_at: string | null }).reviewed_at, null);
    const owner = remember(fresh, OWNER, { text: "Owner preference", scope: "global" }).memory;
    assert.throws(() => reviseMemory(fresh, AGENT, owner.id, { action: "confirm" }), /owner/);
    const target = remember(fresh, AGENT, { text: "prefers patient games", scope: "deck", deck_id: a }).memory;
    for (let i = 0; i < 20; i++) remember(fresh, AGENT, { text: `prefers patient games in pod ${i}`, scope: "deck", deck_id: b });
    assert.throws(() => remember(fresh, AGENT, { text: target.text, scope: "deck", deck_id: a }), /already says exactly/);
    const similar = remember(fresh, AGENT, { text: "prefers patient games with friends", scope: "deck", deck_id: a }).similar;
    assert.ok(similar.some(m => m.id === target.id));
  } finally { fresh.close(); }
});

test("agent deck creation is attributed in the capability and unique errors remain 409", () => {
  const fresh = testDb();
  try {
    createDeck(fresh, AGENT, "Agent created");
    assert.equal((fresh.prepare("SELECT action FROM activity").get() as { action: string }).action, "create_deck");
    assert.throws(() => createDeck(fresh, AGENT, "Agent created"), (e: any) => e.status === 409);
    assert.equal((fresh.prepare("SELECT COUNT(*) n FROM activity").get() as { n: number }).n, 1);
  } finally { fresh.close(); }
});

test("shim without a saved token fails clearly without creating credentials", async () => {
  const missing = join(dir, "missing-token");
  const child = spawn(process.execPath, [join(import.meta.dirname, "../src/mcp/stdio.ts")], { env: { ...process.env, DECKBUILDER_DATA: missing } });
  let error = ""; child.stderr.on("data", chunk => error += chunk);
  const code = await new Promise(resolve => child.on("exit", resolve));
  assert.equal(code, 1); assert.match(error, /No agent token is configured/);
  assert.equal(existsSync(join(missing, "agent-token")), false);
});

test("accept-all validates original stale acknowledgements and rolls back conflicts atomically", async () => {
  const deck = newDeck(db);
  const first = createProposal(db, AGENT, deck, { items: [{ action: "add", oracle_id: FIXTURES[0].oracle_id, reason: "First" }] });
  const second = createProposal(db, AGENT, deck, { items: [{ action: "add", oracle_id: FIXTURES[1].oracle_id, reason: "Second" }] });
  const accepted = await post(`/api/decks/${deck}/proposals/rule`, { expected_revision: 0, proposals: [{ proposal_id: first }, { proposal_id: second }] });
  assert.equal(accepted.status, 200);
  assert.equal((await accepted.json()).state.deck.revision, 2);
  const third = createProposal(db, AGENT, deck, { items: [{ action: "cut", oracle_id: FIXTURES[0].oracle_id, reason: "Third" }] });
  add(db, deck, FIXTURES[2].oracle_id);
  const fresh = createProposal(db, AGENT, deck, { items: [{ action: "cut", oracle_id: FIXTURES[1].oracle_id, reason: "Fourth" }] });
  const proposals = [{ proposal_id: fresh }, { proposal_id: third }];
  const conflict = await post(`/api/decks/${deck}/proposals/rule`, { expected_revision: 3, proposals });
  assert.equal(conflict.status, 409); assert.equal(getProposal(db, OWNER, fresh)!.items[0].status, "pending");
  const acknowledged = await post(`/api/decks/${deck}/proposals/rule`, { expected_revision: 3, proposals: [{ proposal_id: fresh }, { proposal_id: third, acknowledge_stale: true }] });
  assert.equal(acknowledged.status, 200);
});

test("schema 10 gains image/retirement fields and baselines once without rebasing later stale work", () => {
  const file = join(dir, "phase0-migration.db"), old = openDb(file);
  ingestCards(old, FIXTURES);
  const deck = newDeck(old), proposal = createProposal(old, AGENT, deck, { items: [{ action: "add", oracle_id: FIXTURES[0].oracle_id, reason: "Migration" }] });
  old.exec("UPDATE proposals SET base_revision = NULL");
  for (const column of ["image_small", "image_normal", "image_art_crop", "retired"]) old.exec(`ALTER TABLE cards DROP COLUMN ${column}`);
  removeIdentity(old);
  old.exec("UPDATE meta SET value = '10' WHERE key = 'schema_version'");
  old.close();
  const upgraded = openDb(file);
  assert.equal(resolveCard(upgraded, FIXTURES[0].name).image_normal, null);
  assert.equal(resolveCard(upgraded, FIXTURES[0].name).retired, 0);
  assert.equal(getProposal(upgraded, OWNER, proposal)!.base_revision, 0);
  assert.equal(getProposal(upgraded, OWNER, proposal)!.stale, false);
  add(upgraded, deck, FIXTURES[1].oracle_id); upgraded.close();
  const reopened = openDb(file);
  assert.equal(getProposal(reopened, OWNER, proposal)!.base_revision, 0);
  assert.equal(getProposal(reopened, OWNER, proposal)!.stale, true); reopened.close();
});

test("optional S3 backup uploads the SQLite snapshot through the SDK and keeps local data on failure", async () => {
  const { createServer } = await import("node:http");
  let path = "", body = Buffer.alloc(0);
  const bucket = createServer((req, res) => {
    if (req.method === "PUT") path = req.url!;
    const chunks: Buffer[] = [];
    req.on("data", chunk => chunks.push(chunk));
    req.on("end", () => { if (req.method === "PUT") body = Buffer.concat(chunks); res.writeHead(200, { ETag: '"test"' }); res.end(req.method === "GET" ? '<ListBucketResult><IsTruncated>false</IsTruncated></ListBucketResult>' : ""); });
  });
  await new Promise<void>(resolve => bucket.listen(0, "127.0.0.1", resolve));
  const keys = ["BACKUP_S3_ENDPOINT", "BACKUP_S3_BUCKET", "BACKUP_S3_ACCESS_KEY", "BACKUP_S3_SECRET_KEY"];
  const saved = keys.map(key => process.env[key]);
  const fresh = openDb(":memory:"), data = join(dir, "s3-backup");
  try {
    Object.assign(process.env, { BACKUP_S3_ENDPOINT: `http://127.0.0.1:${(bucket.address() as AddressInfo).port}`, BACKUP_S3_BUCKET: "test-bucket", BACKUP_S3_ACCESS_KEY: "test-access-key", BACKUP_S3_SECRET_KEY: "test-secret-key" });
    const result = await backupDatabase(fresh, data, new Date("2026-10-25T00:00:00Z"));
    assert.match(path, /^\/test-bucket\/backups\/deck-builder-2026-10-25T00-00-00-000Z-/);
    assert.ok(body.includes(Buffer.from("SQLite format 3")));
    assert.ok(readFileSync(result.file).includes(Buffer.from("SQLite format 3")));
    delete process.env.BACKUP_S3_BUCKET;
    await assert.rejects(backupDatabase(fresh, data), /S3 backup needs all/);
    assert.ok(readdirSync(join(data, "backups")).some(file => file.endsWith(".db")));
  } finally {
    keys.forEach((key, i) => { if (saved[i] === undefined) delete process.env[key]; else process.env[key] = saved[i]; });
    fresh.close();
    await new Promise<void>(resolve => bucket.close(() => resolve()));
  }
});

for (const marker of [null, "0", "1"]) test(`scheduler checks price-store marker ${marker ?? "missing"} on migrated DB at boot and in next-run status`, async t => {
  const { removePriceSources } = await import("./legacy-db.ts");
  const { ingestPrices, cheapestPrintings } = await import("../src/prices.ts");
  const now = Date.UTC(2026, 9, 25, 12), lastRun = new Date(now - 60 * 60 * 1000).toISOString();
  t.mock.method(Date, "now", () => now);
  const path = join(dir, `price-store-boot-${marker ?? "missing"}.db`);
  const before = openDb(path);
  ingestCards(before, FIXTURES);
  await ingestPrices(before, await cheapestPrintings([{ oracle_id: "id-solring", set: "test", collector_number: "1", games: ["paper"], prices: { usd: "1.19" } }]), lastRun);
  removePriceSources(before);
  before.exec("UPDATE meta SET value='12' WHERE key='schema_version'");
  before.close();
  const fresh = openDb(path), names: string[] = [];
  const setMeta = fresh.prepare("INSERT INTO meta VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value");
  for (const key of ["last_ingest", "prices_as_of"]) setMeta.run(key, lastRun);
  setMeta.run("job:refresh", JSON.stringify({ last_run: lastRun, finished_at: lastRun, result: "success" }));
  if (marker != null) setMeta.run("price_stores_version", marker);
  assert.equal(fresh.prepare("SELECT count(*) n FROM card_prices WHERE store='tcgplayer'").get()!.n, 1);
  assert.equal(fresh.prepare("SELECT count(*) n FROM card_prices WHERE store='cardmarket'").get()!.n, 0);
  const jobs = createJobs(fresh, { spawnJob(name) { names.push(name); return spawn(process.execPath, ["-e", ""], { stdio: "pipe" }); } });
  try {
    assert.equal(jobs.status().refresh.next_run, new Date(marker === "1" ? Date.parse(lastRun) + 24 * 60 * 60 * 1000 : now).toISOString());
    jobs.start();
    assert.deepEqual(names, marker === "1" ? [] : ["refresh"]);
    assert.equal(jobs.status().active, marker === "1" ? null : "refresh");
  } finally { await jobs.close(); fresh.close(); }
});

test("CLI search before a price refresh names its source and explains the missing date", async () => {
  const path = join(dir, "cli-price-date.db"), data = join(dir, "cli-price-data");
  const fresh = openDb(path); ingestCards(fresh, FIXTURES); fresh.close();
  const child = spawn(process.execPath, [join(PKG_ROOT, "src", "cli.ts"), "search", "t:artifact"], {
    env: { ...process.env, DECKBUILDER_MODE: "local", DECKBUILDER_DB: path, DECKBUILDER_DATA: data }, stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = "", stderr = "";
  child.stdout.on("data", data => { stdout += data; }); child.stderr.on("data", data => { stderr += data; });
  assert.equal(await new Promise(resolve => child.once("close", resolve)), 0, stderr);
  assert.equal(stdout.split("\n")[0], "Prices: TCGplayer (USD), Scryfall daily estimates as of unknown (not loaded).");
  assert.doesNotMatch(stdout, /as of null/);
});
