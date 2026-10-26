import {test,mock} from "node:test";
import assert from "node:assert/strict";
import {createServer} from "node:net";
import type {AddressInfo} from "node:net";
import {spawn} from "node:child_process";
import {once} from "node:events";
import {mkdtempSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {createApp} from "../src/app.ts";
import {testDb} from "./helpers.ts";
import {PKG_ROOT} from "../src/db.ts";
test("local 500 keeps a readable message and structured error name/code/frames",async()=>{
 const db=testDb(),lines:string[]=[],messages:string[]=[];
 const originalWrite=process.stdout.write.bind(process.stdout);
 const stdout=mock.method(process.stdout,"write",(chunk:any,...args:any[])=>{if(typeof chunk!=="string" || !chunk.startsWith('{"ts":'))return (originalWrite as any)(chunk,...args);lines.push(chunk);return true;});
 const stderr=mock.method(console,"error",(...args:any[])=>{messages.push(args.join(" "));});
 const app=createApp(db,{resolveUiActor(){throw Object.assign(new TypeError("Readable local failure"),{code:"EIO"});}});
 try{await new Promise<void>(r=>app.server.listen(0,"127.0.0.1",r));const response=await fetch(`http://127.0.0.1:${(app.server.address() as AddressInfo).port}/api/decks`);assert.equal(response.status,500);await response.text();assert.match(messages.join(""),/Readable local failure/);const record=lines.filter(x=>x.startsWith('{"ts":')).map(x=>JSON.parse(x)).find(x=>x.event==="request");assert.equal(record.error.name,"TypeError");assert.equal(record.error.code,"EIO");assert.match(record.error.stack,/at /);assert.equal(record.level,"error");}
 finally{await app.close();db.close();stdout.mock.restore();stderr.mock.restore();}
});
test("local port-in-use startup logs its name/code and readable message",async()=>{
 const dir=mkdtempSync(join(tmpdir(),"startup-error-")),socket=createServer();await new Promise<void>(r=>socket.listen(0,"127.0.0.1",r));
 const child=spawn(process.execPath,[join(PKG_ROOT,"src/server.ts")],{env:{...process.env,DECKBUILDER_MODE:"local",DECKBUILDER_DATA:dir,DECKBUILDER_PORT:String((socket.address() as AddressInfo).port)},stdio:"pipe"});let out="",err="";child.stdout.on("data",c=>out+=c);child.stderr.on("data",c=>err+=c);
 try{const [code]=await once(child,"exit");assert.equal(code,1);const record=out.trim().split("\n").map(x=>JSON.parse(x)).find(x=>x.event==="server_error");assert.equal(record.error.name,"Error");assert.equal(record.error.code,"EADDRINUSE");assert.match(err,/EADDRINUSE/);}
 finally{child.kill();await new Promise<void>(r=>socket.close(()=>r()));rmSync(dir,{recursive:true,force:true});}
});
