// Verify the offline Worker export against the same schema and build used by the app.
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { streamValidator } from '../../game-server/sdk/generated/validators.js';

const producer = process.env.MTG_DECISION_PRODUCER_DIR;
if (!producer) throw Error('Set MTG_DECISION_PRODUCER_DIR to the action_fixture output directory');
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const bytes = await readFile(resolve(producer, 'public-prefixes.json'));
const capture = JSON.parse(bytes);
const pins = JSON.parse(await readFile(new URL('../sdk-qualification.json', import.meta.url)));
if (capture.engine_build_id !== pins.engine_build_id || capture.server_build_id !== pins.server_build_id)
  throw Error('Prefix build mismatch');
if (capture.captures.length !== 1) throw Error('Expected the embedded Foundations capture');
const [pod] = capture.captures;
if (pod.profile !== pins.profile || pod.catalog_sha256 !== pins.catalog_sha256) throw Error('Prefix catalog mismatch');
const stages = pod.stages.map(({ stage, observation }) => {
  const stream = {
    protocol_version: 2,
    entries: [{ cursor: '1', publication: { kind: 'view_replaced', observation } }],
    next: '1',
    has_more: false,
  };
  if (!streamValidator(stream)) throw Error('Invalid native prefix observation');
  const { decision, view } = observation;
  if (decision?.player !== view.viewer || observation.observation_version !== pins.observation_version)
    throw Error('Prefix viewer/version mismatch');
  return {
    stage,
    viewer: view.viewer,
    reason: decision.reason.reason,
    shape: decision.shape.shape,
    candidates: decision.candidates.length,
    actions: [
      ...new Set(
        decision.candidates.flatMap((candidate) => (candidate.candidate === 'action' ? [candidate.action.action] : [])),
      ),
    ].sort(),
  };
});
if (stages.map((s) => s.stage).join() !== 'opening,main-priority-before-land,main-priority-before-cast,cast-mana')
  throw Error('Incomplete Foundations prefix');
const report = {
  profile: pod.profile,
  catalog_sha256: pod.catalog_sha256,
  engine_build_id: capture.engine_build_id,
  server_build_id: capture.server_build_id,
  protocol_schema_sha256: pins.protocol_schema_sha256,
  public_prefixes_sha256: hash(bytes),
  stages,
};
const output = JSON.stringify(report, null, 2) + '\n';
const destination = new URL('../fixtures/decision-controls/prefix-provenance.json', import.meta.url);
if (process.argv.includes('--write')) await writeFile(destination, output);
else if ((await readFile(destination, 'utf8')) !== output) throw Error('Public prefix provenance differs');
console.log('Verified four Foundations stages against the qualified decision schema');
