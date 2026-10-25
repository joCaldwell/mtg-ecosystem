import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { fixture } from "./hosted-fixture.ts";
import { beginSignup, provisionIdentity } from "../src/auth.ts";
import { createInvite } from "../src/accounts.ts";
import { OWNER } from "./actors.ts";
const env={DECKBUILDER_CLAIM_CODE:"test-only-owner-code-0123456789-abcdef",BETTER_AUTH_SECRET:"test-only-auth-secret-0123456789-abcdef"};
function deferred(){let resolve=()=>{};const promise=new Promise<void>(r=>{resolve=r;});return {promise,resolve};}

async function nativeClient(f:Awaited<ReturnType<typeof fixture>>,redirect="http://127.0.0.1:49152/callback",name="Claude") {
  return f.call("/api/auth/oauth2/register",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({client_name:name,application_type:"native",redirect_uris:[redirect],token_endpoint_auth_method:"none",grant_types:["authorization_code","refresh_token"],response_types:["code"],scope:"deck-builder offline_access"})});
}
test("anonymous DCR phishing with a trusted-looking name and attacker HTTPS callback is rejected",async()=>{
  const f=await fixture();try{
    const result=await nativeClient(f,"https://evil.example/cb");assert.equal(result.status,400);
    assert.equal(f.db.prepare("SELECT count(*) n FROM oauthClient").get()!.n,0);
    for(const uri of ["http://evil.example/cb","https://localhost/cb","javascript:alert(1)"])assert.equal((await nativeClient(f,uri)).status,400);
  }finally{await f.close();}
});
test("consent verifies signed parameters before rendering host, CIMD identity and loopback warning",async()=>{
  const f=await fixture();try{
    const id="consent-user",ticket=beginSignup(f.db,createInvite(f.db,OWNER).code,env);
    provisionIdentity(f.db,{id,name:id},ticket,env);const cookie=await f.session(id);
    const client=await (await nativeClient(f)).json() as any;
    const params=new URLSearchParams({client_id:client.client_id,redirect_uri:"http://127.0.0.1:49152/callback",response_type:"code",scope:"deck-builder",resource:f.base+"/mcp",state:"state",code_challenge:createHash("sha256").update(randomBytes(32)).digest("base64url"),code_challenge_method:"S256"});
    const auth=await f.call("/api/auth/oauth2/authorize?"+params,{headers:{Cookie:cookie,Accept:"application/json"}});
    const uri=new URL((await auth.json() as any).url,f.base);
    const consent=await f.call(uri.pathname+uri.search,{headers:{Cookie:cookie}});assert.equal(consent.status,200);
    const html=await consent.text();assert.match(html,/Redirect destination: 127\.0\.0\.1:49152/);assert.match(html,/Unverified app/);assert.match(html,/another app on your computer/);
    uri.searchParams.set("redirect_uri","https://evil.example/cb");assert.equal((await f.call(uri.pathname+uri.search,{headers:{Cookie:cookie}})).status,400);
    assert.equal((await f.call("/consent?client_id="+client.client_id+"&redirect_uri=https://evil.example/cb",{headers:{Cookie:cookie}})).status,400);
  }finally{await f.close();}
});
test("stalled CIMD I/O cannot delay a concurrent health or authenticated UI read",async()=>{
  const blocked=deferred(),entered=deferred();
  const f=await fixture(async()=>{entered.resolve();await blocked.promise;throw new Error("simulated metadata timeout");});
  try{
    provisionIdentity(f.db,{id:"parallel",name:"Parallel"},beginSignup(f.db,createInvite(f.db,OWNER).code,env),env);
    const cookie=await f.session("parallel");
    const pending=f.call("/api/auth/oauth2/authorize?"+new URLSearchParams({client_id:"https://stall.example/client.json",redirect_uri:"http://127.0.0.1/callback",response_type:"code",code_challenge:"a".repeat(43),code_challenge_method:"S256"}));
    await entered.promise;
    const start=performance.now();
    const [health,roster]=await Promise.all([f.call("/api/health",{signal:AbortSignal.timeout(200)}),f.call("/api/decks",{headers:{Cookie:cookie},signal:AbortSignal.timeout(200)})]);
    assert.equal(health.status,200);assert.equal(roster.status,200);assert.ok(performance.now()-start<200);
    blocked.resolve();await pending;
  }finally{blocked.resolve();await f.close();}
});
test("a slow provider token exchange does not hold the shared SQLite reservation",async()=>{
  const f=await fixture(),entered=deferred(),blocked=deferred(),original=globalThis.fetch;
  globalThis.fetch=async(input,init)=>{
    if(String(input instanceof Request?input.url:input).startsWith("https://discord.com/api/oauth2/token")){entered.resolve();await blocked.promise;return Response.json({error:"invalid_grant"},{status:400});}
    return original(input,init);
  };
  try{
    provisionIdentity(f.db,{id:"reader",name:"Reader"},beginSignup(f.db,createInvite(f.db,OWNER).code,env),env);const cookie=await f.session("reader");
    const login=await f.call("/api/auth/sign-in/social",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({provider:"discord",callbackURL:f.base+"/"})});
    const state=new URL((await login.json() as any).url).searchParams.get("state")!;
    const pending=f.call("/api/auth/callback/discord?code=junk&state="+encodeURIComponent(state),{redirect:"manual",headers:{Cookie:login.headers.getSetCookie().map(v=>v.split(";")[0]).join("; ")}});
    await entered.promise;
    const responses=await Promise.all([f.call("/api/health",{signal:AbortSignal.timeout(200)}),f.call("/api/decks",{headers:{Cookie:cookie},signal:AbortSignal.timeout(200)})]);
    assert.deepEqual(responses.map(r=>r.status),[200,200]);blocked.resolve();await pending;
  }finally{blocked.resolve();globalThis.fetch=original;await f.close();}
});
test("CIMD DNS honors abort and fetch concurrency is bounded",async()=>{
  const {createMetadataTransport}=await import("../src/cimd-transport.ts");const blocked=deferred();
  const transport=createMetadataTransport(async()=>{await blocked.promise;return [{address:"1.1.1.1",family:4}];});
  const controllers=Array.from({length:4},()=>new AbortController());
  const pending=controllers.map(c=>transport("https://stall.example/client.json",{signal:c.signal}).catch(e=>e));
  await assert.rejects(transport("https://fifth.example/client.json"),/capacity/);
  const start=performance.now();controllers.forEach(c=>c.abort());await Promise.all(pending);assert.ok(performance.now()-start<100);blocked.resolve();
});

test("a CIMD app naming itself Claude still displays its own document host and redirect host",async()=>{
  const f=await fixture(async()=>Response.json({client_id:"https://evil.example/client.json",client_name:"Claude",redirect_uris:["https://evil.example/cb"],application_type:"web",grant_types:["authorization_code"],response_types:["code"],token_endpoint_auth_method:"none"}));
  try{
    provisionIdentity(f.db,{id:"cimd-consent",name:"CIMD reader"},beginSignup(f.db,createInvite(f.db,OWNER).code,env),env);const cookie=await f.session("cimd-consent");
    const params=new URLSearchParams({client_id:"https://evil.example/client.json",redirect_uri:"https://evil.example/cb",response_type:"code",scope:"deck-builder",resource:f.base+"/mcp",state:"state",code_challenge:"a".repeat(43),code_challenge_method:"S256"});
    const auth=await f.call("/api/auth/oauth2/authorize?"+params,{headers:{Cookie:cookie,Accept:"application/json"}});
    assert.equal(auth.status,200);const uri=new URL((await auth.json() as any).url,f.base);
    const page=await f.call(uri.pathname+uri.search,{headers:{Cookie:cookie}});assert.equal(page.status,200);
    const html=await page.text();assert.match(html,/Client identity: evil\.example/);assert.match(html,/Redirect destination: evil\.example/);
  }finally{await f.close();}
});

test("hosted startup rejects short claim codes and missing legal configuration",async()=>{
  const {createHostedAuth}=await import("../src/auth.ts"),{testDb}=await import("./helpers.ts"),{env:configured}=await import("./hosted-fixture.ts");
  const db=testDb();try{
    for(const code of ["","1","x".repeat(31)])assert.throws(()=>createHostedAuth(db,{...configured,DECKBUILDER_CLAIM_CODE:code}),/32 characters/);
    for(const key of ["DECKBUILDER_OPERATOR_NAME","DECKBUILDER_CONTACT_EMAIL"] as const)assert.throws(()=>createHostedAuth(db,{...configured,[key]:""}),/OPERATOR_NAME and DECKBUILDER_CONTACT_EMAIL/);
    await createHostedAuth(db,{...configured,DECKBUILDER_CLAIM_CODE:undefined}).ready;
  }finally{db.close();}
});
test("trusted proxy uses X-Real-IP or rightmost XFF; IPv6 privacy addresses share a /64",async()=>{
  const {clientIp,ipBucket}=await import("../src/mode.ts");
  const request={headers:{"x-real-ip":"198.51.100.9","x-forwarded-for":"203.0.113.1,198.51.100.8"},socket:{remoteAddress:"127.0.0.1"}} as any;
  assert.equal(clientIp(request,{}),"127.0.0.1");assert.equal(clientIp(request,{DECKBUILDER_TRUST_PROXY:"1"}),"198.51.100.9");
  delete request.headers["x-real-ip"];assert.equal(clientIp(request,{DECKBUILDER_TRUST_PROXY:"1"}),"198.51.100.8");
  assert.equal(ipBucket("2001:db8:1:2::1"),ipBucket("2001:db8:1:2:ffff:ffff:ffff:abcd"));
  assert.notEqual(ipBucket("2001:db8:1:2::1"),ipBucket("2001:db8:1:3::1"));assert.equal(ipBucket("::ffff:192.0.2.1"),"192.0.2.1");
});
test("anonymous saturation cannot lock out fresh authenticated buckets; IPv6 rotations still hit quota",async()=>{
  const {createRateLimiter,LIMITS}=await import("../src/ratelimit.ts");const limit=createRateLimiter(()=>0);
  for(let i=0;i<10001;i++)limit("unauthenticated",`anonymous-${i}`);
  assert.doesNotThrow(()=>limit("mcp","fresh-credential"));assert.doesNotThrow(()=>limit("ui","fresh-user"));assert.doesNotThrow(()=>limit("unauthenticated","fresh-ip"));
  for(let i=0;i<LIMITS.claim.count;i++)limit("claim",`2001:db8:1:2::${i+1}`);
  assert.throws(()=>limit("claim","2001:db8:1:2::ffff"),{status:429});
});
test("claim attempts have a tight bucket despite spoofed leftmost XFF entries",async()=>{
  const {LIMITS}=await import("../src/ratelimit.ts"),f=await fixture(),old=process.env.DECKBUILDER_TRUST_PROXY;process.env.DECKBUILDER_TRUST_PROXY="1";
  try{
    for(let i=0;i<LIMITS.claim.count;i++)assert.equal((await f.call("/api/signup",{method:"POST",headers:{"Content-Type":"application/json","X-Real-IP":"198.51.100.3","X-Forwarded-For":`203.0.113.${i},198.51.100.3`},body:'{"code":"wrong"}'})).status,403);
    const limited=await f.call("/api/signup",{method:"POST",headers:{"Content-Type":"application/json","X-Real-IP":"198.51.100.3"},body:'{"code":"wrong"}'});assert.equal(limited.status,429);assert.equal(limited.headers.get("retry-after"),"60");
  }finally{if(old===undefined)delete process.env.DECKBUILDER_TRUST_PROXY;else process.env.DECKBUILDER_TRUST_PROXY=old;await f.close();}
});
test("open registration accepts a blank ticket and never claims the reserved owner",async()=>{
  const {testDb}=await import("./helpers.ts"),{env:configured}=await import("./hosted-fixture.ts");const db=testDb(),open={...configured,DECKBUILDER_INVITES_REQUIRED:"0"};
  try{
    const user=provisionIdentity(db,{id:"open-user",name:"Open"},"",open);assert.notEqual(user.user_id,1);
    const next=provisionIdentity(db,{id:"open-ticket",name:"Open ticket"},beginSignup(db,"",open),open);assert.notEqual(next.user_id,1);
    assert.equal(db.prepare("SELECT auth_id FROM users WHERE id=1").get()!.auth_id,null);
  }finally{db.close();}
});
test("Railway health is public, minimal and cookie-free; local health keeps its host boundary",async()=>{
  const {request}=await import("node:http"),{createApp}=await import("../src/app.ts"),{testDb}=await import("./helpers.ts");const f=await fixture();
  async function health(base:string,host:string,method="GET") {return new Promise<{status:number;headers:import("node:http").IncomingHttpHeaders;body:string}>(resolve=>{request(base+"/api/health",{method,headers:{Host:host,Origin:"https://other.invalid",Cookie:"bad=bad"}},res=>{let body="";res.on("data",c=>body+=c);res.on("end",()=>resolve({status:res.statusCode!,headers:res.headers,body}));}).end();});}
  try{
    const r=await health(f.base,"healthcheck.railway.app");assert.equal(r.status,200);assert.deepEqual(Object.keys(JSON.parse(r.body)).sort(),["ok","schema_version"]);assert.equal(r.headers["set-cookie"],undefined);assert.equal(r.headers["cache-control"],"no-store");assert.equal((await health(f.base,"healthcheck.railway.app","POST")).status,403);
    const db=testDb(),local=createApp(db,{mode:"local"});await new Promise<void>(r=>local.server.listen(0,"127.0.0.1",r));
    try {const base=`http://127.0.0.1:${(local.server.address() as import("node:net").AddressInfo).port}`;assert.equal((await health(base,"healthcheck.railway.app")).status,403);assert.equal((await fetch(base+"/api/health")).status,200);for(const path of ["/privacy","/terms"])assert.equal((await fetch(base+path)).status,404);}
    finally{await local.close();db.close();}
  }finally{await f.close();}
});
test("hosted legal configuration is public; hashed assets are immutable while index and APIs are private",async()=>{
  const {mkdtempSync,writeFileSync,mkdirSync,rmSync}=await import("node:fs"),{tmpdir}=await import("node:os"),{join}=await import("node:path"),{createApp}=await import("../src/app.ts");
  const f=await fixture(),root=mkdtempSync(join(tmpdir(),"auth-assets-"));mkdirSync(join(root,"assets"));writeFileSync(join(root,"index.html"),"<html>SPA</html>");writeFileSync(join(root,"assets/app-a1b2c3.js"),"export {};");
  const app=createApp(f.db,{mode:"hosted",auth:f.auth,webDist:root});await new Promise<void>(r=>app.server.listen(0,"127.0.0.1",r));const base=`http://127.0.0.1:${(app.server.address() as import("node:net").AddressInfo).port}`;
  try{
    const session=await (await f.call("/api/session")).json() as any;assert.deepEqual(session.legal,{operator_name:"Test operator",contact_email:"operator@test.invalid"});
    process.env.DECKBUILDER_PUBLIC_ORIGIN=base;
    for(const [path,cache] of [["/assets/app-a1b2c3.js","public, max-age=31536000, immutable"],["/signin","no-store"],["/privacy","no-store"],["/api/session","no-store"]]){const result=await fetch(base+path);assert.equal(result.status,200);assert.equal(result.headers.get("cache-control"),cache,path);}
  }finally{await app.close();await f.close();rmSync(root,{recursive:true,force:true});}
});
test("token exchanges use client quotas rather than sharing every user's egress-IP auth quota",async()=>{
  const f=await fixture();try{
    const client=await (await nativeClient(f)).json() as any;
    for(let i=0;i<25;i++){const response=await f.call("/api/auth/oauth2/token",{method:"POST",headers:{"Content-Type":"application/x-www-form-urlencoded"},body:new URLSearchParams({client_id:client.client_id,grant_type:"refresh_token",refresh_token:"invalid"})});assert.equal(response.status,400);}
  }finally{await f.close();}
});
test("handles reserve route words, pad neutrally and permit only the first-run collision picker",async()=>{
  const {chooseHandle,accountInfo}=await import("../src/accounts.ts"),{testDb}=await import("./helpers.ts"),{env:configured}=await import("./hosted-fixture.ts");const db=testDb();
  try{
    const first=provisionIdentity(db,{id:"short",name:"Ab"},beginSignup(db,createInvite(db,OWNER).code,configured),configured),ui={user_id:first.user_id,via:"ui" as const,label:"ui"};assert.equal(accountInfo(db,ui).handle,"ab_");assert.throws(()=>chooseHandle(db,ui,"changed"),{status:403});
    const route=provisionIdentity(db,{id:"route",name:"settings"},beginSignup(db,createInvite(db,OWNER).code,configured),configured),picker={...ui,user_id:route.user_id};assert.equal(accountInfo(db,picker).needs_handle,1);assert.notEqual(accountInfo(db,picker).handle,"settings");
    for(const name of ["admin","api","settings","signin","privacy","terms","mcp"])assert.throws(()=>chooseHandle(db,picker,name),{status:400});chooseHandle(db,picker,"chosen_name");assert.throws(()=>chooseHandle(db,picker,"again"),{status:403});
  }finally{db.close();}
});
test("deleting an invitee preserves the admin's spent invite and others' content with tombstone authorship",async()=>{
  const {createDeck}=await import("../src/deck/service.ts"),{remember}=await import("../src/memory.ts"),{testDb}=await import("./helpers.ts"),{env:configured}=await import("./hosted-fixture.ts"),{deleteAccount,accountInfo,exportAccount}=await import("../src/accounts.ts");const db=testDb();
  try{
    const code=createInvite(db,OWNER).code,id=provisionIdentity(db,{id:"delete-cross",name:"Delete"},beginSignup(db,code,configured),configured).user_id,ui={user_id:id,via:"ui" as const,label:"ui"};createDeck(db,ui,"Owned deleted");
    const memory=remember(db,OWNER,{scope:"global",text:"Keep the owner's content"});db.prepare("UPDATE memories SET author_user_id=?,author_label='private identity' WHERE id=?").run(id,memory.memory.id);
    const {createToken}=await import("../src/tokens.ts"),{createProposal}=await import("../src/deck/proposals.ts");
    const token=createToken(db,ui,"Foreign author token"),ownerDeck=createDeck(db,OWNER,"Preserve owner proposal");
    const proposal=createProposal(db,{...OWNER,via:"agent",label:"Cross-user fixture"},ownerDeck,{items:[{action:"add",oracle_id:"id-forest",reason:"Fixture"}]});
    db.prepare("UPDATE proposals SET author_user_id=?,author_token_id=? WHERE id=?").run(id,token.credential.id,proposal);
    const exported=exportAccount(db,ui);assert.equal(exported.tables.memories.length,0);assert.equal(exported.tables.proposals.length,0);deleteAccount(db,ui,accountInfo(db,ui).handle);
    assert.equal(db.prepare("SELECT author_user_id FROM proposals WHERE id=?").get(proposal)!.author_user_id,0);assert.equal(db.prepare("SELECT author_token_id FROM proposals WHERE id=?").get(proposal)!.author_token_id,null);
    assert.equal(db.prepare("SELECT used_by FROM invites WHERE code=?").get(code)!.used_by,0);assert.equal(db.prepare("SELECT text FROM memories WHERE id=?").get(memory.memory.id)!.text,"Keep the owner's content");assert.equal(db.prepare("SELECT author_user_id FROM memories WHERE id=?").get(memory.memory.id)!.author_user_id,0);assert.deepEqual(db.prepare("PRAGMA foreign_key_check").all(),[]);assert.throws(()=>beginSignup(db,code,configured),{status:403});
  }finally{db.close();}
});
test("malformed MCP URL labels do not become a server error; invalid bearers get the RFC 6750 challenge",async()=>{
  const {agentName}=await import("../src/mcp/http.ts");assert.equal(agentName("/mcp/%E0",null,undefined),"agent");const f=await fixture();
  try{const response=await f.call("/mcp/%E0",{method:"POST",headers:{Authorization:"Bearer wrong"}});assert.equal(response.status,401);assert.match(response.headers.get("www-authenticate")!,/error="invalid_token"/);}finally{await f.close();}
});

test("unused social and session-token listing endpoints are disabled",async()=>{
  const f=await fixture();try{
    for(const [method,path] of [["GET","list-sessions"],["GET","list-accounts"],["POST","link-social"],["POST","unlink-account"],["POST","revoke-session"],["POST","revoke-sessions"],["POST","revoke-other-sessions"],["POST","get-access-token"],["POST","refresh-token"]])assert.equal((await f.call("/api/auth/"+path,{method,headers:{"Content-Type":"application/json"},...(method==="POST"?{body:"{}"}:{})})).status,404,path);
  }finally{await f.close();}
});
test("unconsented DCR clients expire after a day while granted clients survive cleanup",async()=>{
  const f=await fixture();try{
    for(const id of ["stale","granted","recent"])f.db.prepare("INSERT INTO oauthClient(id,clientId,redirectUris,createdAt) VALUES(?,?,?,?)").run(id,id,'["http://127.0.0.1/cb"]',Date.now()-(id==="recent"?1:86_400_001));
    provisionIdentity(f.db,{id:"prune-user",name:"Prune"},beginSignup(f.db,createInvite(f.db,OWNER).code,env),env);await f.session("prune-user");
    f.db.prepare("INSERT INTO oauthConsent(id,clientId,userId,scopes,createdAt,updatedAt) VALUES('keep','granted','prune-user','[]',?,?)").run(Date.now(),Date.now());
    await f.call("/api/auth/jwks");assert.deepEqual(f.db.prepare("SELECT clientId FROM oauthClient ORDER BY clientId").all().map(r=>r.clientId),["granted","recent"]);
  }finally{await f.close();}
});
