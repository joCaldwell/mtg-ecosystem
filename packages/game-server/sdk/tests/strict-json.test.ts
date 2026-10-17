import test from 'node:test';
import assert from 'node:assert/strict';
import { strictJSON } from '../src/strict-json.ts';
import { GameClient, ProtocolError } from '../src/client.ts';

const rawResponse = text => new Response(text, { headers: { 'content-type': 'application/json' } });
const client = text => new GameClient({ baseURL: 'http://127.0.0.1:3000', matchId: 'a', capability: 'private-seat-capability', fetch: async () => rawResponse(text) });
const validPage = '{"protocol_version":1,"entries":[],"next":"0","has_more":false}';

test('raw JSON preserves Unicode/escapes, exact string quantities and canonical safe integers', () => {
  const source = String.raw`{"😀":"雪","escaped":"quote\" slash\\ tab\t","array":[0,-1,9007199254740991,-9007199254740991,true,false,null],"quantity":"90071992547409931234567890"}`;
  assert.deepEqual(strictJSON(source), JSON.parse(source));
  assert.deepEqual(strictJSON(String.raw`{"\uD83D\uDE00":1}`), { '😀': 1 });
  assert.deepEqual(strictJSON('[{"same":1},{"same":2}]'), [{ same: 1 }, { same: 2 }]);
  assert.deepEqual(strictJSON(String.raw`{"é":1,"e\u0301":2}`), { 'é': 1, 'é': 2 }, 'do not invent Unicode normalization aliases');
  assert.equal(strictJSON(String.raw`"literal \"a\":1e0 stays in a string"`), 'literal "a":1e0 stays in a string');
  for (const value of ['0', '-1', '9007199254740991', '-9007199254740991']) assert.equal(strictJSON(value), Number(value));
});

test('raw JSON rejects duplicate keys after unescaping, including astral and prototype-looking names', () => {
  for (const source of [
    '{"a":1,"a":2}', String.raw`{"a":1,"\u0061":2}`,
    String.raw`{"\uD83D\uDE00":1,"😀":2}`,
    String.raw`{"x":{"a":1,"\u0061":2}}`,
    String.raw`[{"\u0061":1,"a":2}]`,
    String.raw`{"__proto__":1,"\u005f\u005fproto__":2}`,
  ]) assert.throws(() => strictJSON(source), ProtocolError, source);
});

test('raw numeric lexemes cannot round into a schema-valid safe integer', () => {
  assert(Number.isSafeInteger(JSON.parse('9007199254740991.1')), 'regression demonstrates information lost by ordinary JSON.parse');
  for (const literal of [
    '9007199254740991.1', '9007199254740990.9', '9007199254740992', '-9007199254740992',
    '1e0', '1E3', '1e-324', '1e309', '0.0', '1.00', '-0', '-0.0',
  ]) {
    for (const source of [literal, `[${literal}]`, `{"nested":{"integer":${literal}}}`]) assert.throws(() => strictJSON(source), ProtocolError, source);
  }
});

test('raw JSON rejects invalid grammar, excessive depth and non-scalar escaped Unicode', () => {
  for (const source of ['', ' ', '{', '{} {}', '[1,]', '{"a":}', '01', '+1', '.1', '1.', 'NaN', 'Infinity', '"\\x41"', '"\n"']) assert.throws(() => strictJSON(source), ProtocolError, source);
  assert.deepEqual(strictJSON('['.repeat(128) + '0' + ']'.repeat(128)), JSON.parse('['.repeat(128) + '0' + ']'.repeat(128)));
  assert.throws(() => strictJSON('['.repeat(129) + '0' + ']'.repeat(129)), /nesting/);
  for (const source of [String.raw`"\ud800"`, String.raw`"\udc00"`, String.raw`{"\ud800":1}`, String.raw`["\ud800x"]`]) assert.throws(() => strictJSON(source), ProtocolError);
});

test('HTTP response decoding invokes the strict scanner before generated schema validation', async () => {
  await client(validPage).poll({ after: '0' });
  for (const source of [
    validPage.replace('"protocol_version":1', '"protocol_version":1,"protocol_version":1'),
    validPage.replace('"protocol_version":1', '"protocol_version":1,"\\u0070rotocol_version":1'),
    validPage.replace('"protocol_version":1', '"protocol_version":1e0'),
    validPage.replace('"protocol_version":1', '"protocol_version":1.00000000000000001'),
    '{"protocol_version":1,"entries":[{"cursor":"1","publication":{"kind":"lifecycle","status":{"status":"running","status":"running"}}}],"next":"1","has_more":false}',
  ]) await assert.rejects(client(source).poll({ after: '0' }), ProtocolError);
  const commandId = 'a'.repeat(32);
  const sdk = client(`{"status":"accepted","command_id":"${commandId}","command_id":"${commandId}"}`);
  await assert.rejects(sdk.submit({ protocol_version: 1, command_id: commandId, expected_revision: '0', command: { kind: 'concede' } }), ProtocolError);
});
