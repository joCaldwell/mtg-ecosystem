import { AsyncLocalStorage } from "node:async_hooks";
import { NodeSqliteDialect } from "@better-auth/kysely-adapter/node-sqlite-dialect";
import type { DatabaseConnection, Driver } from "kysely";
import type { Database } from "./sqlite.ts";

// One connection, one transaction state. The dialect takes this reservation
// for one SQL query or transaction, NEVER for an HTTP handler/outbound fetch.
// Application service batches use the same reentrant reservation.
const gates = new WeakMap<Database, ReturnType<typeof makeGate>>();
function makeGate() {
  const context = new AsyncLocalStorage<{ active: boolean }>();
  let tail = Promise.resolve();
  async function acquire() {
    if (context.getStore()?.active) return () => {};
    const previous = tail;
    let release = () => {};
    tail = new Promise<void>(resolve => { release = resolve; });
    await previous;
    return release;
  }
  async function run<T>(work: () => T | Promise<T>): Promise<T> {
    if (context.getStore()?.active) return work();
    const release = await acquire(), owner = { active: true };
    try { return await context.run(owner, work); }
    finally { owner.active = false; release(); }
  }
  return { acquire, run };
}
export function databaseGate(db: Database) {
  let gate = gates.get(db);
  if (!gate) { gate = makeGate(); gates.set(db, gate); }
  return gate;
}

// Official native dialect API, pinned to 1.7.13:
// https://github.com/better-auth/better-auth/blob/v1.7.13/packages/kysely-adapter/src/node-sqlite-dialect.ts
// Kysely acquires once for a query or the entire transaction callback, and
// releases in finally. Retain that boundary and the native driver's mutex.
export function authDialect(db: Database): NodeSqliteDialect {
  const dialect = new NodeSqliteDialect({ database: db });
  const native = dialect.createDriver(), gate = databaseGate(db);
  const leases = new WeakMap<DatabaseConnection, { connection: DatabaseConnection; release: () => void }>();
  const driver: Driver = {
    init: () => native.init(),
    async acquireConnection() {
      const release = await gate.acquire();
      try {
        const connection = await native.acquireConnection();
        const wrapper: DatabaseConnection = {
          executeQuery: query => connection.executeQuery(query),
          streamQuery: (query, size) => connection.streamQuery(query, size),
        };
        leases.set(wrapper, { connection, release });
        return wrapper;
      } catch (error) { release(); throw error; }
    },
    beginTransaction: (connection, settings) => native.beginTransaction(connection, settings),
    commitTransaction: connection => native.commitTransaction(connection),
    rollbackTransaction: connection => native.rollbackTransaction(connection),
    async releaseConnection(connection) {
      const lease = leases.get(connection)!;
      try { await native.releaseConnection(lease.connection); }
      finally { leases.delete(connection); lease.release(); }
    },
    // The application owns this shared connection's lifetime.
    async destroy() {},
  };
  dialect.createDriver = () => driver;
  return dialect;
}
