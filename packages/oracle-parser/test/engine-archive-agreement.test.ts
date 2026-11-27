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
    if (archive.pending) {
      assert.equal(archive.cut, 'r6');
      assert.equal(archive.file_sha256, null);
      assert.deepEqual(archive.record_sha256, []);
      continue;
    }
    const raw = read(archive.manifest_file);
    const header = JSON.parse(raw);
    assert.equal(sha(raw.trimEnd()), archive.manifest_sha256);
    if (archive.candidate) {
      // A candidate cut is measured but authorizes no tracked fixture rows.
      assert.equal(archive.cut, 'r6');
      assert.deepEqual(archive.record_sha256, []);
    }
    if (archive.cut === 'r6') {
      assert.match(archive.parser_revision, /^[0-9a-f]{40}$/);
      assert.equal(header.schemaVersion, 5);
      assert.equal(header.cut, 'r6');
      assert.deepEqual(Object.keys(header.parser), ['sourceSha256']);
      assert.equal(header.rules.sha256, provenance.rules_sha256);
      assert.equal(header.printed.schemaVersion, 1);
      assert.equal(header.printed.schemaSha256, archive.r6.printed_schema_sha256);
      assert.equal(header.printed.vocabularySha256, archive.r6.printed_vocabulary_sha256);
      assert.equal(header.printed.generatorSha256, archive.r6.printed_generator_sha256);
      assert.equal(header.printed.recordsSha256, archive.r6.printed_records_sha256);
      assert.equal(header.nameCatalog.sha256, archive.r6.name_catalog_sha256);
      assert.equal(header.nameCatalog.names, archive.r6.name_catalog_names);
      assert.equal(header.inputRecords, archive.r6.input_records);
      assert.equal(header.corpus.canonicalRecordsSha256, archive.r6.canonical_records_sha256);
    } else {
      assert.ok(['r4', 'r5'].includes(archive.cut));
      assert.equal(header.schemaVersion, 4);
      assert.equal(header.parser.revision, archive.parser_revision);
    }
    assert.equal(header.parser.sourceSha256, archive.parser_source_sha256);
    assert.equal(header.corpus.sha256, archive.corpus_sha256);
  }
  assert.ok(provenance.records.length > 0);
  for (const witness of provenance.records) {
    const archive = archives.find((a: any) => a.manifest_file === witness.archive);
    assert.ok(archive && !archive.pending, witness.file);
    const raw = read(witness.file);
    const record = JSON.parse(raw);
    assert.equal(sha(raw.trimEnd()), witness.record_sha256);
    assert.ok(archive.record_sha256.includes(witness.record_sha256));
    if (archive.cut === 'r6' && record.status !== 'excluded') {
      assert.equal(sha(JSON.stringify(record.printed)), record.printedSha256);
      assert.equal(record.printed.faces.length, record.faces.length);
    } else {
      assert.equal(Object.hasOwn(record, 'printed'), false);
      assert.equal(Object.hasOwn(record, 'printedSha256'), false);
    }
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
    const faces = record.source.card_faces ?? [record.source];
    assert.equal(record.faces.length, faces.length, record.source.name);
    for (const [index, face] of faces.entries()) {
      const archived = record.faces[index];
      assert.equal(archived.sourceFaceIndex, index, face.name);
      const parsed = parseOracleText(face.oracle_text, face.name, {
        layout: record.source.layout,
        typeLine: face.type_line ?? record.source.type_line,
        includeSourceProvenance: true,
      });
      assert.ok(parsed.ok, face.name);
      // Archive interchange omits optional undefined properties.
      assert.deepEqual(JSON.parse(JSON.stringify(parsed.abilities)), archived.result.abilities, face.name);
    }
  }
});
