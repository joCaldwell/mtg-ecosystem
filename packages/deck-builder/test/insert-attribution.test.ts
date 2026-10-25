import { test } from "node:test";
import assert from "node:assert/strict";
import { globSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { PKG_ROOT } from "../src/db.ts";
import { testDb } from "./helpers.ts";

test("every runtime INSERT names tenant/attribution columns rather than inheriting user-1 defaults",()=>{
  const db=testDb();
  try {
    const required=new Map<string,string[]>();
    for(const {name} of db.prepare("SELECT name FROM sqlite_master WHERE type='table'").all() as {name:string}[]) {
      const cols=db.prepare(`PRAGMA table_info("${name}")`).all() as {name:string}[];
      const names=cols.map(c=>c.name).filter(n=>["user_id","author_user_id","author_via","author_label","actor_user_id","actor_via","actor_label"].includes(n));
      if(names.length)required.set(name,names);
    }
    const seen=new Set<string>();
    for(const file of globSync("**/*.ts",{cwd:join(PKG_ROOT,"src")})) {
      let source=readFileSync(join(PKG_ROOT,"src",file),"utf8");
      // Historical migration INSERTs intentionally backfill via their defaults.
      // Runtime settings in this same file are still checked.
      if(file==="db.ts")source=source.slice(source.indexOf("// ---------- settings"));
      for(const match of source.matchAll(/INSERT\s+(?:OR\s+\w+\s+)?INTO\s+(\w+)\s*(?:\(([^)]+)\))?/gi)) {
        const columns=required.get(match[1]);if(!columns)continue;
        seen.add(match[1]);const named=(match[2]??"").split(",").map(n=>n.trim());
        for(const column of columns)assert.ok(named.includes(column),`${file}: INSERT ${match[1]} must explicitly name ${column}`);
      }
    }
    assert.deepEqual([...seen].sort(),[...required.keys()].sort(),"every tenant table has a checked runtime insertion path");
  } finally {db.close();}
});
