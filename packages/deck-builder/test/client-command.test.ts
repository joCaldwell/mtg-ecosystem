import {test} from "node:test";
import assert from "node:assert/strict";
import ts from "typescript";
import {mkdtempSync,writeFileSync,readFileSync,symlinkSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {join,resolve} from "node:path";
import {pathToFileURL} from "node:url";
import {PKG_ROOT} from "../src/db.ts";
test("rendered Claude Code setup keeps a literal environment header instead of a token argument",async()=>{
 const dir=mkdtempSync(join(tmpdir(),"client-command-"));
 try{
  const source=readFileSync(join(PKG_ROOT,"web/src/features/settings/clients.tsx"),"utf8");
  const output=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext,jsx:ts.JsxEmit.ReactJSX}}).outputText;
  symlinkSync(resolve(PKG_ROOT,"../../node_modules"),join(dir,"node_modules"),"dir");
  const file=join(dir,"clients.mjs");writeFileSync(file,output);
  const {clientConfigs}=await import(pathToFileURL(file).href);
  const client=clientConfigs("https://decks.example/mcp","SECRET_CANARY",undefined).find((c:any)=>c.id==="claude-code");
  const command=client.parts.find((p:any)=>p.code)?.code;
  assert.ok(!command.includes("SECRET_CANARY"));assert.match(command,/read -rs DECKBUILDER_TOKEN/);
  const json=JSON.parse(command.match(/add-json deck-builder '(.*)'/)[1]);
  assert.equal(json.headers.Authorization,"Bearer ${DECKBUILDER_TOKEN}");assert.equal(json.url,"https://decks.example/mcp/claude-code");
 }finally{rmSync(dir,{recursive:true,force:true});}
});
