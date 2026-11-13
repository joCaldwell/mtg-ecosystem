// Explicit local preparation only. Uses the SDK's locked generator/validator;
// never starts Cargo, invokes the producer or builds SDK artifacts implicitly.
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import ts from 'typescript';

const sdkRequire = createRequire(new URL('../../game-server/sdk/package.json', import.meta.url));
if (sdkRequire('json-schema-to-typescript/package.json').version !== '16.0.0') throw new Error('Install the pinned SDK generator before fixture preparation');
const { compile } = sdkRequire('json-schema-to-typescript');
const { Ajv } = sdkRequire('ajv');
const fixtureRoot = new URL('../fixtures/foundations-opening/', import.meta.url);
const generated = new URL('../generated/', import.meta.url);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const read = async name => readFile(new URL(name, fixtureRoot));
const pins = JSON.parse(await read('provenance.json'));
for (const file of pins.files) {
  if (hash(await read(file.path)) !== file.sha256) throw new Error(`Fixture digest mismatch: ${file.path}`);
}
const protocolBytes = await readFile(new URL('../../game-server/sdk/generated/protocol.schema.json', import.meta.url));
if (hash(protocolBytes) !== pins.protocol_schema_sha256) throw new Error('SDK schema is not the pinned fixture schema; prepare SDK explicitly first');
const protocol = JSON.parse(protocolBytes);
const sdkRoot = new URL('../../game-server/sdk/', import.meta.url);
const sdkBuild = JSON.parse(await readFile(new URL('dist/build.json', sdkRoot)));
if (sdkBuild.schema_sha256 !== pins.protocol_schema_sha256) throw new Error('SDK browser build uses a different schema');
for (const file of sdkBuild.files) {
  if (hash(await readFile(new URL(`dist/${file.path}`, sdkRoot))) !== file.sha256) throw new Error('SDK browser artifact digest mismatch');
}
for (const source of sdkBuild.sources) {
  if (hash(await readFile(new URL(source.path, sdkRoot))) !== source.sha256) throw new Error('SDK sources changed after the browser build; rebuild explicitly');
}
const presentationSchema = JSON.parse(await read('presentation.schema.json'));
const presentation = JSON.parse(await read('presentation.json'));
const opening = JSON.parse(await read('opening-seat0-http.json'));
const ajv = new Ajv({ strict: true, allErrors: false, coerceTypes: false, useDefaults: false, removeAdditional: false });
for (const [name, maximum] of [['uint8', 255], ['uint32', 4_294_967_295], ['uint64', Number.MAX_SAFE_INTEGER], ['uint', Number.MAX_SAFE_INTEGER]]) {
  ajv.addFormat(name, { type: 'number', validate: number => Number.isSafeInteger(number) && number >= 0 && number <= maximum });
}
if (!ajv.validate(presentationSchema, presentation)) throw new Error('Producer presentation schema rejected fixture');
if (!ajv.validate(protocol.resume, opening.resume)) throw new Error('Native resume schema rejected fixture');
if (opening.resume.catalog_sha256 !== presentation.catalog_sha256 || presentation.catalog_sha256 !== pins.catalog_sha256) throw new Error('Fixture catalog binding mismatch');
if (opening.resume.viewer !== 0 || opening.resume.state.status !== 'available' || opening.resume.state.observation.view.viewer !== opening.resume.viewer) throw new Error('Expected source-qualified seat-zero observation');
const declaration = await compile(presentationSchema, 'PresentationArtifact', {
  bannerComment: '/* Generated from the pinned native-lane presentation producer schema. Do not edit. */',
  unknownAny: true, unreachableDefinitions: true,
});
const ast = ts.createSourceFile('presentation.d.ts', declaration, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
function visit(node) {
  if (node.kind === ts.SyntaxKind.AnyKeyword || node.kind === ts.SyntaxKind.UnknownKeyword) throw new Error('Unconstrained generated presentation type');
  ts.forEachChild(node, visit);
}
visit(ast);
await mkdir(generated, { recursive: true });
await writeFile(new URL('presentation.d.ts', generated), declaration);
await writeFile(new URL('fixture-verification.json', generated), JSON.stringify({
  native_source: pins.native_source, sdk_source: pins.sdk_source, protocol_schema_sha256: pins.protocol_schema_sha256,
  presentation_schema_sha256: hash(await read('presentation.schema.json')),
  presentation_declaration_sha256: hash(declaration), generator: 'json-schema-to-typescript@16.0.0',
  observation_version: opening.resume.state.observation.observation_version,
  request: opening.resume.state.observation.decision?.request,
  sdk_browser_manifest_sha256: hash(await readFile(new URL('dist/build.json', sdkRoot))),
}, null, 2) + '\n');
console.log('Pinned fixtures validated; producer-schema presentation declaration generated. No native/SDK build invoked.');
