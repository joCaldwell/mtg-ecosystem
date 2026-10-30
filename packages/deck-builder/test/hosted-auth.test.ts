import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { createHmac, randomBytes, createHash } from "node:crypto";
import { createServer } from "node:net";
import type { AddressInfo } from "node:net";
import { generateKeyPair, exportJWK, SignJWT } from "jose";
import { createHostedAuth, beginSignup, provisionIdentity } from "../src/auth.ts";
import { createApp } from "../src/app.ts";
import { accountInfo, accountScopes, exportAccount, deleteAccount, createInvite, chooseHandle, adminInfo } from "../src/accounts.ts";
import { createRateLimiter, LIMITS, RateLimitError } from "../src/ratelimit.ts";
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
  test("handles sanitise, pad, truncate, deduplicate and require a collision picker",()=>{
    const db=testDb();try{
      for(const [id,name] of [["a","!!!"],["b","A NAME"],["c","A NAME"],["d","Z".repeat(60)]]){
        const r=enroll(db,id,createInvite(db,OWNER).code,name);
        const user=accountInfo(db,{user_id:r.user_id,via:"ui",label:"ui"});assert.match(user.handle,/^[a-z0-9_-]{3,24}$/);
        if(id==="c"){assert.equal(user.needs_handle,1);assert.equal(chooseHandle(db,{user_id:r.user_id,via:"ui",label:"ui"},"picked").needs_handle,0);}
      }
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

test("personal and locally minted OAuth JWTs reach the right user's MCP tool; wrong audience, expiry and withdrawn consent fail",async()=>{
  const f=await fixture();try{
    const id=enroll(f.db,"jwt-user",createInvite(f.db,OWNER).code).user_id;authRow(f.db,"jwt-user");
    const ui={user_id:id,via:"ui" as const,label:"ui"};createDeck(f.db,ui,"JWT private deck");createDeck(f.db,OWNER,"Owner invisible deck");
    const keys=await generateKeyPair("EdDSA",{extractable:true}),pub=await exportJWK(keys.publicKey),priv=await exportJWK(keys.privateKey);
    pub.kid="test-key";pub.alg="EdDSA";
    f.db.prepare('INSERT INTO jwks(id,publicKey,privateKey,createdAt,alg,crv) VALUES(?,?,?,?,?,?)').run("test-key",JSON.stringify(pub),JSON.stringify(priv),Date.now(),"EdDSA","Ed25519");
    f.db.prepare('INSERT INTO oauthClient(id,clientId,name,redirectUris,scopes) VALUES(?,?,?,?,?)').run("client-row","test-client","Test OAuth client",'[]','["deck-builder"]');
    f.db.prepare('INSERT INTO oauthConsent(id,clientId,userId,scopes,createdAt,updatedAt) VALUES(?,?,?,?,?,?)').run("consent","test-client","jwt-user",'["deck-builder"]',Date.now(),Date.now());
    const mint=(aud:string,exp="2m",claims:Record<string,unknown>={})=>new SignJWT({client_id:"test-client",scope:"deck-builder",...claims}).setProtectedHeader({alg:"EdDSA",kid:"test-key",typ:"at+jwt"}).setSubject("jwt-user").setIssuer(f.base+"/api/auth").setAudience(aud).setIssuedAt().setExpirationTime(exp).sign(keys.privateKey);
    const run=async(token:string)=>f.call("/mcp",{method:"POST",headers:{Authorization:`Bearer ${token}`,"Content-Type":"application/json",Accept:"application/json, text/event-stream","MCP-Protocol-Version":"2025-03-26"},body:JSON.stringify({jsonrpc:"2.0",id:1,method:"tools/call",params:{name:"start_session",arguments:{}}})});
    const jwt=await mint(f.base+"/mcp");
    for(const credential of [createToken(f.db,ui,"Personal CLI").token,jwt]){
      const r=await run(credential);assert.equal(r.status,200);const text=await r.text();assert.match(text,/JWT private deck/);assert.doesNotMatch(text,/Owner invisible deck/);
    }
    assert.equal((await run(await mint("https://other.invalid/mcp"))).status,401);
    assert.equal((await run(await mint(f.base+"/mcp","-1s"))).status,401);
    assert.equal((await run(await mint(f.base+"/mcp","2m",{scope:""}))).status,401);
    assert.equal((await run(await mint(f.base+"/mcp","2m",{cnf:{jkt:"bound-key"}}))).status,401);
    assert.equal(f.db.prepare("SELECT label FROM oauth_usage WHERE user_id=?").get(id)!.label,"Test OAuth client");
    f.db.exec("DELETE FROM oauthConsent");assert.equal((await run(jwt)).status,401);
  }finally{await f.close();}
});

test("rate limiting is a real sliding window and hosted HTTP sends Retry-After",async()=>{
  let now=0;const limit=createRateLimiter(()=>now);
  for(let i=0;i<LIMITS.search.count;i++)limit("search","one");
  assert.throws(()=>limit("search","one"),e=>e instanceof RateLimitError && e.retryAfter===60);
  limit("search","two");now=60_000;limit("search","one");
  const f=await fixture();try{
    let r:Response|null=null;
    for(let i=0;i<=LIMITS.auth.count;i++)r=await f.call("/.well-known/oauth-authorization-server");
    assert.equal(r!.status,429);assert.ok(Number(r!.headers.get("retry-after"))>0);
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
    const owner=createDeck(db,OWNER,"Preserve this"),before=db.prepare("SELECT * FROM decks WHERE id=?").get(owner);
    const data=exportAccount(db,ui);assert.match(data.decks[0].name,/Delete/);assert.equal(data.tables.account.length,1);assert.equal(data.tables.verification.length,1);
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

test("Discord callback provisions atomically; DCR → S256 PKCE → consent → resource-bound JWT → MCP and refresh",async()=>{
  const f=await fixture(),realFetch=globalThis.fetch;
  let identity="100000000000000000",name="first_discord";
  globalThis.fetch=async(input,init)=>{
    const url=String(input instanceof Request?input.url:input);
    if(url.startsWith("https://discord.com/api/oauth2/token"))return Response.json({access_token:"fixture-provider-token",token_type:"Bearer",expires_in:3600,refresh_token:"fixture-refresh",scope:"identify email"});
    if(decodeURIComponent(url)==="https://discord.com/api/users/@me")return Response.json({id:identity,username:name,global_name:name,email:`${identity}@test.invalid`,verified:true,avatar:null,discriminator:"0"});
    return realFetch(input,init);
  };
  const cookies=(r:Response)=>r.headers.getSetCookie().map(s=>s.split(";")[0]).join("; ");
  async function login(code?:string) {
    let cookie="";
    if(code){const intent=await f.call("/api/signup",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({code})});assert.equal(intent.status,200);cookie=cookies(intent);}
    const start=await f.call("/api/auth/sign-in/social",{method:"POST",headers:{Cookie:cookie,"Content-Type":"application/json"},body:JSON.stringify({provider:"discord",callbackURL:f.base+"/",errorCallbackURL:f.base+"/signin"})});
    assert.equal(start.status,200);cookie+="; "+cookies(start);
    const url=new URL((await start.json() as any).url),state=url.searchParams.get("state")!;
    const callback=await f.call(`/api/auth/callback/discord?code=fixture-code&state=${encodeURIComponent(state)}`,{redirect:"manual",headers:{Cookie:cookie}});
    assert.equal(callback.status,302);assert.equal(callback.headers.get("location"),f.base+"/",await callback.text());
    return cookies(callback);
  }
  try{
    createDeck(f.db,OWNER,"Preserved owner list");
    f.db.exec("CREATE TRIGGER fail_provider_account BEFORE INSERT ON account BEGIN SELECT RAISE(ABORT,'test rollback'); END");
    await assert.rejects(login(env.DECKBUILDER_CLAIM_CODE));
    assert.equal(f.db.prepare("SELECT auth_id FROM users WHERE id=1").get()!.auth_id,null);
    assert.equal(f.db.prepare("SELECT count(*) n FROM user").get()!.n,0);
    f.db.exec("DROP TRIGGER fail_provider_account");
    const ownerCookie=await login(env.DECKBUILDER_CLAIM_CODE);
    assert.equal(f.db.prepare("SELECT count(*) n FROM users WHERE id=1 AND auth_id IS NOT NULL").get()!.n,1);
    assert.equal((await f.call("/api/decks",{headers:{Cookie:ownerCookie}})).status,200);
    identity="200000000000000000";name="invited_discord";
    const cookie=await login(createInvite(f.db,OWNER).code);
    const session=await (await f.call("/api/session",{headers:{Cookie:cookie}})).json() as any;
    assert.notEqual(session.user.id,1);assert.deepEqual(await (await f.call("/api/decks",{headers:{Cookie:cookie}})).json(),[]);
    // Existing identities need no invite and do not create another tenant.
    await login();assert.equal(f.db.prepare("SELECT count(*) n FROM users").get()!.n,2);
    const registration=await f.call("/api/auth/oauth2/register",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({client_name:"Test CLI",application_type:"native",redirect_uris:["http://127.0.0.1:49152/callback"],token_endpoint_auth_method:"none",grant_types:["authorization_code","refresh_token"],response_types:["code"],scope:"deck-builder offline_access"})});
    assert.equal(registration.status,201,await registration.clone().text());const client=await registration.json() as any;
    const verifier=randomBytes(32).toString("base64url"),challenge=createHash("sha256").update(verifier).digest("base64url");
    const params=new URLSearchParams({client_id:client.client_id,redirect_uri:"http://127.0.0.1:49152/callback",response_type:"code",scope:"deck-builder offline_access",resource:f.base+"/mcp",state:"client-state",code_challenge:challenge,code_challenge_method:"S256"});
    const authorize=await f.call("/api/auth/oauth2/authorize?"+params,{redirect:"manual",headers:{Cookie:cookie}});
    assert.equal(authorize.status,200);const consentUrl=new URL((await authorize.json() as any).url,f.base);assert.equal(consentUrl.pathname,"/consent");
    const consentPage=await f.call(consentUrl.pathname+consentUrl.search,{headers:{Cookie:cookie}});assert.equal(consentPage.status,200);assert.match(await consentPage.text(),/Test CLI/);
    const consent=await f.call("/api/auth/oauth2/consent",{method:"POST",headers:{Cookie:cookie,"Content-Type":"application/json",Accept:"application/json"},body:JSON.stringify({accept:true,oauth_query:consentUrl.search.slice(1)})});
    assert.equal(consent.status,200,await consent.clone().text());const redirect=new URL((await consent.json() as any).url);assert.equal(redirect.searchParams.get("state"),"client-state");
    const token=await f.call("/api/auth/oauth2/token",{method:"POST",headers:{"Content-Type":"application/x-www-form-urlencoded"},body:new URLSearchParams({grant_type:"authorization_code",code:redirect.searchParams.get("code")!,client_id:client.client_id,redirect_uri:"http://127.0.0.1:49152/callback",code_verifier:verifier,resource:f.base+"/mcp"})});
    assert.equal(token.status,200,await token.clone().text());const credentials=await token.json() as any;assert.ok(credentials.access_token.split(".").length===3);assert.ok(credentials.refresh_token);
    const actor=await f.auth.verifyAccessToken(credentials.access_token);assert.equal(actor?.user_id,session.user.id);assert.equal(actor?.label,"Test CLI");
    const renewal=await f.call("/api/auth/oauth2/token",{method:"POST",headers:{"Content-Type":"application/x-www-form-urlencoded"},body:new URLSearchParams({grant_type:"refresh_token",refresh_token:credentials.refresh_token,client_id:client.client_id,resource:f.base+"/mcp"})});
    assert.equal(renewal.status,200,await renewal.clone().text());const renewed=await renewal.json() as any;assert.notEqual(renewed.refresh_token,credentials.refresh_token);
    const settings=await (await f.call("/api/settings",{headers:{Cookie:cookie}})).json() as any;
    assert.equal(settings.agent.oauth_clients[0].client_id,client.client_id);
    assert.equal((await f.call("/api/account/oauth",{method:"DELETE",headers:{Cookie:cookie,"Content-Type":"application/json"},body:JSON.stringify({client_id:client.client_id})})).status,200);
    assert.equal(f.db.prepare("SELECT count(*) n FROM oauthConsent").get()!.n,0);
    assert.equal(f.db.prepare("SELECT count(*) n FROM oauthRefreshToken").get()!.n,0);
    assert.equal(await f.auth.verifyAccessToken(credentials.access_token),null);
    assert.equal((await f.call("/mcp",{method:"POST",headers:{Authorization:`Bearer ${credentials.access_token}`}})).status,401);
    const refreshAgain=()=>f.call("/api/auth/oauth2/token",{method:"POST",headers:{"Content-Type":"application/x-www-form-urlencoded"},body:new URLSearchParams({grant_type:"refresh_token",refresh_token:renewed.refresh_token,client_id:client.client_id,resource:f.base+"/mcp"})});
    assert.equal((await refreshAgain()).status,400);
    const reauthorize=await f.call("/api/auth/oauth2/authorize?"+params,{headers:{Cookie:cookie}});
    const query=new URL((await reauthorize.json() as any).url,f.base).search.slice(1);
    assert.equal((await f.call("/api/auth/oauth2/consent",{method:"POST",headers:{Cookie:cookie,"Content-Type":"application/json"},body:JSON.stringify({accept:true,oauth_query:query})})).status,200);
    assert.equal(await f.auth.verifyAccessToken(credentials.access_token),null,"re-consent must not revive an old JWT");
    assert.equal((await refreshAgain()).status,400,"re-consent must not revive a deleted refresh token");
    assert.equal((await f.call("/api/account/export",{headers:{Cookie:cookie}})).status,200);
    const deletion=await f.call("/api/account",{method:"DELETE",headers:{Cookie:cookie,"Content-Type":"application/json"},body:JSON.stringify({handle:session.user.handle})});assert.equal(deletion.status,200,await deletion.text());
    assert.equal(await f.auth.verifyAccessToken(credentials.access_token),null);assert.equal((await f.call("/api/decks",{headers:{Cookie:cookie}})).status,401);
    assert.deepEqual(f.db.prepare("PRAGMA foreign_key_check").all(),[]);
  }finally{globalThis.fetch=realFetch;await f.close();}
});

test("CIMD transport rejects private targets, credentials, non-HTTPS and unsafe methods",async()=>{
  const {fetchClientMetadata}=await import("../src/cimd-transport.ts");
  for(const url of ["http://example.com/client.json","https://user:password@example.com/client.json","https://127.0.0.1/client.json","https://[::1]/client.json","https://localhost/client.json"])
    await assert.rejects(fetchClientMetadata(url),TypeError);
  await assert.rejects(fetchClientMetadata("https://example.com/client.json",{method:"POST"}),TypeError);
});

test("hosted proxy trust is explicit; only the configured public Host/Origin passes; local mode exposes no OAuth",async()=>{
  const {allowedRequest,clientIp}=await import("../src/mode.ts"),savedOrigin=process.env.DECKBUILDER_PUBLIC_ORIGIN,savedTrust=process.env.DECKBUILDER_TRUST_PROXY;
  process.env.DECKBUILDER_PUBLIC_ORIGIN="https://decks.example";
  const req={headers:{host:"internal:8787",origin:"https://decks.example","x-forwarded-host":"decks.example","x-forwarded-proto":"https","x-forwarded-for":"198.51.100.7"},socket:{remoteAddress:"127.0.0.1"}} as any;
  try{
    delete process.env.DECKBUILDER_TRUST_PROXY;assert.equal(allowedRequest(req,"hosted"),false);assert.equal(clientIp(req),"127.0.0.1");
    process.env.DECKBUILDER_TRUST_PROXY="1";assert.equal(allowedRequest(req,"hosted"),true);assert.equal(clientIp(req),"198.51.100.7");
    for(const patch of [{"x-forwarded-proto":"http"},{"x-forwarded-host":"attacker.example"},{origin:"https://attacker.example"},{"x-forwarded-host":"decks.example,attacker.example"}])assert.equal(allowedRequest({...req,headers:{...req.headers,...patch}},"hosted"),false);
    const db=testDb(),app=createApp(db,{mode:"local"});await new Promise<void>(r=>app.server.listen(0,"127.0.0.1",r));
    try{
      const base=`http://127.0.0.1:${(app.server.address() as AddressInfo).port}`;
      for(const path of ["/.well-known/oauth-protected-resource/mcp","/.well-known/oauth-authorization-server","/api/auth/get-session"]){assert.equal((await fetch(base+path)).status,404,path);}
      assert.equal((await fetch(base+"/api/session").then(r=>r.json()) as any).mode,"local");
    }finally{await app.close();db.close();}
  }finally{if(savedOrigin===undefined)delete process.env.DECKBUILDER_PUBLIC_ORIGIN;else process.env.DECKBUILDER_PUBLIC_ORIGIN=savedOrigin;if(savedTrust===undefined)delete process.env.DECKBUILDER_TRUST_PROXY;else process.env.DECKBUILDER_TRUST_PROXY=savedTrust;}
});

test("a live modern MCP subscription never blocks hosted UI or tool requests",async()=>{
  const {Client,StreamableHTTPClientTransport}=await import("@modelcontextprotocol/client");
  const f=await fixture(),client=new Client({name:"streaming-cli",version:"1"},{versionNegotiation:{mode:{pin:"2026-07-28"}}});
  let subscription:Awaited<ReturnType<typeof client.listen>>|undefined;
  try{
    const personal=createToken(f.db,OWNER,"Streaming CLI").token;
    await client.connect(new StreamableHTTPClientTransport(new URL(f.base+"/mcp"),{requestInit:{headers:{Authorization:`Bearer ${personal}`}}}));
    subscription=await client.listen({toolsListChanged:true},{timeout:2000});
    const health=await f.call("/api/health",{signal:AbortSignal.timeout(2000)});assert.equal(health.status,200);
    const result=await client.callTool({name:"start_session",arguments:{}},{timeout:2000});assert.ok(!result.isError);
  }finally{await subscription?.close();await client.close();await f.close();}
});

test("every hosted-only API route is classified; UI sessions alone authorize self/account/admin actions",async()=>{
  const f=await fixture();try{
    const local=createApp(f.db,{mode:"local"});
    const key=(r:{method:string;pattern:RegExp})=>`${r.method} ${r.pattern.source}`;
    const localKeys=new Set(local.routes.map(key));await local.close();
    const routes=[
      ["DELETE","/api/account/oauth","self"],["GET","/api/account","self"],["PATCH","/api/account/handle","self"],
      ["GET","/api/account/export","self"],["DELETE","/api/account","self"],
      ["GET","/api/admin","admin"],["POST","/api/admin/invites","admin"],["POST","/api/admin/jobs","admin"],
    ] as const;
    const added=f.app.routes.filter(r=>!localKeys.has(key(r)));
    assert.equal(added.length,routes.length);
    for(const r of added)assert.equal(routes.filter(([method,path])=>r.method===method&&r.pattern.test(path)).length,1,key(r));
    const userId=enroll(f.db,"route-user",createInvite(f.db,OWNER).code).user_id,cookie=await f.session("route-user");
    const personal=createToken(f.db,{user_id:userId,via:"ui",label:"ui"},"Not a UI session").token;
    for(const [method,path,kind] of routes){
      const body=method==="GET"?undefined:JSON.stringify({handle:"route-user",job:"backup"});
      for(const headers of [{},{Authorization:`Bearer ${personal}`}] as Record<string,string>[])assert.equal((await f.call(path,{method,headers:{...headers,"Content-Type":"application/json"},body})).status,401,path);
      if(kind==="admin")assert.equal((await f.call(path,{method,headers:{Cookie:cookie,"Content-Type":"application/json"},body})).status,403,path);
    }
  }finally{await f.close();}
});

test("a slow incoming hosted body does not reserve the shared database",async()=>{
  const {request}=await import("node:http"),f=await fixture();
  const slow=request(f.base+"/api/signup",{method:"POST",headers:{Origin:f.base,"Content-Type":"application/json","Content-Length":"100"}});
  slow.on("error",()=>{});slow.write('{"code":');
  try{assert.equal((await f.call("/api/health",{signal:AbortSignal.timeout(1000)})).status,200);}
  finally{slow.destroy();await f.close();}
});

test("background failure persistence waits for the hosted database reservation",async()=>{
  const {createJobs}=await import("../src/jobs.ts"),{spawn}=await import("node:child_process"),f=await fixture();
  let release=()=>{},entered=()=>{},exited=()=>{};
  const gate=new Promise<void>(r=>{release=r;}),started=new Promise<void>(r=>{entered=r;}),exit=new Promise<void>(r=>{exited=r;});
  const held=f.app.withDatabase(async()=>{f.db.exec("BEGIN");entered();await gate;f.db.exec("ROLLBACK");});await started;
  const jobs=createJobs(f.db,{writeFailure:fn=>f.app.withDatabase(fn),spawnJob(){const child=spawn(process.execPath,["-e","process.exit(1)"],{stdio:"pipe"});child.once("close",()=>exited());return child;}});
  try{
    jobs.enqueue("backup");await exit;
    assert.equal(f.db.prepare("SELECT value FROM meta WHERE key='job:backup'").get(),undefined);
    release();await held;
    for(let i=0;i<20 && jobs.status().active;i++)await new Promise(r=>setTimeout(r,10));
    assert.equal(jobs.status().backup.result,"error");
  }finally{release();await held;await jobs.close();await f.close();}
});

test("an invited visitor arriving first cannot prevent the later code-bound owner claim",()=>{
  const db=testDb();try{
    const visitor=enroll(db,"first-visitor",createInvite(db,OWNER).code,"Visitor");assert.notEqual(visitor.user_id,1);
    const owner=enroll(db,"real-owner",env.DECKBUILDER_CLAIM_CODE,"Real Owner");assert.deepEqual(owner,{user_id:1,claimed:true});
    assert.equal(db.prepare("SELECT auth_id FROM users WHERE id=1").get()!.auth_id,"real-owner");
  }finally{db.close();}
});

test("Google activates only with both credentials; hosted HTTPS cookies remain secure behind the trusted proxy",async()=>{
  const {authConfiguration}=await import("../src/auth-config.ts"),{request}=await import("node:http");
  const db=testDb(),savedOrigin=process.env.DECKBUILDER_PUBLIC_ORIGIN,savedProxy=process.env.DECKBUILDER_TRUST_PROXY,origin="https://decks.example";
  assert.ok(!authConfiguration(db,origin,{...env,GOOGLE_CLIENT_ID:"id"}).socialProviders?.google);
  assert.ok(authConfiguration(db,origin,{...env,GOOGLE_CLIENT_ID:"id",GOOGLE_CLIENT_SECRET:"secret"}).socialProviders?.google);
  process.env.DECKBUILDER_PUBLIC_ORIGIN=origin;process.env.DECKBUILDER_TRUST_PROXY="1";
  const auth=createHostedAuth(db,{...env,DECKBUILDER_PUBLIC_ORIGIN:origin});await auth.ready;
  const app=createApp(db,{mode:"hosted",auth});await new Promise<void>(r=>app.server.listen(0,"127.0.0.1",r));
  const port=(app.server.address() as AddressInfo).port;
  const call=(path:string,body?:unknown)=>new Promise<{status:number;headers:import("node:http").IncomingHttpHeaders;body:any}>((resolve,reject)=>{
    const req=request({hostname:"127.0.0.1",port,path,method:body?"POST":"GET",headers:{host:"private.internal:8787","x-forwarded-host":"decks.example","x-forwarded-proto":"https",Origin:origin,"Content-Type":"application/json"}},res=>{
      const chunks:Buffer[]=[];res.on("data",c=>chunks.push(c));res.on("end",()=>resolve({status:res.statusCode!,headers:res.headers,body:JSON.parse(Buffer.concat(chunks).toString())}));
    });req.on("error",reject);req.end(body?JSON.stringify(body):undefined);
  });
  try{
    const signup=await call("/api/signup",{code:env.DECKBUILDER_CLAIM_CODE});assert.equal(signup.status,200);
    const cookie=signup.headers["set-cookie"]!.join(";");assert.match(cookie,/; Secure/);assert.match(cookie,/HttpOnly/);assert.match(cookie,/SameSite=Lax/);
    assert.equal(signup.headers["strict-transport-security"],"max-age=31536000");
    const metadata=await call("/.well-known/oauth-authorization-server");assert.equal(metadata.status,200);assert.equal(metadata.body.issuer,origin+"/api/auth");
  }finally{await app.close();db.close();if(savedOrigin===undefined)delete process.env.DECKBUILDER_PUBLIC_ORIGIN;else process.env.DECKBUILDER_PUBLIC_ORIGIN=savedOrigin;if(savedProxy===undefined)delete process.env.DECKBUILDER_TRUST_PROXY;else process.env.DECKBUILDER_TRUST_PROXY=savedProxy;}
});
