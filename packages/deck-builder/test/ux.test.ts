// Phase 4b regressions exercise data safety and tenant boundaries, not CSS.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { openDb, getSetting } from "../src/db.ts";
import { createDemo } from "../src/demo.ts";
import { uiPreferences, setAdvancedOpen } from "../src/preferences.ts";
import { listActivity, recordActivity } from "../src/activity.ts";
import { createDeck, setDeckCategory, getDeck, createSlot } from "../src/deck/service.ts";
import { updateBrief } from "../src/deck/brief.ts";
import { listMemories, remember } from "../src/memory.ts";
import { setProfile, getProfile } from "../src/documents.ts";
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
  assert.deepEqual(uiPreferences(db,OWNER),{has_agent_activity:false,advanced_open:false});
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
  assert.ok(listActivity(db,second).every(e=>e.user_id===2));
  assert.equal(uiPreferences(db,second).has_agent_activity,true);
 }finally{db.close();}
 const empty=openDb(":memory:");try {
  assert.throws(()=>createDemo(empty,OWNER),/sample deck needs card data/);
  assert.equal(empty.prepare("SELECT COUNT(*) n FROM decks").get()!.n,0);
  assert.equal(empty.prepare("SELECT COUNT(*) n FROM activity").get()!.n,0);
 }finally{empty.close();}
});

test("activity/latest and 100-row pages are tenant-scoped; memory filters run before pagination", async()=>{
 const db=testDb();db.exec("INSERT INTO users(id,handle,display_name) VALUES (2,'second','Second')");
 const second={...OWNER,user_id:2};const foreign=createDeck(db,second,"Second private deck");
 recordActivity(db,agent("foreign",2),"write",foreign,"Not yours");
 for(let i=0;i<105;i++) {
  recordActivity(db,agent("writer"),"write",null,`Event ${i}`);
  remember(db,OWNER,{scope:"global",text:`Preference number ${i}: variety matters`});
 }
 const app=createApp(db);await new Promise<void>(resolve=>app.server.listen(0,"127.0.0.1",resolve));
 const base=`http://127.0.0.1:${(app.server.address() as AddressInfo).port}`;
 try {
  const latest=await(await fetch(base+"/api/activity/latest")).json();assert.equal(latest.summary,"Event 104");assert.equal(latest.user_id,1);
  const first=await(await fetch(base+"/api/activity")).json();const next=await(await fetch(base+"/api/activity?offset=100")).json();assert.equal(first.length,100);assert.equal(next.length,5);assert.equal(new Set([...first,...next].map(e=>e.id)).size,105);
  const memories=await(await fetch(base+"/api/memories")).json();assert.equal(memories.memories.length,100);assert.equal(memories.has_more,true);assert.ok(!JSON.stringify(memories.scopes).includes("Second private deck"));
  const page=await(await fetch(base+"/api/memories?offset=100&basis=stated")).json();assert.equal(page.memories.length,5);assert.equal(page.has_more,false);
  const none=await(await fetch(base+"/api/memories?basis=inferred")).json();assert.equal(none.memories.length,0);assert.equal(none.has_more,false);
 } finally {await app.close();db.close();}
});
