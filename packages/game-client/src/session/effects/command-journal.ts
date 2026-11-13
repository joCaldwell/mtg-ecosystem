import type { CommandEnvelope } from '@multiverse/game-server-sdk';
import type { CommandJournal, JournalRecord, SeatBinding } from '../contracts.ts';
import { decodeRecord, JournalFailure, MAX_JOURNAL_RECORDS, sameBinding, seatKey, storedRecord, type CommandCodec } from '../journal.ts';

/** IndexedDB transaction scheduling/commit authority:
 * https://w3c.github.io/IndexedDB/#transaction-scheduling
 * https://w3c.github.io/IndexedDB/#dom-idbtransaction-durability */
const STORE = 'commands';
const DB_VERSION = 1;
const DEADLINE_MS = 3000;
function failure(error: unknown): JournalFailure {
  if (error instanceof JournalFailure) return error;
  return new JournalFailure(error instanceof DOMException && error.name === 'QuotaExceededError' ? 'full' : 'failed');
}
/** No database opens until the session has acquired its seat lock and checked
 * current authorization/manifest. Transactions, not Web Locks alone, enforce one
 * slot per origin/match/seat across build/catalog/schema changes. */
export class IndexedDBCommandJournal implements CommandJournal<CommandEnvelope> {
  readonly #codec: CommandCodec;
  readonly #factory: IDBFactory | undefined;
  readonly #name: string;
  constructor(codec: CommandCodec, options: { factory?: IDBFactory; name?: string } = {}) {
    this.#codec = codec; this.#factory = options.factory;
    this.#name = options.name ?? 'multiverse-command-journal';
  }
  #open(): Promise<IDBDatabase> {
    let factory: IDBFactory | undefined;
    try { factory = this.#factory ?? globalThis.indexedDB; }
    catch { return Promise.reject(new JournalFailure('unavailable')); }
    if (!factory) return Promise.reject(new JournalFailure('unavailable'));
    return new Promise((resolve, reject) => {
      let settled = false;
      const timer = setTimeout(() => finish(new JournalFailure('blocked')), DEADLINE_MS);
      const finish = (error: JournalFailure) => { if (!settled) { settled = true; clearTimeout(timer); reject(error); } };
      let request: IDBOpenDBRequest;
      try { request = factory.open(this.#name, DB_VERSION); }
      catch { finish(new JournalFailure('unavailable')); return; }
      request.onupgradeneeded = () => {
        if (settled) { request.transaction?.abort(); return; }
        request.result.createObjectStore(STORE);
      };
      request.onblocked = () => finish(new JournalFailure('blocked'));
      request.onerror = () => finish(failure(request.error));
      request.onsuccess = () => {
        const db = request.result;
        db.onversionchange = () => db.close();
        if (settled) { db.close(); return; }
        settled = true; clearTimeout(timer); resolve(db);
      };
    });
  }
  async #transaction<T>(mode: IDBTransactionMode, work: (store: IDBObjectStore, result: (value: T) => void, fail: (error: unknown) => void) => void): Promise<T> {
    const db = await this.#open();
    return new Promise((resolve, reject) => {
      let tx: IDBTransaction;
      try { tx = db.transaction(STORE, mode, { durability: 'strict' }); }
      catch (error) { db.close(); reject(failure(error)); return; }
      let output: T, issue: unknown;
      const timer = setTimeout(() => { issue = new JournalFailure('blocked'); try { tx.abort(); } catch { /* completion already queued */ } }, DEADLINE_MS);
      const done = () => { clearTimeout(timer); db.close(); };
      tx.oncomplete = () => { done(); resolve(output); };
      tx.onabort = tx.onerror = () => { done(); reject(failure(issue ?? tx.error)); };
      const fail = (error: unknown) => { issue = error; tx.abort(); };
      try { work(tx.objectStore(STORE), value => { output = value; }, fail); }
      catch (error) { fail(error); }
    });
  }
  async unresolved(binding: SeatBinding): Promise<readonly JournalRecord<CommandEnvelope>[]> {
    return this.#transaction('readonly', (store, result, fail) => {
      const request = store.get(seatKey(binding));
      request.onsuccess = () => {
        try {
          if (request.result === undefined) { result([]); return; }
          const record = decodeRecord(request.result, this.#codec);
          if (seatKey(record.binding) !== seatKey(binding)) throw new JournalFailure('corrupt');
          result([record]);
        } catch (error) { fail(error); }
      };
    });
  }
  async persist(record: JournalRecord<CommandEnvelope>): Promise<void> {
    const validated = decodeRecord(storedRecord(record), this.#codec);
    return this.#transaction('readwrite', (store, result, fail) => {
      const key = seatKey(validated.binding);
      const existing = store.get(key);
      existing.onsuccess = () => {
        if (existing.result !== undefined) { fail(new JournalFailure('conflict')); return; }
        const count = store.count();
        count.onsuccess = () => {
          if (count.result >= MAX_JOURNAL_RECORDS) { fail(new JournalFailure('full')); return; }
          store.add(storedRecord(validated), key); result(undefined);
        };
      };
    });
  }
  async resolve(record: JournalRecord<CommandEnvelope>, _disposition: import('../contracts.ts').DefinitiveDisposition): Promise<void> {
    return this.#transaction('readwrite', (store, result, fail) => {
      const key = seatKey(record.binding), request = store.get(key);
      request.onsuccess = () => {
        try {
          const existing = decodeRecord(request.result, this.#codec);
          if (existing.exactBody !== record.exactBody || !sameBinding(existing.binding, record.binding)) throw new JournalFailure('conflict');
          store.delete(key); result(undefined);
        } catch (error) { fail(error); }
      };
    });
  }
}
