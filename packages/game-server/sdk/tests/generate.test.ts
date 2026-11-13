import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const sdk = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const route = (value, operation) => value.routes.find(row => row.operation === operation);
const source = JSON.parse(fs.readFileSync(path.join(sdk, 'generated/protocol.schema.json'), 'utf8'));

test('generation is repeatable and rejects future, unconstrained and external-reference schemas without publishing partial declarations', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'mtg-sdk-generator-'));
  try {
    const input = path.join(directory, 'source.json'), output = path.join(directory, 'generated');
    const run = value => {
      fs.writeFileSync(input, JSON.stringify(value));
      return spawnSync(process.execPath, [path.join(sdk, 'scripts/generate.mjs')], {
        encoding: 'utf8', env: { ...process.env, MTG_SERVER_SCHEMA: input, MTG_SERVER_BOOTSTRAP_SCHEMA: path.join(sdk, 'generated/bootstrap.schema.json'), MTG_SERVER_OUTPUT: output },
      });
    };
    assert.equal(run(source).status, 0);
    const inventory = () => Object.fromEntries(fs.readdirSync(output).map(name => [name, fs.readFileSync(path.join(output, name), 'utf8')]));
    const first = inventory();
    assert.equal(run(source).status, 0); assert.deepEqual(inventory(), first);
    const future = structuredClone(source); future.protocol_version = 2;
    const unknown = structuredClone(source); unknown.command.properties.command = {};
    const external = structuredClone(source); external.command.properties.command = { $ref: 'https://untrusted.example/receipt.json' };
    const tupleDialect = structuredClone(source); tupleDialect.command.prefixItems = [];
    const unknownRoute = structuredClone(source); unknownRoute.routes[0].operation = 'omniscient_state';
    const externalRoute = structuredClone(source); externalRoute.routes[0].path = 'https://untrusted.example/{match_id}';
    const duplicateRoute = structuredClone(source); duplicateRoute.routes[1] = duplicateRoute.routes[0];
    const traversalRoute = structuredClone(source); traversalRoute.routes[0].path = '/v1/../{match_id}/commands';
    const extraRoute = structuredClone(source); extraRoute.routes[0].viewer = 'caller-selected-seat';
    const changedMethod = structuredClone(source); changedMethod.routes[0].method = 'DELETE';
    const wrongRouteType = structuredClone(source); wrongRouteType.routes[0].operation = ['submit_command'];
    const missingCommandParameter = structuredClone(source); route(missingCommandParameter, 'command_execution').path = '/v1/matches/{match_id}/commands';
    const duplicateCommandParameter = structuredClone(source); route(duplicateCommandParameter, 'cancel_command').path += '/{command_id}';
    const unexpectedCommandParameter = structuredClone(source); route(unexpectedCommandParameter, 'submit_command').path += '/{command_id}';
    const unconstrainedManifest = structuredClone(source); unconstrainedManifest.manifest.properties.catalog = {};
    const externalExecution = structuredClone(source); externalExecution.execution.properties.status = { $ref: 'https://untrusted.example/status.json' };
    const unknownFormat = structuredClone(source); unknownFormat.command.properties.protocol_version.format = 'unreviewed_uint';
    for (const invalid of [future, unknown, external, tupleDialect, unknownRoute, externalRoute, duplicateRoute, traversalRoute, extraRoute, changedMethod, wrongRouteType, missingCommandParameter, duplicateCommandParameter, unexpectedCommandParameter, externalExecution, unconstrainedManifest, unknownFormat]) {
      const result = run(invalid); assert.notEqual(result.status, 0);
      assert.deepEqual(inventory(), first, 'failed generation must preserve last complete declarations and remove temporary output');
    }
    const movedRoute = structuredClone(source); route(movedRoute, 'submit_command').path = '/v1/pods/{match_id}/submit';
    assert.equal(run(movedRoute).status, 0);
    assert(fs.readFileSync(path.join(output, 'routes.ts'), 'utf8').includes('/v1/pods/{match_id}/submit'));
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
});
