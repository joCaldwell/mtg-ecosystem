import assert from "node:assert/strict";
import { test } from "node:test";
import ts from "typescript";
import { printedIdentity, printedSchema, renderPrinted } from "../../../scripts/generate-rust-ast.ts";

const schemaFrom = (text: string) =>
  printedSchema(ts.createSourceFile("probe.ts", text, ts.ScriptTarget.ES2022, true));

test("printed generation preserves all field presence declarations", () => {
  const schema = schemaFrom(`interface Probe {
    required: string;
    optional?: string;
    nullable: string | null;
    both?: string | null;
    forbidden?: never;
  }`);
  const output = renderPrinted({ schema, descriptor: JSON.stringify([...schema]), identity: printedIdentity() });
  for (const line of [
    'pub required: String,',
    'pub optional: Option<String>,',
    'pub nullable: Nullable<String>,',
    'pub both: Presence<String>,',
    'required: required(fields, "required", at)?,',
    'optional: optional(fields, "optional", at)?,',
    'nullable: nullable(fields, "nullable", at)?,',
    'both: presence(fields, "both", at)?,',
    'known_fields(fields, &["both", "nullable", "optional", "required"], at)?;',
  ]) assert.ok(output.includes(line), line);
  assert.ok(!output.includes("pub forbidden:"));
});

test("printed generation requires a closed, unambiguous discriminator", () => {
  assert.throws(() => schemaFrom('type Probe = { t: "one" } | { t: "one"; data: string }'),
    /no disjoint required string discriminator/);
  assert.throws(() => schemaFrom('type Probe = { t?: "one" } | { t: "two" }'),
    /no disjoint required string discriminator/);
  assert.throws(() => schemaFrom('type Probe = { required: never }'),
    /never is supported only as an optional forbidden property/);

  const first = schemaFrom('type Probe = { shared: string } & ({ t: "one" } | { t: "two"; data: number })');
  const second = schemaFrom('type Probe = ({ data: number; t: "two" } | { t: "one" }) & { shared: string }');
  assert.deepEqual([...first], [...second]);
});
