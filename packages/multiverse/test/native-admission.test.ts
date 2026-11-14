import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, chmod, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createNativeAdmission, nativeAdmissionFromEnvironment } from "../src/native-admission.ts";
import type { GameDeck, GameIdentity } from "@mtg-ecosystem/deck-builder/contracts";

const identity: GameIdentity = { catalog_sha256: "b".repeat(64), profile: "fixture", engine_build_id: "engine", server_build_id: "server" };
const deck: GameDeck = { deck_id: 1, revision: 2, sha256: "c".repeat(64), cards: [{ oracle_id: "fixture-commander", role: "commander", quantity: 1 }, { oracle_id: "fixture-card", role: "card", quantity: 99 }] };
async function fixture(source: string) {
  const path = await mkdtemp(join(tmpdir(), "native-admission-"));
  const executable = join(path, "native-fixture");
  await writeFile(executable, `#!${process.execPath}\n${source}`); await chmod(executable, 0o700);
  return { adapter: createNativeAdmission(executable, identity), close: () => rm(path, { recursive: true, force: true }) };
}
const read = `let data='';process.stdin.on('data',chunk=>data+=chunk);process.stdin.on('end',()=>{const r=JSON.parse(data);`;
const accepted = `console.log(JSON.stringify({version:1,status:'admitted',profile:r.profile,engine_build_id:r.engine_build_id,server_build_id:r.server_build_id,catalog_sha256:r.catalog_sha256,deck:r.deck}));});`;

test("real process transport sends exact Commander-included list and does not inherit application secrets", async () => {
  const f = await fixture(read + `if(process.argv[2]!=='admit-client'||r.operation!=='admit'||r.deck.cards.reduce((n,c)=>n+c.count,0)!==100||Object.keys(process.env).some(k=>k!=="LD_LIBRARY_PATH"))process.exit(2);` + accepted);
  try {
    const result = await f.adapter.admit(deck, new AbortController().signal);
    assert.equal(result.state, "supported");
    if (result.state === "supported") { assert.equal(result.deck_sha256, deck.sha256); assert.equal(result.launch_ready, false); }
    assert.equal(f.adapter.lifecycle_qualified, false);
    await assert.rejects(f.adapter.ensure({} as never, new AbortController().signal), /not qualified/);
  } finally { await f.close(); }
});

test("multiple commanders and companion roles receive explicit outcomes without starting a native process", async () => {
  const adapter = createNativeAdmission("/does-not-exist", identity), signal = new AbortController().signal;
  assert.deepEqual(await adapter.admit({ ...deck, cards: [...deck.cards, { oracle_id: "other", role: "commander", quantity: 1 }] }, signal), { state: "unsupported", code: "multiple_commanders" });
  assert.deepEqual(await adapter.admit({ ...deck, cards: [...deck.cards, { oracle_id: "other", role: "companion", quantity: 1 }] }, signal), { state: "unsupported", code: "companion" });
});

test("native rejection, identity drift, deck substitution, oversized output and process errors fail closed without stderr leakage", async () => {
  for (const [source, state] of [
    [`console.log(JSON.stringify({version:1,status:'rejected',reason:'deck',detail:'private-native-detail'}))`, "unsupported"],
    [read + `r.catalog_sha256='wrong';` + accepted, "unavailable"],
    [read + `r.deck.cards[1].count=98;` + accepted, "unavailable"],
    [`process.stdout.write('x'.repeat(70000))`, "unavailable"],
    [`console.error('operator-secret');process.exit(1)`, "unavailable"],
    [`console.log('{bad-json')`, "unavailable"],
  ] as const) {
    const f = await fixture(source);
    try { const result = await f.adapter.admit(deck, new AbortController().signal); assert.equal(result.state, state); assert.equal(JSON.stringify(result).includes("secret"), false); assert.equal(JSON.stringify(result).includes("private-native-detail"), false); }
    finally { await f.close(); }
  }
});

test("abort terminates admission process and partial env configuration cannot enable launch", async () => {
  const f = await fixture(`setInterval(()=>{},1000)`), controller = new AbortController();
  try {
    const pending = f.adapter.admit(deck, controller.signal); controller.abort();
    assert.equal((await pending).state, "unavailable");
    assert.equal(nativeAdmissionFromEnvironment({}), undefined);
    assert.throws(() => nativeAdmissionFromEnvironment({ MULTIVERSE_NATIVE_EXECUTABLE: "/bin/native" }), /incomplete/);
    assert.throws(() => createNativeAdmission("relative", identity), /absolute/);
  } finally { await f.close(); }
});
