import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, copyFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync } from "node:child_process";
import { PKG_ROOT } from "../src/db.ts";
test("runtime check catches a hoisted jose mismatch and succeeds with the nested exact pin",()=>{
 const dir=mkdtempSync(join(tmpdir(),"runtime-layout-"));
 const app=join(dir,"packages/deck-builder");mkdirSync(join(app,"scripts"),{recursive:true});
 const script=join(app,"scripts/check-runtime.mjs");copyFileSync(join(PKG_ROOT,"scripts/check-runtime.mjs"),script);
 writeFileSync(join(app,"package.json"),JSON.stringify({type:"module",dependencies:{jose:"6.2.3"}}));
 writeFileSync(join(app,"runtime-imports.json"),JSON.stringify([{file:"src/auth.ts",name:"jose"}]));
 const pkg=(path:string,version:string)=>{mkdirSync(path,{recursive:true});writeFileSync(join(path,"package.json"),JSON.stringify({name:"jose",version,type:"module",exports:"./index.js"}));writeFileSync(join(path,"index.js"),"");};
 try{pkg(join(dir,"node_modules/jose"),"6.2.12");assert.throws(()=>execFileSync(process.execPath,[script],{stdio:"pipe"}),/Command failed/);pkg(join(app,"node_modules/jose"),"6.2.3");assert.equal(JSON.parse(execFileSync(process.execPath,[script],{encoding:"utf8"})).versions.jose,"6.2.3");}
 finally{rmSync(dir,{recursive:true,force:true});}
});
test("every actual source runtime import and exact package pin resolves",()=>{
 const dir=mkdtempSync(join(tmpdir(),"runtime-imports-")),manifest=join(dir,"imports.json"),script=join(PKG_ROOT,"scripts/check-runtime.mjs");
 try{execFileSync(process.execPath,[script,"--manifest",manifest]);const result=JSON.parse(execFileSync(process.execPath,[script,"--check",manifest],{encoding:"utf8"}));assert.ok(result.runtime_imports_checked>100);assert.equal(result.versions.jose,"6.2.3");}
 finally{rmSync(dir,{recursive:true,force:true});}
});
