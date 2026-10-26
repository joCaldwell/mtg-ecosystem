import { test, mock } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { openDb, PKG_ROOT } from "../src/db.ts";
import { createJobs, runJob } from "../src/jobs.ts";

test("failed backup retries hourly and preserves last success/staleness over failures",async t=>{
 const now=Date.UTC(2026,9,26,12);t.mock.method(Date,"now",()=>now);
 const db=openDb(":memory:"),jobs=createJobs(db),set=db.prepare("insert into meta values(?,?) on conflict(key) do update set value=excluded.value");
 try{
  assert.equal(jobs.status().backup.stale,true);
  const success=new Date(now-3600000).toISOString();set.run("job:backup:last_success",success);
  set.run("job:backup",JSON.stringify({last_run:new Date(now).toISOString(),finished_at:null,result:"error"}));
  assert.equal(jobs.status().backup.last_success_at,success);assert.equal(jobs.status().backup.stale,false);assert.equal(Date.parse(jobs.status().backup.next_run),now+3600000);
  set.run("job:backup:last_success",new Date(now-37*3600000).toISOString());assert.equal(jobs.status().backup.stale,true);
  set.run("job:backup:last_success","invalid");assert.equal(jobs.status().backup.stale,true);
  await runJob(db,"backup",async()=>{});assert.ok(jobs.status().backup.last_success_at);assert.equal(jobs.status().backup.stale,false);
 }finally{await jobs.close();db.close();}
});
test("first failed run after upgrade preserves the legacy successful backup time",async()=>{
 const db=openDb(":memory:"),jobs=createJobs(db),previous=new Date(Date.now()-3600000).toISOString();
 try{db.prepare("insert into meta values(?,?)").run("job:backup",JSON.stringify({last_run:previous,finished_at:previous,result:"success"}));await assert.rejects(runJob(db,"backup",async()=>{throw Error("failure");}));assert.equal(jobs.status().backup.last_success_at,previous);assert.equal(jobs.status().backup.stale,false);}
 finally{await jobs.close();db.close();}
});
test("hosted parent forwards one redacted structured child failure, never arbitrary output",async()=>{
 const db=openDb(":memory:"),lines:string[]=[];const old=process.env.DECKBUILDER_MODE;process.env.DECKBUILDER_MODE="hosted";
 const originalWrite=process.stdout.write.bind(process.stdout);
 const capture=mock.method(process.stdout,"write",(chunk:any,...args:any[])=>{if(typeof chunk!=="string" || !chunk.startsWith('{"ts":'))return (originalWrite as any)(chunk,...args);lines.push(chunk);return true;});
 const record=JSON.stringify({event:"job_error",error:{name:"AccessDenied",code:"InvalidAccessKeyId",message:"SECRET_BODY",stack:"Error: SECRET_BODY\n    at backup (file:///app/backups.ts:1:1)"},body:"SECRET_BODY"});
 const jobs=createJobs(db,{spawnJob(){return spawn(process.execPath,["-e",`console.log('SECRET_BODY');console.log(${JSON.stringify(record)});console.log(${JSON.stringify(record)});process.exitCode=1`],{stdio:"pipe"});}});
 try{jobs.enqueue("backup");while(jobs.status().active)await new Promise(r=>setTimeout(r,10));const records=lines.filter(x=>x.startsWith('{"ts":')).map(x=>JSON.parse(x));const errors=records.filter(x=>x.event==="job_error");assert.equal(errors.length,1);assert.equal(errors[0].error.name,"AccessDenied");assert.equal(errors[0].error.code,"InvalidAccessKeyId");assert.match(errors[0].error.stack,/at backup/);assert.ok(!lines.join("").includes("SECRET_BODY"));}
 finally{await jobs.close();db.close();capture.mock.restore();if(old===undefined)delete process.env.DECKBUILDER_MODE;else process.env.DECKBUILDER_MODE=old;}
});
test("hosted CLI emits a safe diagnosable backup error",async()=>{
 const dir=mkdtempSync(join(tmpdir(),"cli-error-"));
 const env={...process.env,DECKBUILDER_MODE:"hosted",DECKBUILDER_DATA:dir,BACKUP_S3_ENDPOINT:"SECRET_URL",BACKUP_S3_BUCKET:"",BACKUP_S3_ACCESS_KEY:"",BACKUP_S3_SECRET_KEY:""};
 const child=spawn(process.execPath,[join(PKG_ROOT,"src/cli.ts"),"backup"],{env,stdio:"pipe"});let out="",err="";child.stdout.on("data",c=>out+=c);child.stderr.on("data",c=>err+=c);
 try{const [code]=await once(child,"exit");assert.equal(code,1);const record=out.trim().split("\n").map(line=>JSON.parse(line)).find(x=>x.event==="job_error");assert.equal(record.error.name,"Error");assert.match(record.error.stack,/at /);assert.ok(!out.includes("SECRET_URL"));assert.ok(!err.includes("SECRET_URL"));}
 finally{child.kill();rmSync(dir,{recursive:true,force:true});}
});
test("shutdown waits for an active child to exit before completing",async()=>{
 const db=openDb(":memory:");let exited=false;
 const jobs=createJobs(db,{spawnJob(){const child=spawn(process.execPath,["-e","setTimeout(()=>{},100)"],{stdio:"pipe"});child.once("exit",()=>{exited=true;});return child;}});
 try{jobs.enqueue("refresh");const closing=jobs.close();assert.equal(exited,false);await closing;assert.equal(exited,true);assert.equal(jobs.status().active,null);}
 finally{await jobs.close();db.close();}
});
