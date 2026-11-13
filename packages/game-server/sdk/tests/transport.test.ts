import assert from 'node:assert/strict';
import test from 'node:test';
import { requestJSON, TransportError } from '../src/transport.ts';
import { ProtocolError } from '../src/validation.ts';

const headers = { 'content-type': 'application/json' };
test('interrupted response bodies are sanitized transport failures with no SDK retry', async () => {
  let calls = 0;
  const secret = 'private transport detail';
  const fetcher = async () => {
    calls++;
    let reads = 0;
    return new Response(new ReadableStream({ pull(controller) {
      if (reads++ === 0) controller.enqueue(new TextEncoder().encode('{"value":'));
      else controller.error(new Error(secret));
    } }), { headers });
  };
  await assert.rejects(requestJSON(fetcher, 'http://127.0.0.1:1', {}, 1024), error =>
    error instanceof TransportError && !String(error).includes(secret) && error.cause === undefined);
  assert.equal(calls, 1);
});

test('completed malformed JSON and invalid UTF-8 remain protocol failures', async () => {
  for (const bytes of [new TextEncoder().encode('{"value":'), new Uint8Array([0xff])]) {
    await assert.rejects(requestJSON(async () => new Response(bytes, { headers }), 'http://127.0.0.1:1', {}, 1024), ProtocolError);
  }
});

test('abort during a response body keeps abort classification', async () => {
  const abort = new AbortController();
  await assert.rejects(requestJSON(async () => new Response(new ReadableStream({ pull(controller) {
    abort.abort(); controller.error(new Error('body interrupted'));
  } }), { headers }), 'http://127.0.0.1:1', {}, 1024, abort.signal), { name: 'AbortError' });
});
