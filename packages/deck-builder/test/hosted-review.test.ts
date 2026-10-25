import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { fixture } from "./hosted-fixture.ts";
import { beginSignup, provisionIdentity } from "../src/auth.ts";
import { createInvite } from "../src/accounts.ts";
import { OWNER } from "./actors.ts";
const env={DECKBUILDER_CLAIM_CODE:"test-only-owner-code",BETTER_AUTH_SECRET:"test-only-auth-secret-0123456789-abcdef"};
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
