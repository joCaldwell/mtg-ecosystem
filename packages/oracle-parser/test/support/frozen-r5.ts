import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { createInterface } from "node:readline";
import type { ArchiveRecord } from "../../../../scripts/archive-parser.ts";
import { buildCardNameCatalog, type CardNameSource } from "../../src/card-names.ts";

const archiveSha256 = "7400edf1925081db984d619c56ce17f3f004153f99a57160b5f2f08fd52c5303";

export async function* readRows(path: string): AsyncGenerator<ArchiveRecord> {
  const stream = createReadStream(path, { encoding: "utf8" });
  const lines = createInterface({ input: stream, crlfDelay: Infinity });
  let header = true;
  try {
    for await (const line of lines) {
      if (header) {
        const manifest = JSON.parse(line);
        assert.equal(manifest.recordType, "manifest");
        assert.equal(manifest.schemaVersion, 4);
        header = false;
      } else {
        yield JSON.parse(line) as ArchiveRecord;
      }
    }
  } finally {
    lines.close();
    stream.destroy();
  }
}

export async function frozenR5(path: string) {
  const frozen = createHash("sha256");
  for await (const chunk of createReadStream(path)) frozen.update(chunk);
  const observedArchiveSha256 = frozen.digest("hex");
  assert.equal(observedArchiveSha256, archiveSha256);

  // The archive writer builds the catalog before any scope or duplicate filtering.
  const sources: CardNameSource[] = [];
  for await (const row of readRows(path)) {
    assert.equal(row.sourceIndex, sources.length);
    const { name, layout, oracle_id, card_faces } = row.source;
    sources.push({ name, layout, oracle_id, card_faces: card_faces?.map(({ name, oracle_id }) => ({ name, oracle_id })) });
  }
  assert.equal(sources.length, 38_721);
  return { catalog: buildCardNameCatalog(sources), observedArchiveSha256 };
}
