import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { readJsonArray } from "../src/json-array.ts";

test("incremental arrays handle nested objects, escaped quotes and UTF-8 across chunks", async () => {
  const dir = mkdtempSync(join(tmpdir(), "json-array-")), file = join(dir, "bulk.json");
  try {
    const values = [{ text: '"\\}][é'.repeat(20_000), nested: [{ value: [1, 2] }] }, { text: "done" }];
    writeFileSync(file, JSON.stringify(values));
    const read = []; for await (const value of readJsonArray(file)) read.push(value);
    assert.deepEqual(read, values);
    for (const invalid of ['[{"a":1},]', '[{},{}', '["not an object"]', '[{}] trailing']) {
      writeFileSync(file, invalid);
      await assert.rejects(async () => { for await (const _value of readJsonArray(file)) { /* exhaust validation */ } });
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
