import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { captureJsonOutput } from "./capture-json-output.ts";

test("retention JSON capture streams large stdout and cleans every failure path", () => {
  const temporaryRoot = fs.mkdtempSync(path.join(os.tmpdir(), "mtg-capture-test-"));
  const options = { temporaryRoot };
  try {
    const result = captureJsonOutput<{ payload: string; complete: boolean }>(process.execPath,
      ["-e", "process.stdout.write(JSON.stringify({payload:'x'.repeat(20*1024*1024),complete:true}))"], options);
    assert.equal(result.payload.length,20*1024*1024);assert.equal(result.complete,true);
    assert.deepEqual(fs.readdirSync(temporaryRoot),[]);
    assert.throws(() => captureJsonOutput(process.execPath,
      ["-e", "process.stdout.write(JSON.stringify({validButFailed:true}));process.exitCode=7"], options),/failed: 7/);
    assert.deepEqual(fs.readdirSync(temporaryRoot),[]);
    assert.throws(() => captureJsonOutput(process.execPath,["-e", "process.stdout.write('{invalid json')"], options),SyntaxError);
    assert.deepEqual(fs.readdirSync(temporaryRoot),[]);
    assert.throws(() => captureJsonOutput(path.join(temporaryRoot,"missing-executable"),[],options),/could not run/);
    assert.deepEqual(fs.readdirSync(temporaryRoot),[]);
    assert.throws(() => captureJsonOutput(process.execPath,["-e", "process.kill(process.pid,'SIGTERM')"],options),/failed: SIGTERM/);
    assert.deepEqual(fs.readdirSync(temporaryRoot),[]);
  } finally { fs.rmSync(temporaryRoot,{recursive:true,force:true}); }
});

