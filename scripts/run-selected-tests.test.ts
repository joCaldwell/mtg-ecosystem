import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const runner = fileURLToPath(new URL("./run-selected-tests.ts", import.meta.url));
function check(source: string, pattern = "^selected$") {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "engine-test-selection-"));
  try {
    const file = path.join(directory, "fixture.mjs");
    fs.writeFileSync(file, source);
    const env = { ...process.env };
    delete env.NODE_TEST_CONTEXT;
    return spawnSync(process.execPath, [runner, "--name", pattern, file], { encoding: "utf8", env });
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
}
test("selected tests execute and real failures propagate", () => {
  assert.equal(check("import {test} from 'node:test';test('selected',()=>{});").status, 0);
  assert.notEqual(check("import {test} from 'node:test';test('selected',()=>{throw Error('real failure')});").status, 0);
});
test("renamed, empty, skipped and TODO-only selections cannot report a pass", () => {
  for (const source of [
    "import {test} from 'node:test';test('renamed',()=>{});",
    "",
    "import {test} from 'node:test';test.skip('selected',()=>{});",
    "import {test} from 'node:test';test.todo('selected');",
  ]) {
    const result = check(source);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /No runnable tests matched/);
  }
});
test("selected-test import errors and invalid selectors fail", () => {
  assert.notEqual(check("throw Error('bad import');").status, 0);
  assert.notEqual(check("", "[").status, 0);
});
