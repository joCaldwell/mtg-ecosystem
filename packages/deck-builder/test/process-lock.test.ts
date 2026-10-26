import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir, hostname } from "node:os";
import { join } from "node:path";
import { spawn, execFileSync, spawnSync } from "node:child_process";
import { once } from "node:events";
import { acquireProcessLock } from "../src/process-lock.ts";

for (const host of [hostname(), "retired-container"]) test(`reclaims stale own-PID lock from ${host}`, () => {
  const dir = mkdtempSync(join(tmpdir(), "process-lock-")), file = join(dir,"lock");
  try {
    writeFileSync(file, JSON.stringify({pid:process.pid,hostname:host,token:"old"}));
    const release=acquireProcessLock(file);
    assert.equal(JSON.parse(readFileSync(file,"utf8")).hostname,hostname());
    assert.throws(()=>acquireProcessLock(file),/running/);
    release();
  } finally { rmSync(dir,{recursive:true,force:true}); }
});
test("legacy stale PID 1 is reclaimed when the new server is PID 1",()=>{
  const dir=mkdtempSync(join(tmpdir(),"pid-one-")),file=join(dir,"lock");
  try {
    writeFileSync(file,JSON.stringify({pid:1,token:"old"}));
    const module=new URL("../src/process-lock.ts",import.meta.url).href;
    execFileSync(process.execPath,["--input-type=module","-e",`Object.defineProperty(process,'pid',{value:1});const {acquireProcessLock}=await import(${JSON.stringify(module)});acquireProcessLock(${JSON.stringify(file)})();`]);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
test("foreign-host lock is reclaimed even if its PID exists locally",()=>{
  const dir=mkdtempSync(join(tmpdir(),"foreign-host-")),file=join(dir,"lock");
  try {writeFileSync(file,JSON.stringify({pid:process.ppid,hostname:"other-container",token:"old"}));acquireProcessLock(file)();}
  finally{rmSync(dir,{recursive:true,force:true});}
});
test("same-host live process retains exclusion; SIGKILL leaves a recoverable lock",async()=>{
  const dir=mkdtempSync(join(tmpdir(),"live-lock-")),file=join(dir,"lock");
  const module=new URL("../src/process-lock.ts",import.meta.url).href;
  const child=spawn(process.execPath,["--input-type=module","-e",`const {acquireProcessLock}=await import(${JSON.stringify(module)});acquireProcessLock(${JSON.stringify(file)});console.log('ready');setInterval(()=>{},1000)`],{stdio:["ignore","pipe","pipe"]});
  try{await once(child.stdout!,"data");assert.throws(()=>acquireProcessLock(file),/running/);const exit=once(child,"exit");child.kill("SIGKILL");await exit;acquireProcessLock(file)();}
  finally{child.kill();rmSync(dir,{recursive:true,force:true});}
});
test("real PID namespace: force-killed PID 1 leaves a lock that the next PID 1 reclaims",async t=>{
  const prefix=["--user","--map-root-user","--pid","--fork"];
  if(process.platform!=="linux" || spawnSync("unshare",[...prefix,process.execPath,"-e","process.exit(0)"],{stdio:"pipe"}).status!==0){t.skip("User PID namespaces unavailable; portable PID-1 regression still runs");return;}
  const dir=mkdtempSync(join(tmpdir(),"namespace-lock-")),file=join(dir,"lock"),module=new URL("../src/process-lock.ts",import.meta.url).href;
  const code=`const {acquireProcessLock}=await import(${JSON.stringify(module)});if(process.pid!==1)throw Error('Not PID 1');acquireProcessLock(${JSON.stringify(file)});console.log('ready');setInterval(()=>{},1000)`;
  const child=spawn("unshare",[...prefix,process.execPath,"--input-type=module","-e",code],{stdio:["ignore","pipe","pipe"]});
  try{
    await once(child.stdout!,"data");assert.equal(JSON.parse(readFileSync(file,"utf8")).pid,1);
    const hostPid=Number(readFileSync(`/proc/${child.pid}/task/${child.pid}/children`,"utf8").trim());assert.ok(hostPid>1);
    const exit=once(child,"exit");process.kill(hostPid,"SIGKILL");await exit;
    assert.equal(JSON.parse(readFileSync(file,"utf8")).pid,1);
    execFileSync("unshare",[...prefix,process.execPath,"--input-type=module","-e",`const {acquireProcessLock}=await import(${JSON.stringify(module)});if(process.pid!==1)throw Error('Not PID 1');acquireProcessLock(${JSON.stringify(file)})();`],{stdio:"pipe"});
  }finally{child.kill();rmSync(dir,{recursive:true,force:true});}
});
