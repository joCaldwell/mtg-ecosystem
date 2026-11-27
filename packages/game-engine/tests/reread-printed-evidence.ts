// Test-only. Each stdin line supplies one detached source edit on a frozen row.
// Send freshly read legacy and printed bytes to the Rust verifier on stdout.
import { createHash } from "node:crypto";
import { createInterface } from "node:readline";
import {
  analyzeCardBindings,
  parseOracleTextWithPrintedEvidence,
} from "../../oracle-parser/src/index.ts";
import { resolveCard } from "../../oracle-parser/src/semantic/resolution.ts";

const digest = (value: unknown) => createHash("sha256").update(JSON.stringify(value)).digest("hex");

for await (const line of createInterface({ input: process.stdin })) {
  const { row, from, to } = JSON.parse(line);
  const source = row.source;
  if (source.card_faces || source.oracle_text.split(from).length !== 2) {
    throw new Error(`expected one exact substitution for ${source.name}`);
  }
  source.oracle_text = source.oracle_text.replace(from, () => to);
  const capture = parseOracleTextWithPrintedEvidence(source.oracle_text, source.name, {
    layout: source.layout,
    typeLine: source.type_line,
  });
  row.sourceSha256 = digest(source);
  row.faces[0].result = capture.result;
  row.printed.faces = [capture.printed];
  row.printedSha256 = digest(row.printed);
  row.status = capture.result.ok ? "parsed" : "unsupported";
  if (capture.result.ok) row.astSha256 = digest([capture.result.abilities]);
  else delete row.astSha256;

  const input = {
    identity: row.oracleIdentity,
    source,
    faces: [{ sourceFaceIndex: 0, result: capture.result }],
  };
  row.binding = analyzeCardBindings(input);
  row.bindingSha256 = digest(row.binding);
  row.resolution = resolveCard(input, row.binding);
  row.resolutionSha256 = digest(row.resolution);
  process.stdout.write(JSON.stringify(row) + "\n");
}
