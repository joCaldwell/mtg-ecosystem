// Request-local reuse only. SQLite's change counter invalidates the snapshot
// after any intervening write, including organization and nested commands.
import { AsyncLocalStorage } from "node:async_hooks";
import type { Database } from "./sqlite.ts";
const reads = new AsyncLocalStorage<Map<Database, { generation: number; values: Map<string, unknown> }>>();
export function withReadContext<T>(work: () => T): T {
  return reads.getStore() ? work() : reads.run(new Map(), work);
}
export function memoRead<T>(db: Database, key: string, work: () => T): T {
  const context = reads.getStore();
  if (!context) return work();
  const generation = (db.prepare("SELECT total_changes() AS n").get() as { n: number }).n;
  let snapshot = context.get(db);
  if (!snapshot || snapshot.generation !== generation) {
    snapshot = { generation, values: new Map() };
    context.set(db, snapshot);
  }
  if (snapshot.values.has(key)) return snapshot.values.get(key) as T;
  const value = work();
  snapshot.values.set(key, value);
  return value;
}
