import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, readFileSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { backupDatabase } from "../src/backups.ts";
import { openDb } from "../src/db.ts";
const key=(date:string,time="00-00-00-000")=>`backups/deck-builder-${date}T${time}Z-00000000-0000-0000-0000-000000000000.db`;
test("unique bucket snapshots retain newest daily/monthly recovery points and ignore archives",async()=>{
 const dir=mkdtempSync(join(tmpdir(),"backup-retention-")),db=openDb(":memory:"),now=new Date("2026-10-26T12:00:00Z");
 const objects=new Set<string>(["archive/pre-claim-deck-builder-2026-10-25.db","other-owner/file.db","backups/unrecognised.db","backups/deck-builder-2024-01-01.db",key("2027-01-01")]);
 for(let i=0;i<200;i++){const d=new Date(now);d.setUTCDate(d.getUTCDate()-i);const date=d.toISOString().slice(0,10);objects.add(key(date));objects.add(key(date,"06-00-00-000"));}
 const uploads:string[]=[],removed:string[]=[];
 const store={async upload(file:string,k:string){assert.ok(readFileSync(file).includes(Buffer.from("SQLite format 3")));assert.ok(!objects.has(k));uploads.push(k);objects.add(k);},async list(){return [...objects].map(Key=>({Key}));},async remove(keys:string[]){removed.push(...keys);keys.forEach(k=>objects.delete(k));},close(){}};
 try{
  const a=await backupDatabase(db,dir,now,store);
  assert.ok(objects.has(uploads[0]));
  const b=await backupDatabase(db,dir,new Date("2026-10-26T12:00:01Z"),store);
  assert.notEqual(a.file,b.file);assert.notEqual(uploads[0],uploads[1]);
  for(const date of ["2026-10-13","2026-10-25","2026-05-31","2026-06-30","2026-07-31","2026-08-31","2026-09-30"])assert.ok(objects.has(key(date,"06-00-00-000")),`lost newest recovery point ${date}`);
  assert.ok(!objects.has(key("2026-10-13")));assert.ok(!objects.has(key("2026-05-01")));
  assert.ok(!objects.has(uploads[0]));
  assert.ok(!objects.has(key("2026-10-12")));assert.ok(!objects.has(key("2026-04-30")));
  assert.ok(objects.has(uploads[1]));
  for(const k of ["archive/pre-claim-deck-builder-2026-10-25.db","other-owner/file.db","backups/unrecognised.db",key("2027-01-01")])assert.ok(objects.has(k));
  assert.ok(removed.length>100);assert.ok(!removed.some(k=>k.startsWith("archive/")));
 }finally{db.close();rmSync(dir,{recursive:true,force:true});}
});
test("later same-day timestamp beats a date-only key, and this upload survives even a newer key",async()=>{
 const dir=mkdtempSync(join(tmpdir(),"backup-newest-")),db=openDb(":memory:");
 const legacy="backups/deck-builder-2026-10-25.db",later=key("2026-10-25","18-00-00-000"),newer=key("2026-10-26","18-00-00-000");
 const objects=new Set([legacy,later,newer]);let uploaded="";
 try{
  await backupDatabase(db,dir,new Date("2026-10-26T12:00:00Z"),{async upload(_file,k){uploaded=k;objects.add(k);},async list(){return [...objects].map(Key=>({Key}));},async remove(keys){keys.forEach(k=>objects.delete(k));},close(){}});
  assert.ok(!objects.has(legacy));assert.ok(objects.has(later));assert.ok(objects.has(newer));assert.ok(objects.has(uploaded));
 }finally{db.close();rmSync(dir,{recursive:true,force:true});}
});
test("S3 retention lists only backups/ and never lists or deletes archive objects",async()=>{
 const dir=mkdtempSync(join(tmpdir(),"backup-namespace-")),db=openDb(":memory:");
 const archive="archive/pre-promotion/deck-builder-2026-10-26T12-00-00-000Z-example.db",expired="backups/deck-builder-2024-01-01.db";
 const objects=new Set([archive,expired]),listed:string[]=[],deleted:string[]=[];let uploaded="";
 const server=createServer((req,res)=>{
  const url=new URL(req.url!,"http://localhost"),chunks:Buffer[]=[];
  req.on("data",chunk=>chunks.push(chunk));
  req.on("end",()=>{
   res.writeHead(200,{"Content-Type":"application/xml",ETag:'"test"'});
   if(req.method==="PUT"){uploaded=decodeURIComponent(url.pathname.slice("/test-bucket/".length));objects.add(uploaded);res.end();}
   else if(req.method==="GET"){
    const prefix=url.searchParams.get("prefix")!;listed.push(prefix);
    res.end(`<ListBucketResult><IsTruncated>false</IsTruncated>${[...objects].filter(k=>k.startsWith(prefix)).map(k=>`<Contents><Key>${k}</Key></Contents>`).join("")}</ListBucketResult>`);
   }else if(req.method==="POST"){
    for(const match of Buffer.concat(chunks).toString().matchAll(/<Key>(.*?)<\/Key>/g)){deleted.push(match[1]);objects.delete(match[1]);}
    res.end("<DeleteResult/>");
   }else res.end();
  });
 });
 await new Promise<void>(resolve=>server.listen(0,"127.0.0.1",resolve));
 const env={BACKUP_S3_ENDPOINT:`http://127.0.0.1:${(server.address() as AddressInfo).port}`,BACKUP_S3_BUCKET:"test-bucket",BACKUP_S3_ACCESS_KEY:"test-access",BACKUP_S3_SECRET_KEY:"test-secret"},saved=new Map(Object.keys(env).map(k=>[k,process.env[k]]));
 try{
  Object.assign(process.env,env);await backupDatabase(db,dir,new Date("2026-10-26T12:00:00Z"));
  assert.deepEqual(listed,["backups/"]);assert.deepEqual(deleted,[expired]);assert.ok(objects.has(archive));assert.ok(objects.has(uploaded));
 }finally{
  for(const [k,v] of saved)if(v===undefined)delete process.env[k];else process.env[k]=v;
  db.close();await new Promise<void>(resolve=>server.close(()=>resolve()));rmSync(dir,{recursive:true,force:true});
 }
});
for(const failure of ["upload","list"])test(`failed ${failure} never prunes remote recovery points`,async()=>{
 const dir=mkdtempSync(join(tmpdir(),"backup-failure-")),db=openDb(":memory:");let deletes=0,listed=0;
 try{await assert.rejects(backupDatabase(db,dir,new Date(),{async upload(){if(failure==="upload")throw Error("private signed URL");},async list(){listed++;throw Error("incomplete listing");},async remove(){deletes++;},close(){}}));assert.equal(deletes,0);assert.equal(listed,failure==="list"?1:0);assert.ok(readdirSync(join(dir,"backups")).some(file=>file.endsWith(".db")));}
 finally{db.close();rmSync(dir,{recursive:true,force:true});}
});
