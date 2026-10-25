import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { retainPolicySources, verifyRetainedPolicySources, type NativeSourceFile } from "./retain-policy-sources.ts";
const sha = (bytes: string | Buffer) => createHash("sha256").update(bytes).digest("hex");
const fixturePath = "packages/game-engine/tests/fixtures/commander-banned-2026-10-15.json";
function fixture(value: unknown): NativeSourceFile {
  const contents = JSON.stringify(value);
  return { path: fixturePath, contents, sha256: sha(contents) };
}
function setup() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "mtg-policy-retain-"));
  const store = path.join(root, "store");
  const ban = Buffer.from("<html>pinned ban policy\u0000\u00e9</html>");
  const offensive = Buffer.from("<html>pinned offensive policy</html>");
  const files = [fixture({ url: "https://example.test/bans", html_sha256: sha(ban), offensive_policy: {
    url: "https://example.test/offensive", html_sha256: sha(offensive),
  } })];
  const env = { MTG_ENGINE_COMMANDER_BAN_POLICY_PATH: path.join(root, "ban.html"),
    MTG_ENGINE_OFFENSIVE_POLICY_PATH: path.join(root, "offensive.html") };
  fs.writeFileSync(env.MTG_ENGINE_COMMANDER_BAN_POLICY_PATH, ban);
  fs.writeFileSync(env.MTG_ENGINE_OFFENSIVE_POLICY_PATH, offensive);
  return { root, store, ban, offensive, files, env };
}
function scenario(run: (state: ReturnType<typeof setup>) => void) {
  const state = setup();
  try { run(state); } finally { fs.rmSync(state.root, { recursive: true, force: true }); }
}
test("historical source closures without a Commander policy remain a no-op", () => scenario(s => {
  assert.deepEqual(retainPolicySources([], s.root, s.store, {}), []);
  verifyRetainedPolicySources([], [], s.store);
  assert.equal(fs.existsSync(s.store), false);
}));
test("both pinned HTML byte streams are retained outside native sources and verified independently", () => scenario(s => {
  const retained = retainPolicySources(s.files, s.root, s.store, s.env);
  assert.deepEqual(retained.map(r => r.kind), ["commander_ban", "offensive_cards"]);
  assert.deepEqual(retained.map(r => r.url), ["https://example.test/bans", "https://example.test/offensive"]);
  for (const [index, bytes] of [s.ban, s.offensive].entries()) {
    assert.equal(path.dirname(retained[index].retained_path), path.join(s.store, "policy-sources"));
    assert.deepEqual(fs.readFileSync(retained[index].retained_path), bytes);
    assert.equal(retained[index].html_sha256, sha(bytes));
  }
  fs.rmSync(s.env.MTG_ENGINE_COMMANDER_BAN_POLICY_PATH);
  fs.rmSync(s.env.MTG_ENGINE_OFFENSIVE_POLICY_PATH);
  verifyRetainedPolicySources(s.files, retained, s.store);
}));
test("default policy source filenames are exact and environment overrides are optional", () => scenario(s => {
  const folder = path.join(s.root, "generated/policy-sources"); fs.mkdirSync(folder, { recursive: true });
  fs.writeFileSync(path.join(folder, "banned-restricted-20261015.html"), s.ban);
  fs.writeFileSync(path.join(folder, "offensive-policy-20200610.html"), s.offensive);
  const retained = retainPolicySources(s.files, s.root, s.store, {});
  verifyRetainedPolicySources(s.files, retained, s.store);
}));
test("missing or corrupted original policy bytes fail before any retained files are written", () => scenario(s => {
  for (const key of ["MTG_ENGINE_COMMANDER_BAN_POLICY_PATH", "MTG_ENGINE_OFFENSIVE_POLICY_PATH"] as const) {
    const saved = fs.readFileSync(s.env[key]);
    fs.rmSync(s.env[key]);
    assert.throws(() => retainPolicySources(s.files, s.root, s.store, s.env), /Missing pinned/u);
    assert.equal(fs.existsSync(s.store), false);
    fs.writeFileSync(s.env[key], Buffer.concat([saved, Buffer.from("changed")]));
    assert.throws(() => retainPolicySources(s.files, s.root, s.store, s.env), /digest mismatch/u);
    assert.equal(fs.existsSync(s.store), false);
    fs.writeFileSync(s.env[key], saved);
  }
}));
test("offline post-verification detects changed bytes, missing files and forged retained metadata", () => scenario(s => {
  const retained = retainPolicySources(s.files, s.root, s.store, s.env);
  for (const row of retained) {
    const saved = fs.readFileSync(row.retained_path);
    fs.writeFileSync(row.retained_path, "corrupt");
    assert.throws(() => verifyRetainedPolicySources(s.files, retained, s.store), /digest mismatch/u);
    fs.rmSync(row.retained_path);
    assert.throws(() => verifyRetainedPolicySources(s.files, retained, s.store), /Missing retained/u);
    fs.writeFileSync(row.retained_path, saved);
  }
  for (const field of ["url", "html_sha256", "retained_path"] as const) {
    const changed = structuredClone(retained); changed[0][field] += "changed";
    assert.throws(() => verifyRetainedPolicySources(s.files, changed, s.store), /metadata differs/u);
  }
  assert.throws(() => verifyRetainedPolicySources(s.files, retained.slice(1), s.store), /set differs/u);
}));
test("policy fixture integrity and both required official descriptions are mandatory", () => scenario(s => {
  const badHash = { ...s.files[0], sha256: "0".repeat(64) };
  assert.throws(() => retainPolicySources([badHash], s.root, s.store, s.env), /fixture path\/hash/u);
  assert.throws(() => retainPolicySources([...s.files, ...s.files], s.root, s.store, s.env), /fixture path\/hash/u);
  for (const malformed of [null, {}, { url: "https://example.test", html_sha256: sha(s.ban) },
    { url: "https://example.test", html_sha256: "bad", offensive_policy: {} }]) {
    assert.throws(() => retainPolicySources([fixture(malformed)], s.root, s.store, s.env));
  }
  assert.equal(fs.existsSync(s.store), false);
}));
