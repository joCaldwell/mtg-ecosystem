import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import * as root from '@multiverse/game-server-sdk';
import * as browser from '@multiverse/game-server-sdk/browser';

test('published package exports dependency-free ESM and source-qualified declarations', () => {
  assert.deepEqual(root, browser);
  assert.deepEqual(Object.keys(root).sort(), ['GameClient', 'HttpError', 'PROTOCOL_VERSION', 'ProtocolError', 'TransportError', 'counter', 'parseClientConfig', 'parsePreparedCommand', 'parseSeatBootstrap', 'prepareCommand', 'readClientConfig']);
  const build = JSON.parse(fs.readFileSync(new URL('../dist/build.json', import.meta.url), 'utf8'));
  const generation = JSON.parse(fs.readFileSync(new URL('../generated/generation.json', import.meta.url), 'utf8'));
  assert.equal(build.schema_sha256, generation.schema_sha256);
  for (const file of build.files) assert.equal(createHash('sha256').update(fs.readFileSync(new URL(`../dist/${file.path}`, import.meta.url))).digest('hex'), file.sha256);
  assert(build.runtime_inputs.every(name => /^(src|generated)\//.test(name)));
  assert(!build.runtime_inputs.some(name => /examples|tests|fixtures|node_modules/.test(name)));
  assert.equal(root.counter('18446744073709551615'), '18446744073709551615');
  assert.throws(() => root.counter('18446744073709551616'), root.ProtocolError);
});
