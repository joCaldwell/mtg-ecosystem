import assert from "node:assert/strict";
import { once } from "node:events";
import { printedIdentity } from "../../../../scripts/generate-rust-ast.ts";
import { parseOracleTextWithPrintedEvidence } from "../../src/index.ts";
import type { PrintedCard, PrintedFace } from "../../src/printed/schema.ts";
import { frozenR5, readRows } from "./frozen-r5.ts";

// Test protocol for the Rust decoder witness. Only the sidecar leaves this
// process, so the Rust test sees the producer's exact JSON for that value.
const archive = process.argv[2];
assert.ok(archive, "a frozen R5 archive is required");
const { catalog } = await frozenR5(archive);
const witnesses = new Set(["Boseiju, Who Shelters All", "Delighted Halfling", "Junk Diver",
  "Terastodon", "Secret Rendezvous", "Wedgelight Rammer"]);

async function output(value: unknown) {
  if (!process.stdout.write(JSON.stringify(value) + "\n")) await once(process.stdout, "drain");
}

await output(printedIdentity());
for await (const row of readRows(archive)) {
  if (row.status === "excluded" || (row.sourceIndex % 127 !== 0 && !witnesses.has(row.source.name))) continue;
  const sources = row.source.card_faces ?? [row.source];
  const faces = row.faces.map(({ sourceFaceIndex }): PrintedFace => {
    const source = sources[sourceFaceIndex];
    const text = source.oracle_text;
    if (text == null) return { sourceFaceIndex, status: "missing-oracle-text", field: text === null ? "null" : "absent" };
    return parseOracleTextWithPrintedEvidence(text, source.name, {
      cardNames: catalog, layout: row.source.layout, typeLine: source.type_line ?? row.source.type_line ?? undefined,
    }, sourceFaceIndex).printed;
  });
  const printed: PrintedCard = { artifactKind: "oracle-printed-evidence", schemaVersion: 1, faces };
  await output({ sourceIndex: row.sourceIndex, name: row.source.name, printed });
}
