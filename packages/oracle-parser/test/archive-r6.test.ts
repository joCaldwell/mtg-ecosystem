import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, type TestContext } from "node:test";
import { fileURLToPath } from "node:url";
import { prepareArchive } from "../../../scripts/archive-parser.ts";
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
    ["modified legacy prefix", jsonl(candidate.map((line, i) => i === 1 ? line.replace("Élan", "Elan") : line)),
      /D0 line 2: legacy row prefix differs/],
    ["modified printed preimage", jsonl(candidate.map((line, i) => i === 3 ? line.replace('"field":"null"', '"field":"absent"') : line)),
      /D0 line 4: printed hash mismatch/],
    ["truncated archive", jsonl(candidate.slice(0, -1)), /D0 line 4: candidate ended before the last pinned R5 row/],
    ["extra row", jsonl([...candidate, candidate.at(-1)!]), /D0 line 5: candidate has a row beyond R5/],
    ["missing LF", candidate.join("\n"), /line 4: missing LF terminator/],
  ];
  for (const [description, data, error] of variants) {
    await writeFile(file, data);
    await assert.rejects(verifyR6Archive(fixture("baseline"), file, pin), error, description);
  }
});

test("D0 checks legacy hash preimages even when the prefixes agree", async t => {
  const directory = await temporary(t);
  const oldFile = join(directory, "baseline.jsonl");
  const newFile = join(directory, "candidate.jsonl");
  const mutate = (line: string) => line.replace('"oracle_text":"Flying"', '"oracle_text":"Lifelink"');
  const oldBytes = jsonl(baseline.map((line, i) => i === 1 ? mutate(line) : line));
  const brokenPin = { ...pin, bytes: Buffer.byteLength(oldBytes), sha256: sha256(oldBytes) };
  const newBytes = jsonl(candidate.map((line, i) => i === 0 ? line.replace(pin.sha256, brokenPin.sha256)
    : i === 1 ? mutate(line) : line));
  await writeFile(oldFile, oldBytes);
  await writeFile(newFile, newBytes);
  await assert.rejects(verifyR6Archive(oldFile, newFile, brokenPin), /D0 line 2: source hash mismatch \(SHA-256 preimage\)/);
});
