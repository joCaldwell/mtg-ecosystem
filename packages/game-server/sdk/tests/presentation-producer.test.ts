import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { producePresentation } from '@multiverse/game-server-sdk/presentation-producer';
import type { PublicManifest } from '../generated/manifest.js';
import { canonical, presentation, sha256 } from '../../tools/client-artifacts.ts';
import { PRESENTATION_PRODUCER_SHA256, PRESENTATION_SCHEMA_SHA256 } from '../generated/presentation-identity.ts';
const catalog = JSON.parse(fs.readFileSync(new URL('../../../game-engine/tests/fixtures/expected-match-ir.json', import.meta.url), 'utf8'));
function fixture(): PublicManifest {
  return { protocol_version: 1, match_id: 'abc', engine_build_id: 'b'.repeat(64), server_build_id: 'c'.repeat(64), catalog_sha256: sha256(canonical(catalog)), profile: 'commander-foundations-v1', decks: Array.from({ length: 4 }, () => ({ commander: catalog.definitions[0].oracle_id, cards: [] })) as PublicManifest['decks'], starting_player: 0, catalog: structuredClone(catalog) };
}
test('public Node producer packages original source authority and exact allowlisted bytes', async () => {
  const value = fixture(), result = await producePresentation(value, value);
  assert.equal(new TextDecoder().decode(result.bytes), canonical(presentation(value.catalog)) + '\n');
  assert.equal(result.descriptor.producer_sha256, PRESENTATION_PRODUCER_SHA256);
  assert.equal(result.descriptor.producer_sha256, sha256(fs.readFileSync(new URL('../../tools/client-artifacts.ts', import.meta.url))));
  assert.equal(result.descriptor.presentation_schema_sha256, PRESENTATION_SCHEMA_SHA256);
  assert.equal(result.descriptor.path, `/presentation/${sha256(result.bytes)}.json`);
  const build = JSON.parse(fs.readFileSync(new URL('../dist/build.json', import.meta.url), 'utf8'));
  for (const path of ['../tools/client-artifacts.ts', '../tools/presentation.schema.json', 'src/presentation-producer.mjs']) assert.ok(build.sources.some(row => row.path === path));
  assert.ok(build.node_runtime_inputs.includes('../tools/client-artifacts.ts'));
  assert.ok(!build.runtime_inputs.includes('src/presentation-producer.mjs'));
  assert.equal(fs.readFileSync(new URL('../dist/presentation.schema.json', import.meta.url), 'utf8'), fs.readFileSync(new URL('../../tools/presentation.schema.json', import.meta.url), 'utf8'));
});
test('producer rejects exact game/build/catalog identity and canonical digest disagreement', async () => {
  const value = fixture();
  for (const key of ['match_id', 'engine_build_id', 'server_build_id', 'catalog_sha256', 'profile']) await assert.rejects(producePresentation(value, { ...value, [key]: 'wrong' }));
  await assert.rejects(producePresentation({ ...value, catalog_sha256: 'a'.repeat(64) }, { ...value, catalog_sha256: 'a'.repeat(64) }));
  await assert.rejects(producePresentation({ ...value, private_hand: [] } as any, value));
  await assert.rejects(producePresentation({ ...value, private_hand: 'x'.repeat(8 * 1024 * 1024) } as any, value));
});
test('producer rejects source digest/printing/Oracle ambiguity even with a matching catalog hash', async () => {
  for (const mutation of ['digest', 'printing', 'oracle', 'duplicate', 'image', 'faces', 'ambiguous']) {
    const value = fixture(), definition = value.catalog.definitions[0]!;
    const source = JSON.parse(definition.provenance.source_json);
    if (mutation === 'digest') definition.provenance.source_sha256 = '0'.repeat(64);
    if (mutation === 'printing') source.id = 'wrong';
    if (mutation === 'oracle') source.oracle_id = 'wrong';
    if (mutation === 'duplicate') value.catalog.definitions.push(structuredClone(definition));
    if (mutation === 'image') source.image_uris = { normal: 'https://private@invalid.example/x' };
    if (mutation === 'faces') source.card_faces = [];
    if (!['digest', 'duplicate'].includes(mutation)) {
      definition.provenance.source_json = mutation === 'ambiguous' ? JSON.stringify(source).replace('{', '{"oracle_id":"ambiguous",') : JSON.stringify(source);
      definition.provenance.source_sha256 = sha256(definition.provenance.source_json);
    }
    value.catalog_sha256 = sha256(canonical(value.catalog));
    await assert.rejects(producePresentation(value, value), mutation);
  }
});
