import { executeCommand } from "../src/commands.ts";
import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { beginSignup, provisionIdentity } from "../src/auth.ts";
import { accountInfo, accountScopes, exportAccount, deleteAccount, createInvite, chooseHandle, adminInfo } from "../src/accounts.ts";
import { createDeck } from "../src/deck/service.ts";
import { remember } from "../src/memory.ts";
import { createToken } from "../src/tokens.ts";
import { createProposal } from "../src/deck/proposals.ts";
import { OWNER, agent } from "./actors.ts";
import { testDb } from "./helpers.ts";
import { fixture, env, enroll, authRow } from "./hosted-fixture.ts";

describe("safe owner claims and single-use invites",()=>{
  test("correct claim links user 1, preserves their decks and cannot be reused",()=>{
    const db=testDb();try{
      const deck=createDeck(db,OWNER,"Existing owner deck");
      assert.deepEqual(enroll(db,"owner-auth",env.DECKBUILDER_CLAIM_CODE,"Owner Identity"),{user_id:1,claimed:true});
      assert.equal(db.prepare("SELECT user_id FROM decks WHERE id=?").get(deck)!.user_id,1);
      assert.throws(()=>beginSignup(db,env.DECKBUILDER_CLAIM_CODE,env),{status:403});
      assert.equal(db.prepare("SELECT auth_id FROM users WHERE id=1").get()!.auth_id,"owner-auth");
    }finally{db.close();}
  });
  test("wrong and unset codes cannot claim; an invite creates a new principal",()=>{
    const db=testDb();try{
      assert.throws(()=>beginSignup(db,"wrong",env),{status:403});
      assert.throws(()=>beginSignup(db,env.DECKBUILDER_CLAIM_CODE,{...env,DECKBUILDER_CLAIM_CODE:undefined}),{status:403});
      const invite=createInvite(db,OWNER).code;const noClaim={...env,DECKBUILDER_CLAIM_CODE:undefined};
      const result=provisionIdentity(db,{id:"fresh",name:"fresh"},beginSignup(db,invite,noClaim),noClaim);
      assert.notEqual(result.user_id,1);assert.equal(result.claimed,false);
      assert.equal(db.prepare("SELECT auth_id FROM users WHERE id=1").get()!.auth_id,null);
      assert.throws(()=>enroll(db,"another",invite),{status:403});
      assert.equal(db.prepare("SELECT count(*) n FROM users").get()!.n,2);
    }finally{db.close();}
  });
  test("two pending claim tickets recheck unclaimed status; replay/expired tickets fail",()=>{
    const db=testDb();try{
      const a=beginSignup(db,env.DECKBUILDER_CLAIM_CODE,env),b=beginSignup(db,env.DECKBUILDER_CLAIM_CODE,env);
      provisionIdentity(db,{id:"one",name:"One"},a,env);
      for(const ticket of [a,b]) assert.throws(()=>provisionIdentity(db,{id:"two",name:"Two"},ticket,env),{status:403});
      const code=createInvite(db,OWNER).code,t=beginSignup(db,code,env,1);
      assert.throws(()=>provisionIdentity(db,{id:"late",name:"Late"},t,env,600_001),{status:403});
    }finally{db.close();}
  });
});

test("hosted HTTP session cookies scope UI data; metadata, headers, invite gate and CSRF",async()=>{
  const f=await fixture();try{
    createDeck(f.db,OWNER,"Owner secret");
    const id=enroll(f.db,"second",createInvite(f.db,OWNER).code).user_id;
    const cookie=await f.session("second");createDeck(f.db,{user_id:id,via:"ui",label:"ui"},"Second deck");
    assert.equal((await f.call("/api/decks")).status,401);
    const response=await f.call("/api/decks",{headers:{Cookie:cookie}});
    assert.equal(response.status,200);assert.deepEqual((await response.json() as any[]).map(d=>d.name),["Second deck"]);
    assert.match(response.headers.get("content-security-policy")!,/img-src \x27self\x27 data: https:\/\/cards\.scryfall\.io https:\/\/svgs\.scryfall\.io;/);
    assert.equal(response.headers.get("x-content-type-options"),"nosniff");
    assert.equal((await f.call("/api/decks",{method:"POST",headers:{Cookie:cookie,Origin:"https://evil.invalid","Content-Type":"application/json"},body:'{"name":"bad"}'})).status,403);
    assert.equal((await f.call("/api/signup",{method:"POST",headers:{"Content-Type":"application/json"},body:'{"code":"wrong"}'})).status,403);
    const challenge=await f.call("/mcp",{method:"POST"});assert.equal(challenge.status,401);assert.match(challenge.headers.get("www-authenticate")!,/oauth-protected-resource\/mcp/);
    const prm=await f.call("/.well-known/oauth-protected-resource/mcp");assert.equal(prm.status,200);
    const metadata=await prm.json() as any;assert.equal(metadata.resource,f.base+"/mcp");assert.ok(metadata.scopes_supported.includes("deck-builder"));
    const as=await (await f.call("/.well-known/oauth-authorization-server")).json() as any;
    assert.equal(as.issuer,f.base+"/api/auth");assert.ok(as.code_challenge_methods_supported.includes("S256"));assert.equal(as.client_id_metadata_document_supported,true);assert.match(as.registration_endpoint,/oauth2\/register$/);
    assert.equal((await f.call("/api/admin",{headers:{Cookie:cookie}})).status,403);
  }finally{await f.close();}
});

test("export/delete inventory walks sqlite_master; every tenant row, auth grant, FTS and credential is removed without touching the other user",()=>{
  const db=testDb();try{
    const id=enroll(db,"delete-user",createInvite(db,OWNER).code).user_id,ui={user_id:id,via:"ui" as const,label:"ui"};authRow(db,"delete-user");
    const deck=createDeck(db,ui,"Delete this"),token=createToken(db,ui,"Delete token");
    remember(db,ui,{scope:"global",text:"Delete preference"});createProposal(db,{...agent("CLI",id),token_id:token.credential.id},deck,{items:[{action:"add",oracle_id:"id-forest",reason:"Preference"}]});
    db.prepare('INSERT INTO session(id,token,userId,expiresAt,createdAt,updatedAt) VALUES(?,?,?,?,?,?)').run("session","session-secret","delete-user",Date.now()+60_000,Date.now(),Date.now());
    db.prepare('INSERT INTO account(id,accountId,providerId,userId,accessToken,createdAt,updatedAt) VALUES(?,?,?,?,?,?,?)').run("account","provider-id","discord","delete-user","provider-secret",Date.now(),Date.now());
    db.prepare('INSERT INTO oauthClient(id,clientId,userId,redirectUris) VALUES(?,?,?,?)').run("owned","owned","delete-user","[]");
    db.prepare('INSERT INTO oauthConsent(id,clientId,userId,scopes,createdAt,updatedAt) VALUES(?,?,?,?,?,?)').run("c","owned","delete-user","[]",Date.now(),Date.now());
    db.prepare('INSERT INTO verification(id,identifier,value,expiresAt,createdAt,updatedAt) VALUES(?,?,?,?,?,?)').run("v","code-hash",JSON.stringify({userId:"delete-user",secret:"hidden"}),Date.now()+60_000,Date.now(),Date.now());
    executeCommand(db, ui, { operation: "remember", input: {}, idempotency_key: "account-receipt" }, () => ({ ok: true }));
    const owner=createDeck(db,OWNER,"Preserve this"),before=db.prepare("SELECT * FROM decks WHERE id=?").get(owner);
    const data=exportAccount(db,ui);assert.match(data.decks[0].name,/Delete/);assert.equal(data.tables.account.length,1);assert.equal(data.tables.verification.length,1);assert.equal(data.tables.command_receipts.length,1);
    assert.doesNotMatch(JSON.stringify(data),/session-secret|provider-secret|hidden/);
    assert.ok(accountScopes(db).length>20);
    deleteAccount(db,ui,accountInfo(db,ui).handle);
    for(const scope of accountScopes(db)) {
      const q=db.prepare(`SELECT count(*) n FROM "${scope.table}" WHERE ${scope.where}`);q.setAllowUnknownNamedParameters(true);
      assert.equal(q.get({uid:id,aid:"delete-user"})!.n,0,scope.table);
    }
    assert.equal(db.prepare("SELECT count(*) n FROM memories_fts WHERE text='Delete preference'").get()!.n,0);
    assert.equal(db.prepare("SELECT count(*) n FROM verification").get()!.n,0);
    assert.deepEqual(db.prepare("SELECT * FROM decks WHERE id=?").get(owner),before);
    assert.deepEqual(db.prepare("PRAGMA foreign_key_check").all(),[]);
    db.exec("CREATE TABLE future_tenant(id INTEGER PRIMARY KEY,user_id INTEGER REFERENCES users(id),payload TEXT)");
    assert.ok(accountScopes(db).some(s=>s.table==="future_tenant"));
    db.exec("CREATE TABLE unclassified_future(id INTEGER PRIMARY KEY,payload TEXT)");
    assert.throws(()=>accountScopes(db),/unclassified_future/);
  }finally{db.close();}
});

test("last admin cannot delete, non-admin cannot mint invites; agent cannot export/delete or choose a handle",()=>{
  const db=testDb();try{
    assert.equal(accountInfo(db,OWNER).can_delete,false);assert.throws(()=>deleteAccount(db,OWNER,"owner"),{status:403});
    const id=enroll(db,"ordinary",createInvite(db,OWNER).code).user_id,ui={user_id:id,via:"ui" as const,label:"ui"};
    assert.throws(()=>createInvite(db,ui),{status:403});assert.throws(()=>adminInfo(db,ui),{status:403});
    for(const fn of [()=>exportAccount(db,agent("bad",id)),()=>deleteAccount(db,agent("bad",id),"ordinary"),()=>chooseHandle(db,agent("bad",id),"hijack")])assert.throws(fn,{status:403});
  }finally{db.close();}
});

test("CIMD transport rejects private targets, credentials, non-HTTPS and unsafe methods",async()=>{
  const {fetchClientMetadata}=await import("../src/cimd-transport.ts");
  for(const url of ["http://example.com/client.json","https://user:password@example.com/client.json","https://127.0.0.1/client.json","https://[::1]/client.json","https://localhost/client.json"])
    await assert.rejects(fetchClientMetadata(url),TypeError);
  await assert.rejects(fetchClientMetadata("https://example.com/client.json",{method:"POST"}),TypeError);
});

test("an invited visitor arriving first cannot prevent the later code-bound owner claim",()=>{
  const db=testDb();try{
    const visitor=enroll(db,"first-visitor",createInvite(db,OWNER).code,"Visitor");assert.notEqual(visitor.user_id,1);
    const owner=enroll(db,"real-owner",env.DECKBUILDER_CLAIM_CODE,"Real Owner");assert.deepEqual(owner,{user_id:1,claimed:true});
    assert.equal(db.prepare("SELECT auth_id FROM users WHERE id=1").get()!.auth_id,"real-owner");
  }finally{db.close();}
});
