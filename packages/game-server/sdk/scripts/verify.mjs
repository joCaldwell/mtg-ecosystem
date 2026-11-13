import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
const sdk = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.join(sdk, 'generated');
fs.mkdirSync(output, { recursive: true });
const logPath = path.join(output, 'verification.log');
const log = fs.openSync(logPath, 'w');
const steps = [];
const testFiles = fs.readdirSync(path.join(sdk, 'tests')).filter(p => p.endsWith('.test.ts')).sort().map(p => `tests/${p}`);
try {
  for (const [name, args] of [
    ['generate', ['scripts/generate.mjs']],
    ['typescript-strict', ['node_modules/typescript/bin/tsc', '--noEmit']],
    ['node-tests', ['--test', '--test-reporter=tap', ...testFiles]],
  ]) {
    fs.writeSync(log, `\nSTEP ${name}\n`);
    const env = { ...process.env }; delete env.MTG_SERVER_OUTPUT;
    const result = spawnSync(process.execPath, args, { cwd: sdk, env, stdio: ['ignore', log, log] });
    steps.push({ name, args, exit_code: result.status, signal: result.signal });
    if (result.error || result.status !== 0) throw new Error(`SDK ${name} failed; see generated/verification.log`);
  }
} finally { fs.closeSync(log); }
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const sources = [];
function walk(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    if (['generated', 'node_modules', 'dist'].includes(entry.name)) continue;
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) walk(file);
    else if (entry.isFile()) { const bytes = fs.readFileSync(file); sources.push({ path: path.relative(sdk, file), bytes: bytes.length, sha256: sha(bytes) }); }
    else throw new Error('Unexpected SDK source symlink or special file');
  }
}
walk(sdk); sources.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
const generationBytes = fs.readFileSync(path.join(output, 'generation.json'));
const generation = JSON.parse(generationBytes);
const schemas = JSON.parse(fs.readFileSync(path.join(output, 'protocol.schema.json')));
const expectedResponses = [
  'legend_rule', 'announce_x', 'sacrifice_cost', 'forced_sacrifice', 'mana_color', 'search_lands', 'optional_recursion', 'optional_effect', 'mulligan', 'bottom_opening',
  'damage_allocation', 'modes', 'damage_effect', 'order_triggers', 'entity_targets',
  'stack_targets', 'activate_ability', 'player_targets', 'sacrifice', 'resolve_discard',
  'spend_mana', 'commander_return', 'attackers', 'blockers', 'damage', 'trample_damage',
  'discard', 'play_land', 'activate_mana', 'cast', 'pass', 'targets', 'finish_mana',
].sort();
const actualResponses = schemas.command.definitions.PlayerResponse.oneOf.map(row => row.properties.response.const).sort();
if (JSON.stringify(actualResponses) !== JSON.stringify(expectedResponses)) throw new Error('Review exhaustive hosted response variants');
const responseVariants = actualResponses.length;
const executionVariants = schemas.execution.definitions.CommandExecutionStatus.oneOf.length;
const logBytes = fs.readFileSync(logPath);
const tests = Number(/^# tests (\d+)$/m.exec(logBytes.toString('utf8'))?.[1]);
if (!Number.isInteger(tests) || tests < 1 || responseVariants !== 33 || executionVariants !== 5 || schemas.routes.length !== 6) throw new Error('Expected test/response contract counters missing');
for (const file of generation.files) if (sha(fs.readFileSync(path.join(output, file.path))) !== file.sha256) throw new Error('Generated declaration hash mismatch');
if (sha(fs.readFileSync(path.join(output, 'protocol.schema.json'))) !== generation.schema_sha256) throw new Error('Rust schema hash mismatch');
const packages = Object.fromEntries(['ajv', 'json-schema-to-typescript', 'typescript'].map(name => [name, JSON.parse(fs.readFileSync(path.join(sdk, 'node_modules', name, 'package.json'))).version]));
const result = {
  verification_version: 1, status: 'preliminary-sdk-checks-passed',
  scope: 'schema/generated-types/mock-HTTP only; not hosting, durable recovery or four remote consumers',
  node: process.version, platform: process.platform, architecture: process.arch,
  tool_versions: packages, generation_manifest_sha256: sha(generationBytes), generation,
  strict_typescript: true, exhaustive_player_response_variants: responseVariants, player_response_names: actualResponses, exhaustive_command_execution_variants: executionVariants,
  tuple_contracts: ['damage', 'damage_allocation', 'attackers', 'blockers'],
  required_nullable_reference_contract: true,
  route_authority: 'Rust protocol::ROUTES', route_manifest: schemas.routes,
  route_manifest_sha256: sha(Buffer.from(JSON.stringify(schemas.routes))),
  strict_raw_json: { nested_duplicate_keys: 'rejected after escape decoding', numeric_lexemes: 'canonical safe integers only', maximum_nesting: 128 },
  node_tests: tests, steps, log_sha256: sha(logBytes), source_files: sources,
};
fs.writeFileSync(path.join(output, 'verification.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({ status: result.status, source_files: sources.length, node_tests: tests, response_variants: responseVariants }));
