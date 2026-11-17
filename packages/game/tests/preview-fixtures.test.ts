import assert from 'node:assert/strict';
import test from 'node:test';
import { cp, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyPreviewFixtures } from '../scripts/verify-preview.mjs';

const source = fileURLToPath(new URL('../fixtures/foundations-opening/', import.meta.url));
test('public preview gate verifies the unchanged qualified capture without SDK generation', async () => {
  const result = await verifyPreviewFixtures();
  assert.equal(result.files, 5);
});
test('public preview gate rejects altered capture and presentation bytes', async () => {
  const root = await mkdtemp(join(tmpdir(), 'multiverse-preview-fixtures-'));
  try {
    for (const name of ['opening-seat0-http.json', 'presentation.json']) {
      const fixture = join(root, name.replace('.json', ''));
      await cp(source, fixture, { recursive: true });
      await writeFile(join(fixture, name), `${await readFile(join(fixture, name), 'utf8')} `);
      await assert.rejects(verifyPreviewFixtures(fixture), new RegExp(`digest mismatch: ${name}`));
    }
  } finally { await rm(root, { recursive: true, force: true }); }
});
