// Determinism for the harness's Node processes (seed and serve). The app is
// not changed: time and randomness are replaced from outside, before any
// capability runs.
//
// - JS time: globalThis.Date is a Proxy whose zero-argument forms read a
//   settable clock. Dates built from a value behave normally.
// - SQLite time: the app writes timestamps with datetime('now') in SQL and in
//   column DEFAULTs. SQLite lets an application function overload a built-in
//   (https://www.sqlite.org/appfunc.html, "built-in SQL functions may be
//   overloaded"), and DEFAULT expressions use the connection's functions, so
//   datetime('now') reads the same clock. Any other datetime(...) call is
//   delegated to a pristine in-memory connection, so its result is SQLite's own.
// - Randomness: Math.random and node:crypto's randomBytes/randomInt/randomUUID
//   come from one seeded PRNG. module.syncBuiltinESMExports() pushes the
//   patched functions into the ESM named exports that src/ already imported
//   (https://nodejs.org/api/module.html#modulesyncbuiltinesmexports).

import { createRequire, syncBuiltinESMExports } from "node:module";
import { DatabaseSync } from "node:sqlite";

const require = createRequire(import.meta.url);

/** The instant the browser is frozen at, and the server's clock while serving. */
export const FROZEN_NOW = "2026-10-20T17:00:00.000Z";

export const RealDate = Date;
let current = RealDate.parse(FROZEN_NOW);

/** Moves the shared clock. Accepts ISO or SQLite "YYYY-MM-DD HH:MM:SS" (UTC). */
export function setNow(when: string | number): void {
  const t = typeof when === "number" ? when : RealDate.parse(/Z|[+-]\d\d:\d\d$/.test(when) ? when : when.replace(" ", "T") + "Z");
  if (!Number.isFinite(t)) throw new Error(`Unreadable clock value: ${when}`);
  current = t;
}
export function now(): number { return current; }
function sqliteNow(): string {
  return new RealDate(current).toISOString().slice(0, 19).replace("T", " ");
}

let dateInstalled = false;
export function installClock(): void {
  if (dateInstalled) return;
  dateInstalled = true;
  globalThis.Date = new Proxy(RealDate, {
    construct(target, args, newTarget) {
      return Reflect.construct(target, args.length ? args : [current], newTarget);
    },
    apply() { return new RealDate(current).toString(); },
    get(target, prop, receiver) {
      if (prop === "now") return () => current;
      return Reflect.get(target, prop, receiver);
    },
  }) as DateConstructor;
}

/** Overload datetime() on one connection so 'now' reads the shared clock. */
export function installSqliteClock(db: { function: DatabaseSync["function"] }): void {
  const raw = new DatabaseSync(":memory:");
  const cache = new Map<number, ReturnType<DatabaseSync["prepare"]>>();
  db.function("datetime", { varargs: true, deterministic: false, directOnly: false }, (...args: unknown[]) => {
    if (args.length === 1 && typeof args[0] === "string" && args[0].toLowerCase() === "now") return sqliteNow();
    const n = args.length;
    let stmt = cache.get(n);
    if (!stmt) {
      stmt = raw.prepare(`SELECT datetime(${Array.from({ length: n }, () => "?").join(",")}) AS v`);
      cache.set(n, stmt);
    }
    const replaced = args.map(a => (typeof a === "string" && a.toLowerCase() === "now" ? sqliteNow() : a));
    return (stmt.get(...(replaced as any[])) as { v: string | null }).v;
  });
}

// mulberry32: small, fast, and identical on every run for the same seed.
function mulberry32(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function installRandom(seed: number): void {
  const next = mulberry32(seed);
  Math.random = next;
  const crypto = require("node:crypto");
  const bytes = (size: number) => {
    const out = Buffer.alloc(size);
    for (let i = 0; i < size; i++) out[i] = Math.floor(next() * 256);
    return out;
  };
  crypto.randomBytes = (size: number, cb?: (err: Error | null, buf: Buffer) => void) => {
    const buf = bytes(size);
    if (cb) { queueMicrotask(() => cb(null, buf)); return undefined; }
    return buf;
  };
  crypto.randomInt = (min: number, max?: number, cb?: unknown) => {
    if (max === undefined || typeof max === "function") { cb = max; max = min; min = 0; }
    const value = min + Math.floor(next() * (max - min));
    if (typeof cb === "function") { queueMicrotask(() => (cb as (e: null, v: number) => void)(null, value)); return undefined; }
    return value;
  };
  crypto.randomUUID = () => {
    const b = bytes(16);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    const h = b.toString("hex");
    return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
  };
  syncBuiltinESMExports();
}
