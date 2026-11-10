import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseDeckList } from '../src/deck/interop.ts';
// Real exports and provenance: fixtures/moxfield/README.md.
test('Moxfield plain export preserves its commander and 100-card list', () => {
  const parsed = parseDeckList(readFileSync(new URL('./fixtures/moxfield/kinnan-midrange.copy-plain-text.txt', import.meta.url), 'utf8'));
  assert.deepEqual(parsed.unparsed, []);
  assert.equal(parsed.entries.filter(e => !e.categories.some(c => /^(sideboard|maybeboard)$/i.test(c))).reduce((n, e) => n + e.quantity, 0), 100);
  assert.equal(parsed.entries.filter(e => e.categories.includes('Commander')).length, 1);
});
test('Moxfield printing export strips printing metadata without inventing a commander', () => {
  const parsed = parseDeckList(readFileSync(new URL('./fixtures/moxfield/najeela.copy-for-moxfield.txt', import.meta.url), 'utf8'));
  assert.deepEqual(parsed.unparsed, []);
  assert.ok(parsed.entries.every(e => !/[()*★]/.test(e.name)));
  assert.ok(parsed.entries.every(e => !e.categories.includes('Commander')));
  assert.match(parsed.notes.join(' '), /does not mark the commander/);
});
