import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, readFileSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { backupDatabase } from "../src/backups.ts";
import { openDb } from "../src/db.ts";
const key=(date:string,time="00-00-00-000")=>`backups/deck-builder-${date}T${time}Z-00000000-0000-0000-0000-000000000000.db`;
test("unique bucket snapshots retain first daily/monthly recovery points and ignore archives",async()=>{
 const dir=mkdtempSync(join(tmpdir(),"backup-retention-")),db=openDb(":memory:"),now=new Date("2026-10-26T12:00:00Z");
 const objects=new Set<string>(["archive/pre-claim-deck-builder-2026-10-25.db","other-owner/file.db","backups/unrecognised.db","backups/deck-builder-2024-01-01.db",key("2027-01-01")]);
 for(let i=0;i<200;i++){const d=new Date(now);d.setUTCDate(d.getUTCDate()-i);const date=d.toISOString().slice(0,10);objects.add(key(date));objects.add(key(date,"06-00-00-000"));}
 const uploads:string[]=[],removed:string[]=[];
 const store={async upload(file:string,k:string){assert.ok(readFileSync(file).includes(Buffer.from("SQLite format 3")));assert.ok(!objects.has(k));uploads.push(k);objects.add(k);},async list(){return [...objects].map(Key=>({Key}));},async remove(keys:string[]){removed.push(...keys);keys.forEach(k=>objects.delete(k));},close(){}};
 try{
  const a=await backupDatabase(db,dir,now,store),b=await backupDatabase(db,dir,new Date("2026-10-26T12:00:01Z"),store);
  assert.notEqual(a.file,b.file);assert.notEqual(uploads[0],uploads[1]);
  for(const date of ["2026-10-13","2026-10-26","2026-05-01","2026-06-01","2026-07-01","2026-08-01","2026-09-01","2026-10-01"])assert.ok(objects.has(key(date)),`lost recovery point ${date}`);
  assert.ok(!objects.has(key("2026-10-12")));assert.ok(!objects.has(key("2026-04-30")));
  assert.ok(objects.has(uploads[1]));
  for(const k of ["archive/pre-claim-deck-builder-2026-10-25.db","other-owner/file.db","backups/unrecognised.db",key("2027-01-01")])assert.ok(objects.has(k));
  assert.ok(removed.length>100);assert.ok(!removed.some(k=>k.startsWith("archive/")));
 }finally{db.close();rmSync(dir,{recursive:true,force:true});}
});
for(const failure of ["upload","list"])test(`failed ${failure} never prunes remote recovery points`,async()=>{
 const dir=mkdtempSync(join(tmpdir(),"backup-failure-")),db=openDb(":memory:");let deletes=0,listed=0;
 try{await assert.rejects(backupDatabase(db,dir,new Date(),{async upload(){if(failure==="upload")throw Error("private signed URL");},async list(){listed++;throw Error("incomplete listing");},async remove(){deletes++;},close(){}}));assert.equal(deletes,0);assert.equal(listed,failure==="list"?1:0);assert.ok(readdirSync(join(dir,"backups")).some(file=>file.endsWith(".db")));}
 finally{db.close();rmSync(dir,{recursive:true,force:true});}
});
