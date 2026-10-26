import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { spawn } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";
import { Database } from "../src/sqlite.ts";
import { openDb, PKG_ROOT } from "../src/db.ts";
import { displayAuthor } from "../src/actor.ts";
import { createDeck } from "../src/deck/service.ts";
import { getMemory } from "../src/memory.ts";
import { getLogEntry } from "../src/deck/log.ts";
import { ownershipEvidence } from "../src/deck/sourcing.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { OWNER } from "./actors.ts";

// This is main's real v10 output, frozen at 8b6c012, not a stripped v11.
const schema=readFileSync(new URL("./schema/v10.sql",import.meta.url),"utf8");
function v10(path:string) {const db=new Database(path,{timeout:5000});db.exec(schema);return db;}
const master=(db:Database)=>db.prepare("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name").all();

test("frozen main v10 schema migrates to the same sqlite_master as a fresh v15; historical human/system authors render honestly",()=>{
  const dir=mkdtempSync(join(tmpdir(),"frozen-v10-")),path=join(dir,"old.db"),old=v10(path);
  try {
    // Today's ingest writes v13's card credit; v13 adds the column by presence.
    old.exec("ALTER TABLE cards ADD COLUMN artist TEXT");
    ingestCards(old,FIXTURES);
    old.exec(`INSERT INTO decks(id,name,category) VALUES (7,'Previous deck','paper');
      UPDATE sqlite_sequence SET seq=100 WHERE name='decks';
      INSERT INTO log(deck_id,kind,author) VALUES (7,'edit','owner (from v1)');
      INSERT INTO memories(text,scope,basis,author,owner_touched) VALUES ('Historical human preference','global','stated','owner (from v1)',1);
      INSERT INTO ownership_evidence(oracle_id,status,basis,author) VALUES ('id-forest','owned','Migrated explicit Have tag','migration');`);
    old.close();
    const migrated=openDb(path),fresh=openDb(":memory:");
    try {
      assert.deepEqual(master(migrated),master(fresh));
      const memory=getMemory(migrated,OWNER,1),log=getLogEntry(migrated,OWNER,1)!;
      for(const row of [memory,log]) {
        assert.equal(row.author,"owner (from v1)");assert.equal(row.author_user_id,1);
        assert.equal(row.author_via,"ui");assert.equal(row.author_label,"ui");assert.equal(row.author_display,"owner");
      }
      const evidence=ownershipEvidence(migrated,OWNER,"id-forest")[0];
      assert.equal(evidence.author,"migration");assert.equal(evidence.author_via,"ui");
      assert.equal(evidence.author_label,"migration");assert.equal(evidence.author_display,"System migration");
      assert.equal(displayAuthor(migrated,{author_user_id:1,author_via:"agent",author_label:"migration"}),"owner via migration","an agent's chosen label does not become a system author");
      assert.ok(createDeck(migrated,OWNER,"Next list")>100);
      assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(),[]);
      assert.deepEqual(migrated.prepare("PRAGMA integrity_check").all().map(r=>r.integrity_check),["ok"]);
    } finally {migrated.close();fresh.close();}
  } finally {if(old.isOpen)old.close();rmSync(dir,{recursive:true,force:true});}
});

test("failed migration reports a bounded set of offending FK rows and rolls back schema/data",()=>{
  const dir=mkdtempSync(join(tmpdir(),"fk-diagnostics-")),path=join(dir,"old.db"),old=v10(path);
  try {
    old.exec("PRAGMA foreign_keys=OFF");
    const insert=old.prepare("INSERT INTO memories(text,scope,deck_id,basis,author) VALUES (?,'deck',999,'inferred','agent')");
    for(let i=0;i<6;i++)insert.run(`Broken link ${i}`);
    old.close();
    assert.throws(()=>openDb(path),(error:any)=>{
      assert.match(error.message,/first 5 of 6/);assert.ok(error.message.includes('"table":"memories"'));
      assert.ok(error.message.includes('"rowid":1'));assert.ok(error.message.includes('"parent":"decks"'));
      assert.ok(!error.message.includes('"rowid":6'));return true;
    });
    const check=new Database(path,{readOnly:true});
    try {assert.equal(check.prepare("SELECT value FROM meta WHERE key='schema_version'").get()!.value,"10");assert.equal(check.prepare("SELECT name FROM sqlite_master WHERE name='users'").get(),undefined);assert.equal(check.prepare("SELECT count(*) n FROM memories").get()!.n,6);}
    finally {check.close();}
  } finally {if(old.isOpen)old.close();rmSync(dir,{recursive:true,force:true});}
});

test("concurrent openers wait for the writer, re-read version under BEGIN IMMEDIATE, and migrate once",{timeout:15000},async()=>{
  const dir=mkdtempSync(join(tmpdir(),"concurrent-migration-")),path=join(dir,"old.db"),parent=v10(path);
  parent.exec("PRAGMA journal_mode=WAL; BEGIN IMMEDIATE");
  const children=Array.from({length:2},()=>{
    const code=`import {openDb} from ${JSON.stringify(pathToFileURL(join(PKG_ROOT,"src/db.ts")).href)};
      process.send('opening');const db=openDb(${JSON.stringify(path)});
      console.log(JSON.stringify(db.prepare("SELECT value FROM meta WHERE key='schema_version'").get()));db.close();`;
    const child=spawn(process.execPath,["--input-type=module","-e",code],{stdio:["ignore","pipe","pipe","ipc"]});
    let stdout="",stderr="";child.stdout!.on("data",c=>stdout+=c);child.stderr!.on("data",c=>stderr+=c);
    const started=new Promise<void>((resolve,reject)=>{child.once("message",()=>resolve());child.once("error",reject);child.once("exit",code=>{if(code!==0)reject(new Error(stderr));});});
    const done=new Promise<void>((resolve,reject)=>{child.once("error",reject);child.once("close",code=>code===0?resolve():reject(new Error(stderr)));});
    return {child,started,done,result:()=>JSON.parse(stdout)};
  });
  let released=false;
  try {
    await Promise.all(children.map(c=>c.started));await delay(100);
    assert.ok(children.every(c=>c.child.exitCode===null),"both openers must wait rather than fail while a writer holds the lock");
    parent.exec("COMMIT");released=true;
    await Promise.all(children.map(c=>c.done));
    for(const c of children)assert.deepEqual(c.result(),{value:"15"});
    const check=openDb(path);try {assert.equal(check.prepare("SELECT count(*) n FROM users").get()!.n,1);assert.deepEqual(check.prepare("PRAGMA foreign_key_check").all(),[]);}finally{check.close();}
  } finally {
    if(!released)parent.exec("ROLLBACK");parent.close();
    for(const c of children)if(c.child.exitCode===null)c.child.kill();
    await Promise.allSettled(children.map(c=>c.done));rmSync(dir,{recursive:true,force:true});
  }
});
