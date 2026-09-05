import { test } from "node:test";
import assert from "node:assert/strict";
import { Readable } from "node:stream";
import { jsonlToJsonArray } from "../src/scryfall.ts";

test("converts Scryfall JSONL into the shared cached JSON-array shape", async () => {
  const chunks: Buffer[] = [];
  const output = Readable.from([
    '{"oracle_id":"one"}\n{"oracle_',
    'id":"two"}\n\n',
  ]).pipe(jsonlToJsonArray());

  for await (const chunk of output) chunks.push(Buffer.from(chunk));

  assert.deepEqual(JSON.parse(Buffer.concat(chunks).toString("utf8")), [
    { oracle_id: "one" },
    { oracle_id: "two" },
  ]);
});
