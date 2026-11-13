// Rust schema is the sole DTO authority. Pinned generator API:
// https://github.com/bcherny/json-schema-to-typescript#api
import { compile } from 'json-schema-to-typescript';
import ts from 'typescript';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { standalone, validationOptions } from './standalone.mjs';

const sdk = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.resolve(process.env.MTG_SERVER_OUTPUT ?? path.join(sdk, 'generated'));
fs.mkdirSync(output, { recursive: true });
const pending = fs.mkdtempSync(path.join(output, '.pending-'));
try {
const schemaPath = path.join(pending, 'protocol.schema.json');
const bootstrapPath = path.join(pending, 'bootstrap.schema.json');
if (process.env.MTG_SERVER_SCHEMA) {
  if (!process.env.MTG_SERVER_BOOTSTRAP_SCHEMA) throw new Error('MTG_SERVER_BOOTSTRAP_SCHEMA is required with an explicit protocol export');
  fs.copyFileSync(process.env.MTG_SERVER_SCHEMA, schemaPath);
  fs.copyFileSync(process.env.MTG_SERVER_BOOTSTRAP_SCHEMA, bootstrapPath);
} else {
  const executable = process.env.MTG_SERVER_BIN ?? path.resolve(sdk, '../../../target/debug/mtg-server');
  for (const [file, operation] of [[schemaPath, 'schema'], [bootstrapPath, 'bootstrap-schema']]) {
    const fd = fs.openSync(file, 'w');
    try {
      const result = spawnSync(executable, [operation], {
        stdio: ['ignore', fd, 'pipe'], encoding: 'utf8', maxBuffer: 1024 * 1024,
      });
      if (result.error || result.status !== 0) throw new Error(`Rust schema export failed: ${result.error?.message ?? result.stderr}`);
    } finally { fs.closeSync(fd); }
  }
}
const bytes = fs.readFileSync(schemaPath);
const bootstrapBytes = fs.readFileSync(bootstrapPath);
if (bytes.length > 8 * 1024 * 1024 || bootstrapBytes.length > 8 * 1024 * 1024) throw new Error('Protocol schema exceeds generator bound');
const schemas = JSON.parse(bytes.toString('utf8'));
if (schemas.protocol_version !== 1 || Object.keys(schemas).sort().join(',') !== 'command,execution,manifest,protocol_version,resume,routes,stream,submission') {
  throw new Error('Unsupported Rust protocol schema envelope');
}
const bootstrap = JSON.parse(bootstrapBytes.toString('utf8'));
// Operation identities describe supported SDK semantics. Paths and methods
// themselves are exported by Rust; no parallel endpoint strings live in client.
const operations = {
  get_resume: { method: 'GET', response_schema: 'resume' },
  submit_command: { method: 'POST', response_schema: 'submission' },
  get_manifest: { method: 'GET', response_schema: 'manifest' },
  poll_events: { method: 'GET', response_schema: 'stream' },
  command_execution: { method: 'GET', response_schema: 'execution', command_id: true },
  cancel_command: { method: 'POST', response_schema: 'submission', command_id: true },
};
if (!Array.isArray(schemas.routes) || schemas.routes.length !== Object.keys(operations).length) throw new Error('Unsupported Rust route manifest');
const routes = {};
for (const route of schemas.routes) {
  if (!route || typeof route !== 'object' || Array.isArray(route) || Object.keys(route).sort().join(',') !== 'method,operation,path,response_schema') throw new Error('Invalid Rust route descriptor');
  if (['operation', 'method', 'path', 'response_schema'].some(key => typeof route[key] !== 'string')) throw new Error('Invalid Rust route field type');
  const expected = Object.hasOwn(operations, route.operation) ? operations[route.operation] : undefined;
  if (!expected || Object.hasOwn(routes, route.operation) || route.method !== expected.method || route.response_schema !== expected.response_schema) throw new Error('Unsupported Rust route operation');
  if (typeof route.path !== 'string' || route.path.length > 512 || !route.path.startsWith('/')) throw new Error('Invalid Rust route path');
  const parts = route.path.slice(1).split('/');
  const parameters = expected.command_id ? ['{match_id}', '{command_id}'] : ['{match_id}'];
  if (parameters.some(parameter => parts.filter(part => part === parameter).length !== 1) || parts.some(part => !parameters.includes(part) && !/^[A-Za-z0-9_-]+$/.test(part))) throw new Error('Unsafe or unsupported Rust route path');
  routes[route.operation] = { method: route.method, path: route.path, response_schema: route.response_schema };
}
function localOnly(value) {
  if (!value || typeof value !== 'object') return;
  if (typeof value.$ref === 'string' && !value.$ref.startsWith('#/')) throw new Error('External schema references are forbidden');
  if ('prefixItems' in value) throw new Error('Expected Draft7 tuple schema');
  for (const child of Object.values(value)) localOnly(child);
}
const generatorOptions = {
  bannerComment: '/* Generated from Rust mtg-server schema; do not edit. */',
  unknownAny: true, unreachableDefinitions: true,
};
const files = [];
for (const [key, title] of [['command', 'CommandEnvelope'], ['stream', 'StreamPage'], ['submission', 'SubmissionResult'], ['execution', 'CommandExecution'], ['manifest', 'PublicManifest'], ['resume', 'SeatResume'], ['bootstrap', 'SeatBootstrap']]) {
  const schema = key === 'bootstrap' ? bootstrap : schemas[key];
  if (schema.$schema !== 'http://json-schema.org/draft-07/schema#') throw new Error(`Unexpected ${key} schema dialect`);
  localOnly(schema);
  const declaration = await compile(schema, title, generatorOptions);
  // An unsupported shape must fail generation, never silently become any/unknown.
  const ast = ts.createSourceFile(`${key}.d.ts`, declaration, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  function inspect(node) {
    if (node.kind === ts.SyntaxKind.AnyKeyword || node.kind === ts.SyntaxKind.UnknownKeyword) {
      throw new Error(`${key} schema produced an unconstrained TypeScript type`);
    }
    ts.forEachChild(node, inspect);
  }
  inspect(ast);
  const filename = `${key}.d.ts`;
  fs.writeFileSync(path.join(pending, filename), declaration);
  files.push({ path: filename, sha256: createHash('sha256').update(declaration).digest('hex') });
}
const routeSource = `/* Generated from Rust protocol::ROUTES; do not edit. */\nexport const ROUTES = ${JSON.stringify(routes, null, 2)} as const;\n`;
fs.writeFileSync(path.join(pending, 'routes.ts'), routeSource);
files.push({ path: 'routes.ts', sha256: createHash('sha256').update(routeSource).digest('hex') });
const validators = await standalone({ ...schemas, bootstrap });
for (const [filename, content] of [
  ['validators.js', validators.source], ['validators.d.ts', validators.declaration],
  ['protocol.ts', `/* Generated from Rust; do not edit. */\nexport const PROTOCOL_VERSION = ${schemas.protocol_version} as const;\n`],
  ['bootstrap.schema.json', bootstrapBytes],
]) {
  fs.writeFileSync(path.join(pending, filename), content);
  files.push({ path: filename, sha256: createHash('sha256').update(content).digest('hex') });
}
fs.writeFileSync(path.join(pending, 'generation.json'), JSON.stringify({
  protocol_version: schemas.protocol_version, routes: schemas.routes, routes_sha256: createHash('sha256').update(JSON.stringify(schemas.routes)).digest('hex'), generator: 'json-schema-to-typescript@16.0.0',
  generator_options: generatorOptions, schema_dialect: 'Draft7',
  schema_sha256: createHash('sha256').update(bytes).digest('hex'), files,
  bootstrap_schema_sha256: createHash('sha256').update(bootstrapBytes).digest('hex'),
  standalone: { generator: 'ajv@8.20.0', bundler: 'esbuild@0.28.2', options: validationOptions,
    sources: ['scripts/generate.mjs', 'scripts/standalone.mjs', 'scripts/numeric-formats.mjs', 'package-lock.json'].map(file => ({
      path: file, sha256: createHash('sha256').update(fs.readFileSync(path.join(sdk, file))).digest('hex'),
    })),
  },
}, null, 2) + '\n');

// Publish only after every schema/declaration passed. Manifest is committed last.
for (const filename of ['protocol.schema.json', ...files.map(file => file.path), 'generation.json']) {
  fs.renameSync(path.join(pending, filename), path.join(output, filename));
}
console.log(JSON.stringify({ status: 'generated', declarations: files.length, schema_bytes: bytes.length }));
} finally { fs.rmSync(pending, { recursive: true, force: true }); }
