import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// The engine retains exact archived records as Card IR sources. Their archive
// headers, record hashes and parsed abilities must agree with this parser.
const fixtures = new URL('../../game-engine/tests/fixtures/', import.meta.url);
const read = (name: string) => readFileSync(new URL(name, fixtures), 'utf8');
const sha = (value: string) => createHash('sha256').update(value).digest('hex');

test('retained archive hashes agree with JavaScript and source parser', () => {
  const provenance = JSON.parse(read('provenance.json'));
  const archives = JSON.parse(read('accepted-archives.json'));
  for (const archive of archives) {
    const raw = read(archive.manifest_file);
    const header = JSON.parse(raw);
    assert.equal(sha(raw.trimEnd()), archive.manifest_sha256);
    assert.equal(header.parser.revision, archive.parser_revision);
    assert.equal(header.parser.sourceSha256, archive.parser_source_sha256);
    assert.equal(header.corpus.sha256, archive.corpus_sha256);
  }
  assert.ok(provenance.records.length > 0);
  for (const witness of provenance.records) {
    const archive = archives.find((a: any) => a.manifest_file === witness.archive);
    assert.ok(archive, witness.file);
    const raw = read(witness.file);
    const record = JSON.parse(raw);
    assert.equal(sha(raw.trimEnd()), witness.record_sha256);
    assert.equal(sha(JSON.stringify(record.source)), record.sourceSha256);
    if (record.binding != null) assert.equal(sha(JSON.stringify(record.binding)), record.bindingSha256);
    else assert.equal(record.bindingSha256, undefined);
    if (record.resolution != null) assert.equal(sha(JSON.stringify(record.resolution)), record.resolutionSha256);
    assert.equal(record.sourceSha256, witness.source_sha256);
    assert.equal(record.astSha256 ?? null, witness.ast_sha256);
    if (record.astSha256 != null) {
      assert.equal(sha(JSON.stringify(record.faces.map((face: any) => face.result.abilities))), record.astSha256);
    }
    // Extraction also preserves unparsed records for rejection witnesses.
    if (record.status !== 'parsed') continue;
    const parsed = parseOracleText(record.source.oracle_text, record.source.name, {
      layout: record.source.layout, typeLine: record.source.type_line, includeSourceProvenance: true,
    });
    assert.equal(parsed.ok, true, record.source.name);
    // Archive interchange is JSON: optional undefined TS properties are absent.
    if (parsed.ok) assert.deepEqual(JSON.parse(JSON.stringify(parsed.abilities)), record.faces[0].result.abilities, record.source.name);
  }
});
