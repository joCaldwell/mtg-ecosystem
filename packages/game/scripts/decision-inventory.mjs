// Explicit client-only inventory. Reads qualified generated SDK contracts and
// pinned public evidence; never invokes Cargo, installs, fetches or changes pins.
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import ts from 'typescript';
const client = new URL('../', import.meta.url);
const sdk = new URL('../../game-server/sdk/', import.meta.url);
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const readJSON = async (url) => JSON.parse(await readFile(url, 'utf8'));
const pins = await readJSON(new URL('sdk-qualification.json', client));
const schemaBytes = await readFile(new URL('generated/protocol.schema.json', sdk));
if (hash(schemaBytes) !== pins.protocol_schema_sha256)
  throw new Error('Inventory requires the currently qualified protocol export');
const generation = await readFile(new URL('generated/generation.json', sdk));
const build = await readJSON(new URL('dist/build.json', sdk));
if (
  hash(generation) !== build.generation_sha256 ||
  build.schema_sha256 !== pins.protocol_schema_sha256 ||
  JSON.parse(generation).bootstrap_schema_sha256 !== pins.bootstrap_schema_sha256
)
  throw new Error('Inventory SDK generation/build qualification mismatch');
for (const file of build.files)
  if (hash(await readFile(new URL(`dist/${file.path}`, sdk))) !== file.sha256)
    throw new Error('Inventory SDK artifact changed');
for (const source of build.sources)
  if (hash(await readFile(new URL(source.path, sdk))) !== source.sha256)
    throw new Error('Inventory SDK source changed');
const schema = JSON.parse(schemaBytes);
const openingProvenancePath = 'fixtures/foundations-opening/provenance.json';
const openingProvenanceBytes = await readFile(new URL(openingProvenancePath, client));
const provenance = JSON.parse(openingProvenanceBytes);
for (const file of provenance.files)
  if (hash(await readFile(new URL(`fixtures/foundations-opening/${file.path}`, client))) !== file.sha256)
    throw new Error('Inventory fixture provenance mismatch');
const controls = await readFile(new URL('web/src/decisions/controls.ts', client), 'utf8');
const ast = ts.createSourceFile('controls.ts', controls, ts.ScriptTarget.Latest, true);
function variants(definitions, name, tag) {
  return definitions[name].oneOf.map(row=>row.properties[tag].const).sort();
}
const {definitions: visible}=schema.resume;
const {definitions: command}=schema.command;
const shapes=variants(visible,'Shape','shape');
const answers=variants(command,'Answer','answer');
for (const fn of ['decisionSurface', 'answerFor']) {
  const declaration = ast.statements.find((node) => ts.isFunctionDeclaration(node) && node.name?.text === fn);
  const dispatch = declaration?.body.statements.find((node) => ts.isSwitchStatement(node));
  const cases = dispatch?.caseBlock.clauses.map((clause) => clause.expression?.text).sort();
  if (JSON.stringify(cases) !== JSON.stringify(shapes)) throw Error('Incomplete shape dispatch in ' + fn);
}
if (JSON.stringify(shapes) !== JSON.stringify(answers)) throw Error('Shape and answer families differ');
const report = {
  protocol_version: schema.protocol_version,
  protocol_schema_sha256: hash(schemaBytes),
  opening_provenance_sha256: hash(openingProvenanceBytes),
  shapes,
  answers,
  reasons: variants(visible,'Reason','reason'),
  candidates: variants(visible,'Candidate','candidate'),
  actions: variants(visible,'Action','action'),
  joint_rules: variants(visible,'JointRule','rule'),
  payment_steps: variants(visible,'PayStep','step'),
  payment_answers: variants(command,'PayAnswer','step'),
};
const dest = new URL('fixtures/decision-controls/coverage.json', client);
const bytes = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--write')) await writeFile(dest, bytes);
else if ((await readFile(dest, 'utf8')) !== bytes) throw Error('Stale client decision inventory');
console.log('Qualified inventory: ' + shapes.length + ' shapes, ' + answers.length + ' answers');
