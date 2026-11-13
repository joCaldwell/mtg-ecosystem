// Explicit read-only producer-artifact verification. No native execution or SDK
// generation. New query schema is evidence only; the client SDK pin stays intact.
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
const client = new URL('../', import.meta.url);
const producer = process.env.MTG_DECISION_PRODUCER_DIR;
if (!producer) throw new Error('Set MTG_DECISION_PRODUCER_DIR to the explicit source-qualified producer export directory');
const root = pathToFileURL(`${producer.replace(/\/$/, '')}/`);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const manifestBytes = await readFile(new URL('producer-manifest.json', root));
const manifest = JSON.parse(manifestBytes);
const nativeBytes = await readFile(new URL(manifest.source_manifest.split('/').at(-1), root));
if (hash(nativeBytes) !== manifest.source_manifest_sha256) throw new Error('Producer source manifest mismatch');
for (const [name, artifact] of Object.entries(manifest.artifacts)) if (hash(await readFile(new URL(name, root))) !== artifact.sha256) throw new Error('Producer artifact digest mismatch');
const captureBytes = await readFile(new URL('public-prefixes.json', root));
const capture = JSON.parse(captureBytes);
const schemaBytes = await readFile(new URL('protocol.schema.json', root));
const schema = JSON.parse(schemaBytes);
const baseline = JSON.parse(await readFile(new URL('../../game-server/sdk/generated/protocol.schema.json', import.meta.url)));
const pins = JSON.parse(await readFile(new URL('sdk-qualification.json', client)));
if (hash(await readFile(new URL('../../game-server/sdk/generated/protocol.schema.json', import.meta.url))) !== pins.protocol_schema_sha256) throw new Error('Baseline schema changed');
const require = createRequire(new URL('../../game-server/sdk/package.json', import.meta.url));
const { Ajv } = require('ajv');
const ajv = new Ajv({ strict: true, allErrors: false, coerceTypes: false, useDefaults: false, removeAdditional: false });
for (const [name, maximum] of [['uint8', 255], ['uint32', 4_294_967_295], ['uint64', Number.MAX_SAFE_INTEGER], ['uint', Number.MAX_SAFE_INTEGER]]) ajv.addFormat(name, { type: 'number', validate: number => Number.isSafeInteger(number) && number >= 0 && number <= maximum });
const observation = protocol => ({ definitions: protocol.resume.definitions, $ref: '#/definitions/Observation' });
const validateBaseline = ajv.compile(observation(baseline));
const validateObservation = ajv.compile(observation(schema));
const validateQuery = ajv.compile(schema.action_query);
if (capture.engine_build_id !== manifest.native_info.engine_build_id || capture.server_build_id !== manifest.native_info.server_build_id) throw new Error('Capture build identity mismatch');
const summaries = [];
for (const item of capture.captures) {
  const summary = { profile: item.profile, namespace: item.capture_namespace, catalog_sha256: item.catalog_sha256, fixture_sha256: item.fixture_sha256, stages: [] };
  const expected = manifest.capture_inventory.find(value => value.namespace === item.capture_namespace);
  if (!expected || expected.profile !== item.profile || expected.catalog_sha256 !== item.catalog_sha256) throw new Error('Capture namespace/catalog mismatch');
  for (const stage of item.stages) {
    const obs = stage.observation;
    if (!validateBaseline(obs) || !validateObservation(obs)) throw new Error('Producer observation not valid under both explicit schemas');
    const listed = expected.stages.find(value => value.stage === stage.stage);
    if (!listed || listed.viewer !== obs.view.viewer || listed.request !== obs.decision?.request || listed.has_query !== Boolean(stage.query)) throw new Error('Capture inventory mismatch');
    const query = stage.query;
    if (query && (!validateQuery(query) || query.viewer !== obs.view.viewer || query.match_id !== item.match_id || query.outcome.page?.token !== obs.decision?.token)) throw new Error('Captured query binding/schema mismatch');
    summary.stages.push({ stage: stage.stage, viewer: obs.view.viewer, request: obs.decision?.request ?? null,
      observation_sha256: hash(JSON.stringify(obs)), query_sha256: query ? hash(JSON.stringify(query)) : null,
      query_status: query?.outcome.status ?? null, action_families: [...new Set(query?.outcome.page?.beginnings.map(value => value.initiation.action) ?? [])].sort(),
      labels: [...new Set(query?.outcome.page?.beginnings.map(value => value.label) ?? [])],
      source_context: query?.outcome.page?.context?.source ?? null });
  }
  summaries.push(summary);
}
const output = {
  provenance_version: 1, producer_base_source: manifest.base_source, producer_manifest_sha256: hash(manifestBytes), native_source_manifest_sha256: hash(nativeBytes),
  engine_build_id: capture.engine_build_id, server_build_id: capture.server_build_id,
  producer_protocol_schema_sha256: hash(schemaBytes), baseline_validation_schema_sha256: pins.protocol_schema_sha256,
  public_prefixes_sha256: hash(captureBytes), raw_artifact_locator: 'producer-manifest.json#artifacts.public-prefixes.json (explicit producer directory)',
  binaries: Object.fromEntries(['mtg-server', 'action_fixture', 'provision_client'].map(name => [name, manifest.artifacts[name].sha256])),
  captures: summaries, scope: 'Two isolated admitted offline prefixes only. Observation hashes use compact JSON.stringify over parsed producer bytes; full raw-file hash is separately pinned. Query beginnings do not prove accepted responses, whole-profile coverage or client integration. Labels remain unavailable; no source/card metadata imported.',
};
const bytes = JSON.stringify(output, null, 2) + '\n';
const destination = new URL('fixtures/decision-controls/prefix-provenance.json', client);
if (process.argv.includes('--write')) await writeFile(destination, bytes);
else if (await readFile(destination, 'utf8') !== bytes) throw new Error('Producer provenance changed; review before writing');
console.log(`Verified ${summaries.length} isolated profiles, ${summaries.flatMap(value => value.stages).length} permitted observations; new query schema is evidence only. SDK unchanged.`);
