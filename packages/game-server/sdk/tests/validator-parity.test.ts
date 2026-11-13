import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { Ajv } from 'ajv';
import * as emitted from '../generated/validators.js';

const schemas = JSON.parse(fs.readFileSync(new URL('../generated/protocol.schema.json', import.meta.url), 'utf8'));
schemas.bootstrap = JSON.parse(fs.readFileSync(new URL('../generated/bootstrap.schema.json', import.meta.url), 'utf8'));
// Independent copy of the pre-standalone SDK validator configuration is the
// oracle here; importing the generator's format implementation would mask drift.
const ajv = new Ajv({ strict: true, allErrors: false, coerceTypes: false, useDefaults: false, removeAdditional: false });
for (const [name, maximum] of [['uint8', 255], ['uint32', 4_294_967_295], ['uint64', Number.MAX_SAFE_INTEGER], ['uint', Number.MAX_SAFE_INTEGER]]) {
  ajv.addFormat(name, { type: 'number', validate: n => Number.isSafeInteger(n) && n >= 0 && n <= maximum });
}
const reference = Object.fromEntries(['command', 'stream', 'submission', 'execution', 'manifest', 'resume', 'bootstrap'].map(name => [name, ajv.compile(schemas[name])]));
reference.commandId = ajv.compile(schemas.command.properties.command_id);
reference.counter = ajv.compile({ $ref: '#/definitions/DecimalU64Counter', definitions: schemas.stream.definitions });

// Schema-derived structural witnesses only. These do not assert game legality,
// provide card facts, or substitute for source-qualified live observations.
function witness(schema, root) {
  if (schema.$ref) return witness(schema.$ref.slice(2).split('/').reduce((v, k) => v[k], root), root);
  if ('const' in schema) return schema.const;
  if (schema.enum) return schema.enum[0];
  if (schema.allOf?.length === 1) return witness(schema.allOf[0], root);
  if (schema.oneOf || schema.anyOf) return witness((schema.oneOf ?? schema.anyOf)[0], root);
  const type = Array.isArray(schema.type) ? schema.type[0] : schema.type;
  switch (type) {
    case 'null': return null;
    case 'boolean': return false;
    case 'integer': case 'number': return schema.minimum ?? 0;
    case 'string': {
      const candidates = ['0', 'a'.repeat(32), 'a'.repeat(64), 'a', '', 'http://127.0.0.1:18741'];
      const result = candidates.find(s => s.length >= (schema.minLength ?? 0) && s.length <= (schema.maxLength ?? Infinity) && (!schema.pattern || new RegExp(schema.pattern).test(s)));
      assert.notEqual(result, undefined, `unhandled witness pattern ${schema.pattern}`);
      return result;
    }
    case 'array': return Array.isArray(schema.items) ? schema.items.map(item => witness(item, root)) : Array.from({ length: schema.minItems ?? 0 }, () => witness(schema.items, root));
    case 'object': return Object.fromEntries((schema.required ?? []).map(key => [key, witness(schema.properties[key], root)]));
    default: throw new Error('Unreviewed witness schema');
  }
}
function* mutations(value) {
  yield null; yield false; yield 0; yield '0'; yield []; yield {};
  if (value === null || typeof value !== 'object') {
    for (const candidate of [-1, 0.5, 256, 4_294_967_296, Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER + 1,
      NaN, Infinity, '', '00', '1\n', '-0', '18446744073709551616', '90071992547409931234567890']) yield candidate;
    return;
  }
  if (!Array.isArray(value)) {
    yield { ...value, private_state: {} };
    for (const key of Object.keys(value)) { const copy = structuredClone(value); delete copy[key]; yield copy; }
  }
  for (const [key, child] of Object.entries(value)) {
    for (const replacement of mutations(child)) { const copy = structuredClone(value); copy[key] = replacement; yield copy; }
  }
}
function parity(name, samples) {
  let comparisons = 0;
  for (const sample of samples) {
    const before = structuredClone(sample);
    assert.equal(Boolean(emitted[`${name}Validator`](sample)), Boolean(reference[name](sample)), `${name} parity`);
    assert.deepEqual(sample, before, 'validation must not mutate input');
    comparisons++;
  }
  return comparisons;
}

test('standalone parity with previous runtime Ajv on every response/request variant and adversarial mutations', t => {
  let comparisons = 0;
  const command = witness(schemas.command, schemas.command);
  const requests = schemas.stream.definitions.PlayerRequest.oneOf;
  for (const response of schemas.command.definitions.PlayerResponse.oneOf) {
    const sample = { ...command, command: { kind: 'decision', token: 'token', response: witness(response, schemas.command) } };
    assert(reference.command(sample), JSON.stringify(reference.command.errors));
    comparisons += parity('command', [sample, ...mutations(sample)]);
  }
  for (const request of requests) {
    const observation = witness(schemas.stream.definitions.Observation, schemas.stream);
    observation.decision = witness(request, schemas.stream);
    const sample = { protocol_version: 1, next: '1', has_more: false, entries: [{ cursor: '1', publication: { kind: 'view_replaced', observation } }] };
    assert(reference.stream(sample), JSON.stringify(reference.stream.errors));
    comparisons += parity('stream', [sample, ...mutations(sample)]);
  }
  for (const name of ['submission', 'execution', 'manifest', 'resume', 'bootstrap']) {
    const sample = witness(schemas[name], schemas[name]);
    assert(reference[name](sample), JSON.stringify(reference[name].errors));
    comparisons += parity(name, [sample, ...mutations(sample)]);
  }
  for (const name of ['counter', 'commandId']) comparisons += parity(name, [
    '0', '18446744073709551615', '9007199254740992', 'a'.repeat(32), 'f'.repeat(32),
    ...mutations('0'), 'a'.repeat(31), 'a'.repeat(33), 'A'.repeat(32), 'a'.repeat(32) + '\n',
  ]);
  t.diagnostic(`${comparisons} parity cases; ${schemas.command.definitions.PlayerResponse.oneOf.length} response and ${requests.length} request variants (wire shapes, not hosted coverage)`);
});

test('standalone formats reject unsafe uints and retain exact decimal quantities/tuples', () => {
  const base = { protocol_version: 1, command_id: 'a'.repeat(32), expected_revision: '18446744073709551615' };
  const command = response => ({ ...base, command: { kind: 'decision', token: 't', response } });
  for (const [response, valid] of [
    [{ response: 'player_targets', players: [255] }, true], [{ response: 'player_targets', players: [256] }, false],
    [{ response: 'modes', mode: 4_294_967_295 }, true], [{ response: 'modes', mode: 4_294_967_296 }, false],
    [{ response: 'damage', assignments: [['h', '90071992547409931234567890']] }, true],
    [{ response: 'damage', assignments: [['h', 9_007_199_254_740_992]] }, false],
    [{ response: 'damage', assignments: [['h', '3', 'extra']] }, false],
  ]) {
    const value = command(response);
    assert.equal(emitted.commandValidator(value), valid);
    parity('command', [value]);
  }
});
