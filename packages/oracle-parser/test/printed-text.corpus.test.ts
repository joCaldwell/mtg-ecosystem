import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { createInterface } from "node:readline";
import { test } from "node:test";
import type { ArchiveRecord } from "../../../scripts/archive-parser.ts";
import { buildCardNameCatalog, type CardNameSource } from "../src/card-names.ts";
import { normalizeOracleText, normalizeOracleTextTracked } from "../src/normalize.ts";
import { replayNormalizationEdits } from "../src/printed/replay.ts";
import { normalizeOracleText as legacyNormalize } from "./support/legacy-normalize.ts";

const archivePath = process.env.MTG_R5_ARCHIVE;
const archiveSha256 = "7400edf1925081db984d619c56ce17f3f004153f99a57160b5f2f08fd52c5303";
const normalizedSha256 = "ab0114310125eeb9268512e0a2346004fdcc4b696adf0074eee519816e7be550";

async function* readRows(path: string): AsyncGenerator<ArchiveRecord> {
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

test("tracked normalization reproduces the frozen R5 archive", { skip: !archivePath && "set MTG_R5_ARCHIVE" }, async (test) => {
  assert.ok(archivePath);
  const frozen = createHash("sha256");
  for await (const chunk of createReadStream(archivePath)) frozen.update(chunk);
  const observedArchiveSha256 = frozen.digest("hex");
  assert.equal(observedArchiveSha256, archiveSha256);

  // The archive writer builds the catalog before any scope or duplicate filtering.
  const sources: CardNameSource[] = [];
  for await (const row of readRows(archivePath)) {
    assert.equal(row.sourceIndex, sources.length);
    const { name, layout, oracle_id, card_faces } = row.source;
    sources.push({ name, layout, oracle_id, card_faces: card_faces?.map(({ name, oracle_id }) => ({ name, oracle_id })) });
  }
  assert.equal(sources.length, 38_721);
  const catalog = buildCardNameCatalog(sources);

  const digest = createHash("sha256");
  const kinds = { gap: 0, "self-name": 0, punct: 0, "literal-name": 0, pronoun: 0 };
  const counts = { attempted: 0, changed: 0, nonAscii: 0, edits: 0, maxEdits: 0, mismatches: 0, unmappable: 0, replayFailures: 0 };
  const failures: string[] = [];
  const recordFailure = (location: string, message: string) => {
    if (failures.length < 12) failures.push(`${location}: ${message}`);
  };

  for await (const row of readRows(archivePath)) {
    if (row.status === "excluded") continue;
    const faces = row.source.card_faces ?? [row.source];
    for (const face of row.faces) {
      if (face.status !== "attempted") continue;
      const { sourceFaceIndex } = face;
      const { oracle_text: original, name, type_line } = faces[sourceFaceIndex];
      const typeLine: string | undefined = type_line ?? row.source.type_line ?? undefined;
      assert.equal(typeof original, "string");
      const text = original!;
      const location = `source ${row.sourceIndex} face ${sourceFaceIndex} (${name})`;
      counts.attempted++;
      if (/[^\x00-\x7f]/u.test(text)) counts.nonAscii++;
      const traced = normalizeOracleTextTracked(text, name, catalog, typeLine);
      const normal = traced.lines.join("\n");
      digest.update(JSON.stringify([row.sourceIndex, sourceFaceIndex, normal]) + "\n");
      if (normal !== text) counts.changed++;
      const archived = face.result.lines.map(line => line.text).join("\n");
      const legacy = legacyNormalize(text, name, catalog, typeLine);
      const ordinary = normalizeOracleText(text, name, catalog, typeLine);
      if (normal !== archived || traced.lines.length !== legacy.length ||
          traced.lines.some((line, i) => line !== legacy[i]) || ordinary.length !== legacy.length ||
          ordinary.some((line, i) => line !== legacy[i])) {
        counts.mismatches++;
        recordFailure(location, "normalized lines differ from legacy or archive");
      }
      if (traced.status === "unmappable") {
        counts.unmappable++;
        recordFailure(location, traced.reason);
        continue;
      }
      counts.edits += traced.edits.length;
      counts.maxEdits = Math.max(counts.maxEdits, traced.edits.length);
      for (const edit of traced.edits) kinds[edit.edit]++;
      try {
        assert.equal(replayNormalizationEdits(text, traced, name, catalog, typeLine), normal);
      } catch (error) {
        counts.replayFailures++;
        recordFailure(location, String(error));
      }
    }
  }
  const observedDigest = digest.digest("hex");
  test.diagnostic(JSON.stringify({ archiveSha256: observedArchiveSha256, normalizedSha256: observedDigest, ...counts, kinds }));
  assert.deepEqual(failures, []);
  assert.deepEqual(counts, { attempted: 34_464, changed: 14_831, nonAscii: 3_891, edits: 19_118, maxEdits: 9, mismatches: 0, unmappable: 0, replayFailures: 0 });
  assert.deepEqual(kinds, { gap: 12_181, "self-name": 5_702, punct: 579, "literal-name": 438, pronoun: 218 });
  assert.equal(observedDigest, normalizedSha256);
});
