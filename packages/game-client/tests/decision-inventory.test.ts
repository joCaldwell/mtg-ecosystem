import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import test from 'node:test';

test('decision inventory matches qualified SDK, pinned fixture hashes and reviewed coverage sources', () => {
  // Read-only public-artifact verification. No generator, installs or Cargo.
  const output = execFileSync(process.execPath, [new URL('../scripts/decision-inventory.mjs', import.meta.url).pathname], { encoding: 'utf8' });
  assert.match(output, /Qualified inventory: 28 requests, 33 responses; 9 local adapters/);
});
