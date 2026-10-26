import { PRICE_STORES_VERSION } from "./prices.ts";
import { spawn, type ChildProcess } from "node:child_process";
import { randomUUID } from "node:crypto";
import { join } from "node:path";
import type { Database } from "./sqlite.ts";
import { DATA_DIR, DEFAULT_DB_PATH, PKG_ROOT, withTransaction } from "./db.ts";

export type JobName = "refresh" | "backup";
export interface JobRun {
  last_run: string;
  finished_at: string | null;
  result: "running" | "success" | "error";
  error?: string;
}
const HOUR = 60 * 60 * 1000;
const DAY = 24 * HOUR;
export const LOCK_MAX_AGE = 2 * 60 * 60 * 1000;
const read = (db: Database, key: string) => (db.prepare("SELECT value FROM meta WHERE key = ?").get(key) as { value: string } | undefined)?.value;
const write = (db: Database, key: string, value: string) => db.prepare("INSERT INTO meta VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value").run(key, value);

export function acquireJobLock(db: Database, now = Date.now()): () => void {
  const token = randomUUID();
  withTransaction(db, () => {
    // Acquire the SQLite write reservation before reading, avoiding a WAL
    // read-to-write upgrade race when the CLI and scheduler start together.
    db.prepare("INSERT OR IGNORE INTO meta VALUES ('job:lock', '')").run();
    const raw = read(db, "job:lock");
    if (raw && now - (JSON.parse(raw) as { timestamp: number }).timestamp <= LOCK_MAX_AGE)
      throw new Error("Another refresh or backup job is running");
    write(db, "job:lock", JSON.stringify({ token, timestamp: now }));
  });
  return () => db.prepare("DELETE FROM meta WHERE key = 'job:lock' AND json_extract(value, '$.token') = ?").run(token);
}

export async function runJob<T>(db: Database, name: JobName, work: () => Promise<T>): Promise<T> {
  const release = acquireJobLock(db);
  const run: JobRun = { last_run: new Date().toISOString(), finished_at: null, result: "running" };
  try {
    write(db, `job:${name}`, JSON.stringify(run));
    const result = await work();
    run.result = "success";
    return result;
  } catch (e) {
    run.result = "error";
    // Do not persist raw SDK errors that can contain credentials or signed URLs.
    run.error = `${name} failed; see the server/CLI log`;
    throw e;
  } finally {
    run.finished_at = new Date().toISOString();
    try { write(db, `job:${name}`, JSON.stringify(run)); }
    finally { release(); }
  }
}

function nextNight(now: number) {
  const date = new Date(now);
  date.setHours(2, 0, 0, 0);
  if (date.getTime() <= now) date.setDate(date.getDate() + 1);
  return date.getTime();
}

/** The serving process only schedules: every heavy operation runs in a child. */
export function createJobs(db: Database, opts: {
  dbPath?: string; dataDir?: string; onChange?: () => void;
  spawnJob?: (name: JobName) => ChildProcess;
  /** Hosted auth shares this connection; delayed failure writes must queue
   * behind its async transaction rather than accidentally joining it. */
  writeFailure?: (write: () => void) => Promise<void>;
} = {}) {
  const pending = new Set<JobName>();
  let active: JobName | null = null, child: ChildProcess | null = null, stopped = false;
  const initialNight = nextNight(Date.now());
  const run = (name: JobName): JobRun | null => {
    const value = read(db, `job:${name}`);
    return value ? JSON.parse(value) : null;
  };
  const next = (name: JobName) => {
    const last = run(name);
    if (name === "backup") return last ? nextNight(Date.parse(last.last_run)) : initialNight;
    if (last?.result === "error") return Date.parse(last.last_run) + HOUR;
    if (read(db, "price_stores_version") !== PRICE_STORES_VERSION) return Date.now();
    const dates = [read(db, "last_ingest"), read(db, "prices_as_of")].map(v => v ? Date.parse(v) : 0);
    const freshUntil = Math.min(...dates) + DAY;
    return Math.max(freshUntil, last ? Date.parse(last.last_run) + DAY : 0);
  };
  const status = () => ({
    active, queued: [...pending],
    refresh: { last_run: null, result: null, finished_at: null, ...run("refresh"), next_run: new Date(Math.max(Date.now(), next("refresh"))).toISOString() },
    backup: { last_run: null, result: null, finished_at: null, ...run("backup"), next_run: new Date(Math.max(Date.now(), next("backup"))).toISOString() },
  });
  function pump() {
    if (stopped || active || !pending.size) return;
    const lock = read(db, "job:lock");
    if (lock && Date.now() - JSON.parse(lock).timestamp <= LOCK_MAX_AGE) return;
    const name = pending.values().next().value!;
    pending.delete(name);
    active = name;
    const lastRun = new Date().toISOString();
    let finished = false;
    const finish = async (code: number | null, error?: Error) => {
      if (finished) return;
      finished = true;
      if (code !== 0) {
        const persist=()=>write(db, `job:${name}`, JSON.stringify({ last_run: lastRun, finished_at: new Date().toISOString(), result: "error", error: error?.message ?? `Child exited ${code}` }));
        if(opts.writeFailure) await opts.writeFailure(persist);else persist();
      }
      console.log(`Job ${name} finished (${code ?? "error"})`);
      active = null; child = null;
      opts.onChange?.();
      pump();
    };
    try {
      child = opts.spawnJob?.(name) ?? spawn(process.execPath, [join(PKG_ROOT, "src", "cli.ts"), name], {
        cwd: PKG_ROOT, env: { ...process.env, DECKBUILDER_DB: opts.dbPath ?? DEFAULT_DB_PATH, DECKBUILDER_DATA: opts.dataDir ?? DATA_DIR },
        stdio: ["ignore", "pipe", "pipe"],
      });
      child.stdout?.on("data", chunk => process.stdout.write(chunk));
      child.stderr?.on("data", chunk => process.stderr.write(chunk));
      child.once("error", error => { void finish(null,error).catch(()=>console.error("Could not record failed job")); });
      child.once("close", code => { void finish(code).catch(()=>console.error("Could not record failed job")); });
    } catch (error) { void finish(null,error as Error).catch(()=>console.error("Could not record failed job")); }
    opts.onChange?.();
  }
  const enqueue = (name: JobName) => { if (active !== name) pending.add(name); pump(); return status(); };
  const tick = () => {
    if (stopped) return;
    for (const name of ["refresh", "backup"] as const) if (active !== name && next(name) <= Date.now()) pending.add(name);
    pump();
  };
  const timer = setInterval(tick, 30_000);
  timer.unref();
  const start = () => {
    const fresh = read(db, "price_stores_version") === PRICE_STORES_VERSION && [read(db, "last_ingest"), read(db, "prices_as_of")]
      .every(value => value != null && Date.now() - Date.parse(value) < DAY);
    if (!fresh) pending.add("refresh");
    tick();
  };
  return { status, enqueue, start, close: async () => {
    stopped = true; clearInterval(timer); pending.clear();
    // Let an active job release its lock and finish its transaction on shutdown.
    if (child) await new Promise<void>(resolve => child!.once("close", () => resolve()));
  } };
}
export type Jobs = ReturnType<typeof createJobs>;
