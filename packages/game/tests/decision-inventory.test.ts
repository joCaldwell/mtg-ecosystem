import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { OBSERVATION_VERSION } from '../web/src/session/read-state.ts';

test('decision inventory matches qualified wire schemas, web request switches and pinned captures', () => {
  const pins = JSON.parse(readFileSync(new URL('../sdk-qualification.json', import.meta.url), 'utf8'));
  assert.equal(OBSERVATION_VERSION, pins.observation_version);
  // Read-only public-artifact verification. No generator, installs or Cargo.
  const output = execFileSync(process.execPath, [new URL('../scripts/decision-inventory.mjs', import.meta.url).pathname], { encoding: 'utf8' });
  assert.match(output, /Qualified inventory: 5 shapes, 5 answers/);
});
