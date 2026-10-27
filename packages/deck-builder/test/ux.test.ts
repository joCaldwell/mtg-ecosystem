// Phase 4b regressions exercise data safety and tenant boundaries, not CSS.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { openDb, getSetting } from "../src/db.ts";
import { createDemo } from "../src/demo.ts";
import { uiPreferences, setAdvancedOpen, setSidebarCollapsed } from "../src/preferences.ts";
import { latestActivity, listActivity, recordActivity } from "../src/activity.ts";
import { createDeck, deleteDeck, setDeckCategory, getDeck, createSlot } from "../src/deck/service.ts";
import { createProposal } from "../src/deck/proposals.ts";
import { applyChanges } from "../src/deck/membership.ts";
import { updateBrief } from "../src/deck/brief.ts";
import { listMemories, remember } from "../src/memory.ts";
import { setProfile, getProfile, proposeDocumentEdit } from "../src/documents.ts";
import { ingestCards } from "../src/ingest.ts";
import { createToken } from "../src/tokens.ts";
import { createApp } from "../src/app.ts";
import { FIXTURES } from "./fixtures.ts";
import { OWNER, agent } from "./actors.ts";
import { testDb, add } from "./helpers.ts";
import type { AddressInfo } from "node:net";

// The v14 decks shape after phase 4a. Explicit SQL keeps this test independent
// of v15's reconstruction algorithm and includes every sharing/lineage column.
const V14_DECKS = `CREATE TABLE decks_v14 (
 id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL REFERENCES users(id),
 name TEXT NOT NULL, category TEXT NOT NULL DEFAULT 'theorycrafted' CHECK (category IN ('paper','theorycrafted','legacy')),
 parent_id INTEGER REFERENCES decks(id) ON DELETE SET NULL, ci_mask INTEGER,
 color_identity TEXT NOT NULL DEFAULT '', revision INTEGER NOT NULL DEFAULT 0,
 created_at TEXT NOT NULL DEFAULT (datetime('now')), visibility TEXT NOT NULL DEFAULT 'private' CHECK (visibility IN ('private','unlisted','public')),
 slug TEXT, publish_decisions INTEGER NOT NULL DEFAULT 0 CHECK (publish_decisions IN (0,1)),
 accept_suggestions INTEGER NOT NULL DEFAULT 0 CHECK (accept_suggestions IN (0,1)),
 parent_owner_handle TEXT, parent_slug TEXT, parent_name TEXT, UNIQUE(user_id,name)
)`;

test("v14 → v15 preserves both tenants, all relations, prose, sharing and the deleted-id sequence", () => {
 const dir=mkdtempSync(join(tmpdir(),"ux-v14-")), path=join(dir,"test.db");
 try {
  const db=openDb(path);ingestCards(db,FIXTURES);
  db.exec("INSERT INTO users(id,handle,display_name) VALUES (2,'second','Second')");
  const ids=[];
  for(const user_id of [1,2]) {
   const actor={...OWNER,user_id};const id=createDeck(db,actor,`Legacy and rulings ${user_id}`);ids.push(id);
   const slot=createSlot(db,actor,id,"Utility");
   // Membership always goes through the capability layer.
   if(user_id===1)add(db,id,"id-forest",{slotId:slot});
   updateBrief(db,actor,id,{thesis:"My legacy list keeps the old rulings."});
   remember(db,actor,{scope:"deck",deck_id:id,text:"legacy and rulings are my own words"});
   setDeckCategory(db,actor,id,"archived");
   db.prepare("UPDATE decks SET visibility='public',slug=?,publish_decisions=1 WHERE id=?").run(`archive00${user_id}`,id);
  }
  const before=ids.map((id,i)=>getDeck(db,{...OWNER,user_id:i+1},id));
  const memories=db.prepare("SELECT * FROM memories").all();const log=db.prepare("SELECT * FROM log").all();
  db.exec(`PRAGMA foreign_keys=OFF; PRAGMA legacy_alter_table=ON; ${V14_DECKS};
   INSERT INTO decks_v14 SELECT id,user_id,name,CASE category WHEN 'archived' THEN 'legacy' ELSE category END,parent_id,ci_mask,color_identity,revision,created_at,visibility,slug,publish_decisions,accept_suggestions,parent_owner_handle,parent_slug,parent_name FROM decks;
   DROP TABLE decks; ALTER TABLE decks_v14 RENAME TO decks;
   CREATE INDEX idx_decks_user ON decks(user_id); CREATE UNIQUE INDEX idx_decks_slug ON decks(slug);
   UPDATE sqlite_sequence SET seq=999 WHERE name='decks'; DELETE FROM user_settings WHERE key='advanced_open';
   UPDATE meta SET value='14' WHERE key='schema_version';`);db.close();
  const migrated=openDb(path);
  assert.equal(migrated.prepare("SELECT COUNT(*) n FROM decks WHERE category='legacy'").get()!.n,0);
  assert.equal(migrated.prepare("SELECT COUNT(*) n FROM decks WHERE category='archived'").get()!.n,2);
  assert.deepEqual(ids.map((id,i)=>getDeck(migrated,{...OWNER,user_id:i+1},id)),before);
  assert.deepEqual(migrated.prepare("SELECT * FROM memories").all(),memories);
  assert.deepEqual(migrated.prepare("SELECT * FROM log").all(),log);
  for(const user_id of [1,2])assert.equal(uiPreferences(migrated,{...OWNER,user_id}).advanced_open,true);
  assert.throws(()=>migrated.prepare("UPDATE decks SET category='legacy'").run(),/CHECK/);
  assert.ok(createDeck(migrated,OWNER,"After migration")>999);
  assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(),[]);
  assert.equal(migrated.prepare("PRAGMA integrity_check").get()!.integrity_check,"ok");
  migrated.close();const reopened=openDb(path);assert.equal(getSetting(reopened,OWNER,"advanced_open"),"true");reopened.close();
 } finally {rmSync(dir,{recursive:true,force:true});}
});

test("first agent write changes only its principal's voice; token creation and reads do not", () => {
 const db=testDb();try {
  db.exec("INSERT INTO users(id,handle,display_name) VALUES (2,'second','Second')");
  const second={...OWNER,user_id:2};
  assert.deepEqual(uiPreferences(db,OWNER),{has_agent_activity:false,advanced_open:false,sidebar_collapsed:false});
  createToken(db,OWNER,"unused");listMemories(db,agent("reader"));
  assert.equal(uiPreferences(db,OWNER).has_agent_activity,false);
  remember(db,agent("writer"),{scope:"global",text:"I prefer longer games"});
  assert.equal(uiPreferences(db,OWNER).has_agent_activity,true);
  assert.equal(uiPreferences(db,second).has_agent_activity,false);
  setAdvancedOpen(db,second,true);assert.equal(uiPreferences(db,second).advanced_open,true);
  assert.equal(uiPreferences(db,OWNER).advanced_open,false);
  assert.throws(()=>setAdvancedOpen(db,agent("writer"),true),{status:403});
  assert.throws(()=>setAdvancedOpen(db,OWNER,"true"),/Advanced/);
 }finally{db.close();}
});

test("every kind of real agent write enables the voice; the same writes by the sample label do not", () => {
 for(const label of ["writer","sample-agent"]) for(const kind of ["memory","proposal","document edit","log","activity"]) {
  const db=testDb();try {
   const ag=agent(label),deck=createDeck(db,OWNER,"Probe deck");
   add(db,deck,"id-forest");
   if(kind==="memory") remember(db,ag,{scope:"deck",deck_id:deck,text:"Likes lands"});
   if(kind==="proposal") createProposal(db,ag,deck,{items:[{action:"add",oracle_id:"id-solring",reason:"Ramp"}]});
   if(kind==="document edit") proposeDocumentEdit(db,ag,deck,"thesis",{content:"A new thesis"},"Because");
   if(kind==="log") applyChanges(db,ag,deck,[{action:"add",oracle_id:"id-solring"}],{kind:"edit"});
   if(kind==="activity") recordActivity(db,ag,"write",deck,"A write");
   assert.equal(uiPreferences(db,OWNER).has_agent_activity,label==="writer",`${label} ${kind}`);
  } finally {db.close();}
 }
});

test("the settings probes are indexed, never a scan across tenants", () => {
 const db=testDb();try {
  for(const [table,column] of [["memories","user_id"],["proposals","author_user_id"],["document_edits","user_id"],["log","user_id"],["activity","user_id"]]) {
   const plan=db.prepare(`EXPLAIN QUERY PLAN SELECT 1 FROM ${table} WHERE ${column}=? AND ${table==="activity"?"actor_via":"author_via"}='agent' AND ${table==="activity"?"actor_label":"author_label"}<>? LIMIT 1`).all(1,"sample-agent") as {detail:string}[];
   assert.match(plan.map(p=>p.detail).join(" "),/USING (COVERING )?INDEX/,table);
  }
 } finally {db.close();}
});

test("sample creation is atomic, idempotent, human-only and leaves global/profile data untouched",()=>{
 const db=testDb();try{
  db.exec("INSERT INTO users(id,handle,display_name) VALUES (2,'second','Second')");
  const second={...OWNER,user_id:2};
  setProfile(db,second,"User-authored legacy and rulings");
  const global=remember(db,second,{scope:"global",text:"My original preference"}).memory;
  const ownerDeck=createDeck(db,OWNER,"Existing owner's deck");
  assert.throws(()=>createDemo(db,agent("untrusted",2)),{status:403});
  const sample=createDemo(db,second);assert.equal(createDemo(db,second).deckId,sample.deckId);
  assert.equal(db.prepare("SELECT COUNT(*) n FROM decks WHERE user_id=2").get()!.n,1);
  assert.equal(getProfile(db,second).body_md,"User-authored legacy and rulings");
  assert.deepEqual(listMemories(db,second,{scope:"global"}),[global]);
  assert.equal(getDeck(db,OWNER,ownerDeck).deck.name,"Existing owner's deck");
  assert.equal(getDeck(db,second,sample.deckId).deck.visibility,"private");
  assert.equal(db.prepare("SELECT COUNT(*) n FROM ownership_evidence WHERE user_id=2").get()!.n,0,"sample tags never assert physical ownership");
  // §11.3: the sample is demonstration data, not a connected agent. Its
  // synthetic writes are attributed to the reserved label and write no
  // activity, so the voice and the sidebar presence stay deck-first.
  assert.equal(uiPreferences(db,second).has_agent_activity,false);
  assert.equal(latestActivity(db,second),null);
  assert.deepEqual(listActivity(db,second),[]);
  const memories=listMemories(db,second,{deckId:sample.deckId});
  assert.ok(memories.some(m=>m.text.startsWith("The owner goldfished")),"the owner's reported results are the sample agent's record");
  for(const m of memories) assert.deepEqual([m.author_via,m.author_label,m.owner_touched],["agent","sample-agent",false],m.text);
  for(const table of ["proposals","document_edits"]) assert.ok(db.prepare(`SELECT COUNT(*) n FROM ${table} WHERE author_via='agent' AND author_label='sample-agent'`).get()!.n);
  // Deleting the sample removes its deck-scoped records; the flag stays off.
  deleteDeck(db,second,sample.deckId);
  assert.equal(uiPreferences(db,second).has_agent_activity,false);
  // The first real agent write turns it on.
  recordActivity(db,agent("claude-code",2),"remember",null,"A real write");
  assert.equal(uiPreferences(db,second).has_agent_activity,true);
  assert.equal(latestActivity(db,second)!.actor_label,"claude-code");
 }finally{db.close();}
 const empty=openDb(":memory:");try {
  assert.throws(()=>createDemo(empty,OWNER),/sample deck needs card data/);
  assert.equal(empty.prepare("SELECT COUNT(*) n FROM decks").get()!.n,0);
  assert.equal(empty.prepare("SELECT COUNT(*) n FROM activity").get()!.n,0);
 }finally{empty.close();}
});

test("activity/latest and 100-row keyset pages are tenant-scoped; filters run before pagination", async()=>{
 const db=testDb();db.exec("INSERT INTO users(id,handle,display_name) VALUES (2,'second','Second')");
 const second={...OWNER,user_id:2};const foreign=createDeck(db,second,"Second private deck");
 recordActivity(db,agent("foreign",2),"write",foreign,"Not yours");
 for(let i=0;i<105;i++) {
  recordActivity(db,agent(i%2?"codex":"writer"),"write",null,`Event ${i}`);
  remember(db,OWNER,{scope:"global",text:`Preference number ${i}: variety matters`});
 }
 // A sample row, as an unreleased build wrote: never the sidebar presence.
 recordActivity(db,agent("sample-agent"),"create_sample",null,"Created sample deck");
 const app=createApp(db);await new Promise<void>(resolve=>app.server.listen(0,"127.0.0.1",resolve));
 const base=`http://127.0.0.1:${(app.server.address() as AddressInfo).port}`;
 const get=async(path:string)=>(await fetch(base+path)).json();
 try {
  const latest=await get("/api/activity/latest");assert.equal(latest.summary,"Event 104");assert.equal(latest.user_id,1);
  const first=await get("/api/activity");
  assert.equal(first.entries.length,100);assert.equal(first.has_more,true);
  assert.deepEqual(first.actors,["codex","sample-agent","writer"],"every label of this principal, none of another's");
  // A write between loads neither shifts nor repeats a row.
  recordActivity(db,agent("writer"),"write",null,"Written between pages");
  const next=await get(`/api/activity?before_id=${first.entries.at(-1).id}`);
  assert.equal(next.entries.length,6);assert.equal(next.has_more,false);
  assert.equal(new Set([...first.entries,...next.entries].map((e:any)=>e.id)).size,106);
  assert.ok(![...first.entries,...next.entries].some((e:any)=>e.summary==="Written between pages"));
  // The agent filter applies before the page, not to the page.
  const codex=await get("/api/activity?actor=codex");
  assert.equal(codex.entries.length,52);assert.equal(codex.has_more,false);assert.ok(codex.entries.every((e:any)=>e.actor_label==="codex"));
  assert.deepEqual(codex.actors,first.actors);
  assert.equal((await fetch(base+"/api/activity?before_id=x")).status,400);
  // Memories: keyset on (revised_at, id), filters before the page.
  const memories=await get("/api/memories");assert.equal(memories.memories.length,100);assert.equal(memories.has_more,true);assert.ok(!JSON.stringify(memories.scopes).includes("Second private deck"));
  const last=memories.memories.at(-1);
  remember(db,OWNER,{scope:"global",text:"Written between memory pages"});
  const page=await get(`/api/memories?basis=stated&before_id=${last.id}&before_revised_at=${encodeURIComponent(last.revised_at)}`);
  assert.equal(page.memories.length,5);assert.equal(page.has_more,false);
  assert.equal(new Set([...memories.memories,...page.memories].map((m:any)=>m.id)).size,105);
  // A search filters the browse; its pages continue the same way.
  const searched=await get("/api/memories?q=variety");assert.equal(searched.memories.length,100);assert.equal(searched.has_more,true);
  const tail=searched.memories.at(-1);
  assert.equal((await get(`/api/memories?q=variety&before_id=${tail.id}&before_revised_at=${encodeURIComponent(tail.revised_at)}`)).memories.length,5);
  assert.equal((await fetch(base+`/api/memories?before_id=${last.id}`)).status,400,"the cursor needs both keys");
  const none=await get("/api/memories?basis=inferred");assert.equal(none.memories.length,0);assert.equal(none.has_more,false);
 } finally {await app.close();db.close();}
});

test("POST /api/demo names the skipped cards and has its own hosted rate-limit bucket", async()=>{
 const {fixture,enroll}=await import("./hosted-fixture.ts");
 const {createInvite}=await import("../src/accounts.ts");
 const {LIMITS}=await import("../src/ratelimit.ts");
 const {tryResolveCard}=await import("../src/cards.ts");
 const f=await fixture();
 try {
  enroll(f.db,"sampler",createInvite(f.db,OWNER).code);const cookie=await f.session("sampler");
  const post=()=>f.call("/api/demo",{method:"POST",headers:{Cookie:cookie}});
  const first=await post();assert.equal(first.status,200);
  const body=await first.json() as {state:{deck:{name:string}};missing:string[]};
  assert.equal(body.state.deck.name,"Sample — Atraxa Superfriends");
  // The fixture card set lacks most optional cards: each is named, none invented.
  assert.ok(body.missing.length>0);
  for(const name of body.missing) assert.ok("problem" in tryResolveCard(f.db,name),name);
  for(let i=1;i<LIMITS.demo.count;i++) assert.equal((await post()).status,200);
  const limited=await post();assert.equal(limited.status,429);assert.ok(Number(limited.headers.get("retry-after"))>0);
  assert.equal((await f.call("/api/decks",{headers:{Cookie:cookie}})).status,200,"the general UI budget is separate");
 } finally {await f.close();}
});

test("sidebar collapse is a persistent, UI-only preference for the caller without a migration", () => {
 const dir=mkdtempSync(join(tmpdir(),"sidebar-pref-")), path=join(dir,"test.db");
 let db=openDb(path);
 try {
  db.exec("INSERT INTO users(id,handle,display_name) VALUES (2,'second','Second')");
  const second={...OWNER,user_id:2};
  const version=db.prepare("PRAGMA user_version").get();
  assert.equal(uiPreferences(db,OWNER).sidebar_collapsed,false);
  assert.equal(uiPreferences(db,second).sidebar_collapsed,false);
  setSidebarCollapsed(db,second,true);
  assert.equal(getSetting(db,second,"sidebar_collapsed"),"true");
  assert.equal(uiPreferences(db,OWNER).sidebar_collapsed,false);
  for(const value of ["true",1,null,{}]) assert.throws(()=>setSidebarCollapsed(db,OWNER,value),{status:400});
  assert.throws(()=>setSidebarCollapsed(db,agent("writer",2),false),{status:403});
  assert.throws(()=>setSidebarCollapsed(db,null as any,true),{status:401});
  assert.equal(uiPreferences(db,second).sidebar_collapsed,true);
  db.close();db=openDb(path);
  assert.deepEqual(db.prepare("PRAGMA user_version").get(),version);
  assert.equal(uiPreferences(db,second).sidebar_collapsed,true);
  setSidebarCollapsed(db,second,false);
  assert.equal(uiPreferences(db,second).sidebar_collapsed,false);
 }finally{db.close();rmSync(dir,{recursive:true,force:true});}
});
