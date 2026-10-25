import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { createToken, listTokens, migrateLegacyToken, revokeToken, verifyToken } from "../src/tokens.ts";
import { createProposal, getProposal, withdrawProposal } from "../src/deck/proposals.ts";
import { createDeck } from "../src/deck/service.ts";
import { PermissionError } from "../src/actor.ts";
import { testDb, AGENT } from "./helpers.ts";
import { OWNER } from "./actors.ts";
import { getMode } from "../src/mode.ts";

test("tokens are per-user, random, hashed, returned once, throttled and immediately revoked", () => {
  const db = testDb();
  try {
    db.exec("INSERT INTO users(id,handle,display_name) VALUES (2,'second','Second')");
    const second = { ...OWNER, user_id: 2 };
    const a = createToken(db, OWNER, "CLI"), b = createToken(db, second, "CLI");
    assert.match(a.token, /^dbk_[A-Za-z0-9_-]{43}$/);
    assert.notEqual(a.token, b.token);
    assert.deepEqual(listTokens(db, OWNER), [a.credential]);
    const stored = db.prepare("SELECT token_hash FROM agent_tokens WHERE id = ?").get(a.credential.id) as { token_hash: string };
    assert.match(stored.token_hash, /^[a-f0-9]{64}$/); assert.notEqual(stored.token_hash, a.token);
    assert.ok(!JSON.stringify(listTokens(db, OWNER)).includes(a.token));
    const now = Date.parse("2026-10-25T12:00:00Z");
    assert.deepEqual(verifyToken(db, a.token, now), { user_id: 1, via: "agent", label: "CLI", token_id: a.credential.id });
    assert.equal(verifyToken(db, b.token, now)?.user_id, 2);
    verifyToken(db, a.token, now + 59_999);
    assert.equal(listTokens(db, OWNER)[0].last_used_at, new Date(now).toISOString());
    verifyToken(db, a.token, now + 60_000);
    assert.equal(listTokens(db, OWNER)[0].last_used_at, new Date(now + 60_000).toISOString());
    assert.equal(verifyToken(db, a.token.slice(0, -1) + "!"), null);
    assert.throws(() => revokeToken(db, second, a.credential.id), (e: any) => e.status === 404);
    assert.throws(() => createToken(db, AGENT, "escalate"), PermissionError);
    assert.throws(() => listTokens(db, AGENT), PermissionError);
    revokeToken(db, OWNER, a.credential.id);
    assert.equal(verifyToken(db, a.token), null);
    assert.equal(verifyToken(db, b.token)?.user_id, 2);
    db.exec("UPDATE users SET deleted_at = datetime('now') WHERE id = 2");
    assert.equal(verifyToken(db, b.token), null);
  } finally { db.close(); }
});

test("legacy import preserves the secret's access, commits before rename and retries by hash", () => {
  const dir = mkdtempSync(join(tmpdir(), "deck-token-")), file = join(dir, "agent-token"), db = testDb();
  try {
    migrateLegacyToken(db, file); assert.equal(existsSync(file), false);
    const deck = createDeck(db, OWNER, "Legacy pending work");
    const pending = createProposal(db, AGENT, deck, {items:[{action:"add",oracle_id:"id-solring",reason:"Existing work"}]});
    const secret = "old-local-client-token";
    writeFileSync(file, secret, { mode: 0o600 });
    migrateLegacyToken(db, file);
    assert.equal(existsSync(file), false);
    assert.equal(readFileSync(join(dir,"stdio-token"),"utf8"),secret);
    assert.equal(statSync(join(dir,"stdio-token")).mode & 0o777,0o600);
    const actor = verifyToken(db, secret)!;
    assert.equal(actor.user_id, 1);
    assert.equal(getProposal(db, OWNER, pending)!.author_token_id, actor.token_id);
    assert.doesNotThrow(() => withdrawProposal(db, {...actor,label:"another hint"},pending,"Continue old work"));
    writeFileSync(file, secret); migrateLegacyToken(db, file);
    assert.equal(listTokens(db, OWNER).length, 1);
    writeFileSync(file, "must-survive-failure");
    db.exec("CREATE TRIGGER fail_import BEFORE INSERT ON agent_tokens BEGIN SELECT RAISE(ABORT, 'disk failure'); END");
    assert.throws(() => migrateLegacyToken(db, file), /disk failure/);
    assert.equal(readFileSync(file, "utf8"), "must-survive-failure");
  } finally { db.close(); rmSync(dir, { recursive: true, force: true }); }
});

test("modes are explicit and invalid hosted configuration fails closed", () => {
  assert.equal(getMode({}), "local");
  assert.equal(getMode({ DECKBUILDER_MODE: "local" }), "local");
  assert.throws(() => getMode({ DECKBUILDER_MODE: "typo" }), /must be local or hosted/);
  assert.throws(() => getMode({ DECKBUILDER_MODE: "hosted" }), /PUBLIC_ORIGIN/);
  assert.equal(getMode({ DECKBUILDER_MODE: "hosted", DECKBUILDER_PUBLIC_ORIGIN: "https://decks.example" }), "hosted");
});

test("migration preserves a no-token-argument stdio client and restores its saved credential", async () => {
  const { Client } = await import("@modelcontextprotocol/client");
  const { StdioClientTransport } = await import("@modelcontextprotocol/client/stdio");
  const { createApp } = await import("../src/app.ts");
  const { PKG_ROOT } = await import("../src/db.ts");
  const dir = mkdtempSync(join(tmpdir(),"default-stdio-")), file = join(dir,"agent-token"), db = testDb();
  writeFileSync(file,"old-stdio-client-credential",{mode:0o644});
  migrateLegacyToken(db,file);
  const app = createApp(db), client = new Client({name:"old-stdio-client",version:"1.0.0"});
  await new Promise<void>(resolve=>app.server.listen(0,"127.0.0.1",resolve));
  try {
    await client.connect(new StdioClientTransport({command:process.execPath,
      args:[join(PKG_ROOT,"src/mcp/stdio.ts"),"--name","chatgpt-desktop"],cwd:tmpdir(),
      env:{...process.env as Record<string,string>,DECKBUILDER_MODE:"local",DECKBUILDER_DATA:dir,DECKBUILDER_TOKEN_FILE:"",DECKBUILDER_PORT:String((app.server.address() as import("node:net").AddressInfo).port)}}));
    const result = await client.callTool({name:"start_session",arguments:{}});
    assert.ok(!result.isError);assert.match(JSON.stringify(result.content),/owner — Owner/);
    const restored = testDb();
    try {
      migrateLegacyToken(restored,file);
      assert.equal(verifyToken(restored,readFileSync(join(dir,"stdio-token"),"utf8"))!.user_id,1);
      const row=listTokens(restored,OWNER)[0];revokeToken(restored,OWNER,row.id);
      migrateLegacyToken(restored,file);
      assert.equal(verifyToken(restored,"old-stdio-client-credential"),null,"boot does not undo a revocation");
    } finally {restored.close();}
  } finally {await client.close();await app.close();db.close();rmSync(dir,{recursive:true,force:true});}
});

test("stdio rejects secret argv and accepts an explicit token-file environment path", async () => {
  const { spawnSync } = await import("node:child_process");
  const { PKG_ROOT } = await import("../src/db.ts");
  const dir=mkdtempSync(join(tmpdir(),"stdio-args-")),file=join(dir,"saved");
  try {
    writeFileSync(file,"test-file-credential",{mode:0o600});
    const rejected=spawnSync(process.execPath,[join(PKG_ROOT,"src/mcp/stdio.ts"),"--token","must-not-be-supported"],{encoding:"utf8"});
    assert.equal(rejected.status,1);assert.match(rejected.stderr,/--token is not supported/);
    const accepted=spawnSync(process.execPath,[join(PKG_ROOT,"src/mcp/stdio.ts")],{input:"",encoding:"utf8",env:{...process.env,DECKBUILDER_TOKEN_FILE:file}});
    assert.equal(accepted.status,0);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
