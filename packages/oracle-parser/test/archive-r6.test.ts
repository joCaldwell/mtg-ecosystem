import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, type TestContext } from "node:test";
import { fileURLToPath } from "node:url";
import { addResolutionSummary, emptyResolutionSummary, prepareArchive } from "../../../scripts/archive-parser.ts";
import { serializeR6Row, verifyR6Archive } from "../../../scripts/archive-r6.ts";
import { sha256 } from "../../../scripts/validate-parser.ts";
import { paperEvidenceFixture } from "./paper-evidence-fixture.ts";

const fixture = (name: string) => fileURLToPath(new URL(`./fixtures/archive-r6/${name}.jsonl`, import.meta.url));
const baseline = readFileSync(fixture("baseline"), "utf8").trimEnd().split("\n");
const candidate = readFileSync(fixture("candidate"), "utf8").trimEnd().split("\n");
const pin = {
  bytes: 6623,
  sha256: "5771eb09f21b8c136e20f10252643bb9723006d937037f5917914c0d574e39a8",
  headerSha256: sha256(baseline[0]),
};
const jsonl = (lines: string[]) => lines.join("\n") + "\n";
const replaceLine = (index: number, replacement: string) => jsonl(candidate.map((line, i) => i === index ? replacement : line));
const printedStart = candidate[1].indexOf(',"printed":');
const printedHashStart = candidate[1].lastIndexOf(',"printedSha256":');
const printedField = candidate[1].slice(printedStart, printedHashStart);
const printedHashField = candidate[1].slice(printedHashStart, -1);

async function temporary(t: TestContext) {
  const directory = await mkdtemp(join(tmpdir(), "oracle-archive-r6-test-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  return directory;
}

test("R6 golden archive preserves legacy bytes and pins its own ordered evidence", async t => {
  const cacheFile = join(await temporary(t), "source.json");
  await writeFile(cacheFile, JSON.stringify(baseline.slice(1).map(line => JSON.parse(line).source)));
  const archive = prepareArchive({
    cacheFile, paperEvidence: paperEvidenceFixture(),
    fingerprint: () => ({ revision: "fixture", sourceSha256: "a".repeat(64) }),
  });
  const output = Array.from(archive.records(true), ({ record, printed }) => serializeR6Row(record, printed).line);
  assert.equal(Buffer.concat(output).toString("utf8"), jsonl(candidate.slice(1)));
  assert.equal(JSON.stringify(archive.manifest), baseline[0]);

  const result = await verifyR6Archive(fixture("baseline"), fixture("candidate"), pin);
  assert.equal(result.fileSha256, "135fa82feb2dfab9ae9d09020bd8e7ccfee6d14f61eb44ce8b8f8c1bff041198");
  assert.equal(result.manifest.printed.recordsSha256, "d7db7f8952512833977ea89656a679d24bc4600e15b939ca54bf3d4eab6b47f3");
  assert.equal(result.bytes, 8872);
  assert.equal(result.lines, 4);
  assert.equal(result.rows, 3);
  assert.deepEqual(result.largestLine, { bytes: 3394, line: 2, sourceIndex: 0, name: "Élan" });
  assert.deepEqual(result.manifest.printedSummary.faces, { read: 2, "missing-oracle-text": 2, unmappable: 0 });
});

test("D0 rejects the first row outside the frozen transition", async t => {
  const file = join(await temporary(t), "candidate.jsonl");
  const variants: [string, string, RegExp][] = [
    ["reordered row", jsonl([candidate[0], candidate[2], candidate[1], candidate[3]]), /D0 line 2: source order/],
    ["modified exclusion", jsonl(candidate.map((line, i) => i === 2 ? line.replace("layout:token", "new-reason") : line)),
      /D0 line 3: excluded row differs/],
    ["extra appended field", jsonl(candidate.map((line, i) => i === 1 ? line.slice(0, -1) + ',"extra":null}' : line)),
      /D0 line 2: expected exactly printed and printedSha256/],
    ["missing printed hash", replaceLine(1, candidate[1].slice(0, printedHashStart) + "}"),
      /D0 line 2: expected exactly printed and printedSha256/],
    ["missing printed value", replaceLine(1, candidate[1].slice(0, printedStart) + printedHashField + "}"),
      /D0 line 2: expected exactly printed and printedSha256/],
    ["reversed suffix fields", replaceLine(1, candidate[1].slice(0, printedStart) + printedHashField + printedField + "}"),
      /D0 line 2: expected exactly printed and printedSha256/],
    ["modified legacy prefix", jsonl(candidate.map((line, i) => i === 1 ? line.replace("Élan", "Elan") : line)),
      /D0 line 2: legacy row prefix differs/],
    ["legacy whitespace", replaceLine(1, candidate[1].replace('"source":{', '"source":{ ')),
      /D0 line 2: legacy row prefix differs/],
    ["legacy key order", replaceLine(1, candidate[1].replace('"id":"a","oracle_id":"a"', '"oracle_id":"a","id":"a"')),
      /D0 line 2: legacy row prefix differs/],
    ["modified printed preimage", jsonl(candidate.map((line, i) => i === 3 ? line.replace('"field":"null"', '"field":"absent"') : line)),
      /D0 line 4: printed hash mismatch/],
    ["truncated archive", jsonl(candidate.slice(0, -1)), /D0 line 4: candidate ended before the last pinned R5 row/],
    ["extra row", jsonl([...candidate, candidate.at(-1)!]), /D0 line 5: candidate has a row beyond R5/],
    ["missing LF", candidate.join("\n"), /line 4: missing LF terminator/],
  ];
  for (const [description, data, error] of variants) {
    assert.notEqual(data, jsonl(candidate), description);
    await writeFile(file, data);
    await assert.rejects(verifyR6Archive(fixture("baseline"), file, pin), error, description);
  }
});

test("D0 requires the exact schema-5 header fields and writer order", async t => {
  const file = join(await temporary(t), "candidate.jsonl");
  const header = JSON.parse(candidate[0]);
  const { cut, ...withoutCut } = header;
  const { recordsSha256, ...printed } = header.printed;
  const { read, ...otherFaceCounts } = header.printedSummary.faces;
  const variants: [string, object][] = [
    ["cut first", { cut, ...withoutCut }],
    ["extra printed revision", { ...header, printed: { ...header.printed, revision: "extra" } }],
    ["extra legacy timestamp", { ...header, legacy: { ...header.legacy, timestamp: "extra" } }],
    ["extra name catalog field", { ...header, nameCatalog: { ...header.nameCatalog, extra: 0 } }],
    ["reordered printed fields", { ...header, printed: { recordsSha256, ...printed } }],
    ["missing summary count", { ...header, printedSummary: { ...header.printedSummary, faces: otherFaceCounts } }],
    ["reordered summary counts", { ...header, printedSummary: { ...header.printedSummary, faces: { ...otherFaceCounts, read } } }],
    ["extra summary count", { ...header, printedSummary: {
      ...header.printedSummary, faces: { ...header.printedSummary.faces, extra: 0 },
    } }],
  ];
  for (const [description, value] of variants) {
    const line = JSON.stringify(value);
    assert.notEqual(line, candidate[0], description);
    await writeFile(file, replaceLine(0, line));
    await assert.rejects(verifyR6Archive(fixture("baseline"), file, pin), /D0 line 1: .*header.*fields/, description);
  }
});

test("D0 rejects an incorrect ordered printed digest", async t => {
  const file = join(await temporary(t), "candidate.jsonl");
  const header = JSON.parse(candidate[0]);
  header.printed.recordsSha256 = "0".repeat(64);
  await writeFile(file, replaceLine(0, JSON.stringify(header)));
  await assert.rejects(verifyR6Archive(fixture("baseline"), file, pin), /D0 line 1: ordered printed digest/);
});

test("D0 requires compact printed bytes in writer order even when both hashes agree", async t => {
  const file = join(await temporary(t), "candidate.jsonl");
  const printed = printedField.slice(',"printed":'.length);
  const variants: [string, string, RegExp][] = [
    ["whitespace", "{ " + printed.slice(1), /D0 line 2: printed value is not compact canonical JSON/],
    ["reordered envelope", printed.replace(
      '"artifactKind":"oracle-printed-evidence","schemaVersion":1', '"schemaVersion":1,"artifactKind":"oracle-printed-evidence"'),
      /D0 line 2: printed field order differs from the writer/],
    ["reordered token", printed.replace('"t":"word","grammarIndex":0', '"grammarIndex":0,"t":"word"'),
      /D0 line 2: printed field order differs from the writer/],
  ];
  for (const [description, edited, error] of variants) {
    assert.notEqual(edited, printed, description);
    assert.deepEqual(JSON.parse(edited), JSON.parse(printed), description);
    const firstRow = candidate[1].slice(0, printedStart) + `,"printed":${edited},"printedSha256":"${sha256(edited)}"}`;
    const rows = [firstRow, ...candidate.slice(2)];
    const header = JSON.parse(candidate[0]);
    header.printed.recordsSha256 = sha256(rows.map(line => {
      const { sourceIndex, oracleIdentity, printedSha256 } = JSON.parse(line);
      return `${sourceIndex}\t${oracleIdentity}\t${printedSha256 ?? "excluded"}\n`;
    }).join(""));
    await writeFile(file, jsonl([JSON.stringify(header), ...rows]));
    await assert.rejects(verifyR6Archive(fixture("baseline"), file, pin), error, description);
  }
});

test("D0 checks legacy hash preimages even when the prefixes agree", async t => {
  const directory = await temporary(t);
  const oldFile = join(directory, "baseline.jsonl");
  const newFile = join(directory, "candidate.jsonl");
  const variants = [
    ["source", '"oracle_text":"Flying"', '"oracle_text":"Lifelink"'],
    ["AST", '"keyword":"flying"', '"keyword":"lifelink"'],
    ["binding", '"identity":"a","rules":', '"identity":"b","rules":'],
    ["resolution", '"residuals":[],"complete":true', '"residuals":[],"complete":false'],
  ];
  for (const [section, from, to] of variants) {
    const mutate = (line: string) => line.replaceAll(from, to);
    assert.notEqual(mutate(baseline[1]), baseline[1], section);
    const oldBytes = jsonl([baseline[0], mutate(baseline[1]), ...baseline.slice(2)]);
    const brokenPin = { ...pin, bytes: Buffer.byteLength(oldBytes), sha256: sha256(oldBytes) };
    const newBytes = jsonl([candidate[0].replace(pin.sha256, brokenPin.sha256), mutate(candidate[1]), ...candidate.slice(2)]);
    await writeFile(oldFile, oldBytes);
    await writeFile(newFile, newBytes);
    await assert.rejects(verifyR6Archive(oldFile, newFile, brokenPin),
      { message: `D0 line 2: ${section} hash mismatch (SHA-256 preimage)` });
  }
});

test("a grammar cut lists each reparsed legacy row and recounts its header", async t => {
  const file = join(await temporary(t), "candidate.jsonl");
  const printedSuffix = candidate[1].slice(printedStart);
  const legacy = JSON.parse(candidate[1].slice(0, printedStart) + "}");
  legacy.resolution.residuals = ["grammar:fixture"];
  legacy.resolution.complete = false;
  legacy.resolutionSha256 = sha256(JSON.stringify(legacy.resolution));
  const changedRow = JSON.stringify(legacy).slice(0, -1) + printedSuffix;
  const rows = [changedRow, ...candidate.slice(2)];
  const header = JSON.parse(candidate[0]);
  const resolutionSummary = emptyResolutionSummary();
  for (const line of rows) {
    const row = JSON.parse(line);
    if (row.status !== "excluded") addResolutionSummary(resolutionSummary, row.resolution);
  }
  const recounted = { ...header, resolutionSummary };
  await writeFile(file, jsonl([JSON.stringify(recounted), ...rows]));
  const result = await verifyR6Archive(fixture("baseline"), file, pin, { legacyChanges: "listed" });
  assert.deepEqual(result.changedLegacyRows, [{ sourceIndex: 0, oracleIdentity: legacy.oracleIdentity, name: "Élan",
    before: "parsed", after: "parsed" }]);
  // The inert transition still rejects the same row and its recount.
  await assert.rejects(verifyR6Archive(fixture("baseline"), file, pin), /D0 line 1: R6 header fields/);
  await writeFile(file, jsonl([candidate[0], ...rows]));
  await assert.rejects(verifyR6Archive(fixture("baseline"), file, pin), /D0 line 2: legacy row prefix differs/);
  // The unchanged golden candidate lists nothing.
  assert.deepEqual((await verifyR6Archive(fixture("baseline"), fixture("candidate"), pin, { legacyChanges: "listed" })).changedLegacyRows, []);
  const negatives: [string, string, RegExp][] = [
    ["R5 counts kept", jsonl([candidate[0], ...rows]), /D0 line 1: grammar-cut header differs/],
    ["stale resolution hash", jsonl([JSON.stringify(recounted), changedRow.replace(legacy.resolutionSha256, "0".repeat(64)), ...rows.slice(1)]),
      /D0 line 2: resolution hash mismatch/],
    ["changed source", jsonl([JSON.stringify(recounted), changedRow.replace("Élan", "Elan"), ...rows.slice(1)]),
      /D0 line 2: (changed legacy row has a different source than R5|source hash mismatch)/],
    ["changed exclusion", jsonl([JSON.stringify(recounted), rows[0], rows[1].replace("layout:token", "new-reason"), rows[2]]),
      /D0 line 3: excluded row differs/],
  ];
  for (const [description, data, error] of negatives) {
    await writeFile(file, data);
    await assert.rejects(verifyR6Archive(fixture("baseline"), file, pin, { legacyChanges: "listed" }), error, description);
  }
});
