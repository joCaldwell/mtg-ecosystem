// Check explicitly qualified public SDK bytes in both build and final layouts.
// No generation, native invocation, dependency install or private producer source.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, lstatSync } from 'node:fs';
import { dirname, resolve, relative, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const sdk = resolve(dirname(fileURLToPath(import.meta.resolve('@multiverse/game-server-sdk'))), '..');
const read = path => readFileSync(join(sdk, path));
const json = path => JSON.parse(read(path));
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const build = json('dist/build.json'), generation = json('generated/generation.json'), verification = json('generated/verification.json');
assert.equal(build.generation_sha256, sha(read('generated/generation.json')));
assert.equal(verification.generation_manifest_sha256, build.generation_sha256);
assert.equal(verification.status, 'preliminary-sdk-checks-passed');
assert.equal(build.schema_sha256, generation.schema_sha256);
const files = directory => readdirSync(join(sdk, directory)).sort().flatMap(name => {
  const path = `${directory}/${name}`, stat = lstatSync(join(sdk, path));
  assert.ok(!stat.isSymbolicLink(), `SDK symlink: ${path}`);
  if (stat.isDirectory()) return files(path);
  assert.ok(stat.isFile(), `SDK special file: ${path}`);
  return [path];
});
const check = (path, rows) => {
  assert.ok(!relative(sdk, resolve(sdk, path)).startsWith('..'), 'SDK manifest path escapes package');
  const row = rows.find(row => row.path === path);
  assert.ok(row, `Unverified SDK runtime file: ${path}`);
  assert.equal(sha(read(path)), row.sha256, `SDK runtime digest: ${path}`);
};
check('package.json', verification.source_files);
for (const path of files('src')) check(path, verification.source_files);
const generatedRows = generation.files.filter(row => /\.(ts|js)$/.test(row.path)).map(row => ({ ...row, path: `generated/${row.path}` }));
assert.deepEqual(files('generated').filter(path => /\.(ts|js)$/.test(path)).sort(), generatedRows.map(row => row.path).sort());
for (const row of generatedRows) check(row.path, generatedRows);
assert.deepEqual(files('dist').sort(), [...build.files.map(row => `dist/${row.path}`), 'dist/build.json'].sort());
for (const row of build.files) check(`dist/${row.path}`, build.files.map(row => ({ ...row, path: `dist/${row.path}` })));
const root = await import('@multiverse/game-server-sdk');
const browser = await import('@multiverse/game-server-sdk/browser');
const policies = await import('@multiverse/game-server-sdk/policies');
const producer = await import('@multiverse/game-server-sdk/presentation-producer');
assert.equal(root.GameClient, browser.GameClient);
assert.equal(typeof policies.foundationsPolicy, 'function');
assert.equal(typeof producer.producePresentation, 'function');
console.log(JSON.stringify({ sdk_public_runtime: 'verified', schema_sha256: build.schema_sha256,
  generation_sha256: build.generation_sha256, build_sha256: sha(read('dist/build.json')),
  exports: ['.', './browser', './policies', './presentation-producer'] }));
