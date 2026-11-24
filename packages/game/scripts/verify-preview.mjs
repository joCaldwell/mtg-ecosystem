import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const fixtureDirectory = fileURLToPath(new URL('../fixtures/foundations-opening/', import.meta.url));
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

/** Node-only consumer gate. Exact immutable bytes retain their recorded capture
 * qualification; this neither regenerates nor accepts a new native/SDK schema. */
export async function verifyPreviewFixtures(directory = fixtureDirectory) {
  const read = name => readFile(resolve(directory, name));
  const pins = JSON.parse(await read('provenance.json'));
  if (pins.fixture_version !== 1 || !Array.isArray(pins.files) || !/^[a-f0-9]{64}$/.test(pins.catalog_sha256)) throw new Error('Invalid preview provenance');
  const verified = new Map();
  for (const file of pins.files) {
    if (!/^[a-zA-Z0-9._-]+\.json$/.test(file.path) || !/^[a-f0-9]{64}$/.test(file.sha256) || verified.has(file.path)) throw new Error('Invalid preview provenance entry');
    const bytes = await read(file.path);
    if (sha256(bytes) !== file.sha256) throw new Error(`Preview fixture digest mismatch: ${file.path}`);
    verified.set(file.path, bytes);
  }
  for (const name of ['opening-seat0.json', 'presentation.json', 'presentation.schema.json', 'producer-artifacts.json', 'decision-inventory.json']) {
    if (!verified.has(name)) throw new Error(`Missing preview provenance: ${name}`);
  }
  const capture = JSON.parse(verified.get('opening-seat0.json'));
  const presentation = JSON.parse(verified.get('presentation.json'));
  const producer = JSON.parse(verified.get('producer-artifacts.json'));
  const resume = capture.resume;
  if (resume.catalog_sha256 !== pins.catalog_sha256 || presentation.catalog_sha256 !== pins.catalog_sha256 || producer.catalog_sha256 !== pins.catalog_sha256 || producer.protocol_schema_sha256 !== pins.protocol_schema_sha256 || producer.producer_sha256 !== pins.producer_sha256) throw new Error('Preview catalog/source binding mismatch');
  if (resume.viewer !== 0 || resume.state.status !== 'available' || resume.state.observation.view.viewer !== resume.viewer || resume.state.observation.decision?.player !== resume.viewer || resume.state.observation.decision?.reason.reason !== 'mulligan') throw new Error('Preview capture/viewer binding mismatch');
  return { files: verified.size, captureSource: pins.native_source, catalogSha256: pins.catalog_sha256 };
}
