import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import { parseOracleText } from "../../oracle-parser/src/index.ts";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const fixtureDir = new URL("fixtures/", import.meta.url);
const sha = (value: string) => createHash("sha256").update(value).digest("hex");
const read = (name: string) => fs.readFileSync(new URL(name, fixtureDir), "utf8");
function run(command: string) {
  const result = spawnSync("cargo", ["run", "--quiet", "--locked", "-p", "mtg-sim", "--", command], { cwd: root, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

test("retained archive hashes agree with JavaScript and source parser", () => {
  const provenance = JSON.parse(read("provenance.json"));
  assert.equal(sha(read("archive-manifest.json").trimEnd()), provenance.archive_manifest_sha256);
  for (const witness of provenance.records) {
    const raw = read(witness.file);
    const record = JSON.parse(raw);
    assert.equal(sha(raw.trimEnd()), witness.record_sha256);
    assert.equal(sha(JSON.stringify(record.source)), record.sourceSha256);
    assert.equal(sha(JSON.stringify(record.binding)), record.bindingSha256);
    assert.equal(sha(JSON.stringify(record.faces.map((face: any) => face.result.abilities))), record.astSha256);
    const parsed = parseOracleText(record.source.oracle_text, record.source.name, {
      layout: record.source.layout, typeLine: record.source.type_line, includeSourceProvenance: true,
    });
    assert.equal(parsed.ok, true, record.source.name);
    // Archive interchange is JSON: optional undefined TS properties are absent.
    if (parsed.ok) assert.deepEqual(JSON.parse(JSON.stringify(parsed.abilities)), record.faces[0].result.abilities, record.source.name);
  }
});

test("Rust lowering matches reviewed IR and its generated JSON Schema", () => {
  const catalog = run("compile");
  assert.deepEqual(catalog, JSON.parse(read("expected-ir.json")));
  const schema = run("schema");
  const validate = new Ajv2020({ strict: false, formats: {
    uint32: { type: "number", validate: (n: number) => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff },
  } }).compile(schema);
  assert.equal(validate(catalog), true, JSON.stringify(validate.errors));
  for (const mutate of [
    (c: any) => { c.unknownSemanticField = true; },
    (c: any) => { c.schema_version = 2; },
    (c: any) => { c.definitions[0].characteristics.power = 2; },
    (c: any) => { c.definitions[0].characteristics.power = "02"; },
    (c: any) => { c.definitions.find((d: any) => d.spell).spell.instructions[0].op = "execute_text"; },
    (c: any) => { c.definitions.find((d: any) => d.spell).spell.instructions[0].extra = true; },
  ]) {
    const bad = structuredClone(catalog); mutate(bad);
    assert.equal(validate(bad), false);
  }
});
