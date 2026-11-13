import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareCommand, parsePreparedCommand } from '@multiverse/game-server-sdk/browser';
import { decodeRecord, storedRecord, JournalFailure } from '../src/session/journal.ts';
import { IndexedDBCommandJournal } from '../src/session/effects/command-journal.ts';
const codec = { prepare: prepareCommand, parse: parsePreparedCommand };
const binding = { origin: 'http://127.0.0.1:80', match: 'a1', seat: '0', engineBuild: '1'.repeat(64), serverBuild: '2'.repeat(64), catalog: '3'.repeat(64), protocol: '4'.repeat(64) };
const prepared = prepareCommand({ protocol_version: 1, command_id: 'a'.repeat(32), expected_revision: '1', command: { kind: 'decision', token: 'v0-d0', response: { response: 'mulligan', take: false } } });
const stored = { version: 1, binding, exactBody: prepared.exactBody };
const corrupt = (value: unknown) => assert.throws(() => decodeRecord(value, codec), (error: unknown) => error instanceof JournalFailure && error.kind === 'corrupt');

test('strict bounded journal decoder preserves explicit :80 identity and persists no redundant envelope/secrets', () => {
  const record = decodeRecord(stored, codec);
  assert.equal(record.binding.origin, binding.origin);
  assert.ok(Object.isFrozen(record.binding)); assert.ok(Object.isFrozen(record.envelope));
  assert.deepEqual(Object.keys(storedRecord(record) as object).sort(), ['binding', 'exactBody', 'version']);
  for (const value of [null, [], {}, { ...stored, version: 2 }, { ...stored, envelope: prepared.envelope }, { ...stored, capability: 'extra' },
    { ...stored, observation: {} }, { ...stored, diagnostics: 'private' }, { ...stored, exactBody: 1 }, { ...stored, exactBody: ' '.repeat(65537) },
    { ...stored, exactBody: '{"command_id":"a","command_id":"b"}' }, { ...stored, exactBody: '[]' }]) corrupt(value);
  for (const patch of [{ origin: 'http://user:password@localhost:80' }, { origin: 'http://localhost:80/path' }, { origin: 'http://localhost/?secret=a' },
    { seat: '4' }, { match: 'wrong-match' }, { catalog: 'not-a-digest' }, { engineBuild: '' }, { protocol: '1' }, { secret: 'extra' }]) corrupt({ ...stored, binding: { ...binding, ...patch } });
});

test('unavailable/blocked/open-failure IndexedDB behavior is bounded and never fabricates an empty journal', async () => {
  const original = Object.getOwnPropertyDescriptor(globalThis, 'indexedDB');
  Object.defineProperty(globalThis, 'indexedDB', { configurable: true, get() { throw new DOMException('denied', 'SecurityError'); } });
  try {
    const journal = new IndexedDBCommandJournal(codec);
    await assert.rejects(journal.unresolved(binding), (error: unknown) => error instanceof JournalFailure && error.kind === 'unavailable');
  } finally { if (original) Object.defineProperty(globalThis, 'indexedDB', original); else Reflect.deleteProperty(globalThis, 'indexedDB'); }
  for (const event of ['onblocked', 'onerror'] as const) {
    const request = { error: new DOMException('open failed', 'UnknownError') } as unknown as IDBOpenDBRequest;
    const factory = { open() { queueMicrotask(() => request[event]?.(new Event(event) as IDBVersionChangeEvent)); return request; } } as unknown as IDBFactory;
    const journal = new IndexedDBCommandJournal(codec, { factory });
    await assert.rejects(journal.unresolved(binding), (error: unknown) => error instanceof JournalFailure && error.kind === (event === 'onblocked' ? 'blocked' : 'failed'));
  }
});
