import { backup, Database } from "./sqlite.ts";
import { createReadStream, mkdirSync, readdirSync, renameSync, rmSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { randomUUID } from "node:crypto";
import { DATA_DIR, DEFAULT_DB_PATH, openDb } from "./db.ts";
import { acquireJobLock } from "./jobs.ts";
import { acquireProcessLock } from "./process-lock.ts";

/** Online SQLite backup includes committed WAL content. Never copy the .db. */
export async function backupDatabase(db: Database, dataDir = DATA_DIR, now = new Date()) {
  const dir = join(dataDir, "backups");
  mkdirSync(dir, { recursive: true });
  const file = join(dir, `deck-builder-${now.toISOString().slice(0, 10)}.db`);
  const temp = `${file}.${randomUUID()}.tmp`;
  try {
    // https://nodejs.org/api/sqlite.html#sqlitebackupsourceDb-path-options
    await backup(db, temp);
    renameSync(temp, file);
  } finally { rmSync(temp, { force: true }); }
  const files = readdirSync(dir).filter(n => /^deck-builder-\d{4}-\d{2}-\d{2}\.db$/.test(n)).sort().reverse();
  for (const old of files.slice(14)) rmSync(join(dir, old));
  const keys = ["BACKUP_S3_ENDPOINT", "BACKUP_S3_BUCKET", "BACKUP_S3_ACCESS_KEY", "BACKUP_S3_SECRET_KEY"] as const;
  if (keys.some(k => process.env[k])) {
    if (!keys.every(k => process.env[k])) throw new Error(`S3 backup needs all of ${keys.join(", ")}`);
    // https://docs.aws.amazon.com/sdk-for-javascript/v3/developer-guide/javascript_s3_code_examples.html
    const { S3Client, PutObjectCommand } = await import("@aws-sdk/client-s3");
    const client = new S3Client({
      endpoint: process.env.BACKUP_S3_ENDPOINT, region: "auto", forcePathStyle: true,
      credentials: { accessKeyId: process.env.BACKUP_S3_ACCESS_KEY!, secretAccessKey: process.env.BACKUP_S3_SECRET_KEY! },
    });
    try {
      await client.send(new PutObjectCommand({
        Bucket: process.env.BACKUP_S3_BUCKET, Key: `backups/${file.split("/").at(-1)}`,
        Body: createReadStream(file), ContentLength: statSync(file).size, ContentType: "application/vnd.sqlite3",
      }));
    } finally { client.destroy(); }
  }
  return { file };
}

/** Validate a consistent copy, then atomically replace the stopped instance. */
export async function restoreDatabase(file: string, destination = DEFAULT_DB_PATH) {
  if (resolve(file) === resolve(destination)) throw new Error("Restore source must differ from the live database");
  const release = acquireProcessLock(`${destination}.server-lock`);
  const temp = `${destination}.${randomUUID()}.restore`;
  let jobLease: string | null = null;
  let current: Database | null = null;
  try {
    // readOnly must fail on a missing file rather than creating an empty DB.
    const source = new Database(file, { readOnly: true, timeout: 5000 });
    let decks: number;
    try {
      const integrity = source.prepare("PRAGMA integrity_check").all() as { integrity_check: string }[];
      if (integrity.some(r => r.integrity_check !== "ok")) throw new Error("Backup failed integrity_check");
      if (source.prepare("PRAGMA foreign_key_check").all().length) throw new Error("Backup has broken foreign keys");
      decks = (source.prepare("SELECT COUNT(*) n FROM decks").get() as { n: number }).n;
      mkdirSync(dirname(destination), { recursive: true });
      await backup(source, temp);
    } finally { source.close(); }
    // Schema validation/migration occurs on the copy; never on the source.
    const validated = openDb(temp);
    try {
      const lock = validated.prepare("SELECT value FROM meta WHERE key = 'job:lock'").get();
      if (lock) validated.exec("DELETE FROM meta WHERE key = 'job:lock'");
      validated.exec(`UPDATE meta SET value = json_set(value, '$.result', 'error', '$.error', 'Interrupted before restore', '$.finished_at', datetime('now'))
        WHERE key IN ('job:refresh', 'job:backup') AND json_extract(value, '$.result') = 'running'`);
      validated.exec("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE");
    } finally { validated.close(); }
    // Refuse a concurrent CLI writer, too, and checkpoint its WAL before swap.
    if (statExists(destination)) {
      current = openDb(destination);
      acquireJobLock(current);
      jobLease = (current.prepare("SELECT value FROM meta WHERE key = 'job:lock'").get() as { value: string }).value;
      const checkpoint = current.prepare("PRAGMA wal_checkpoint(TRUNCATE)").get() as { busy: number };
      if (checkpoint.busy) throw new Error("Database is busy; stop all clients before restoring");
      // Keep the lease in the old file until the atomic rename. A CLI that
      // already opened it still sees the lease and cannot start a new job.
      current.close(); current = null;
    }
    renameSync(temp, destination);
    jobLease = null;
    for (const suffix of ["-wal", "-shm"]) rmSync(`${destination}${suffix}`, { force: true });
    return { decks };
  } finally {
    try {
      if (jobLease) {
        if (!current) current = openDb(destination);
        current.prepare("DELETE FROM meta WHERE key = 'job:lock' AND value = ?").run(jobLease);
      }
    } finally {
      try { current?.close(); }
      finally {
        try { for (const suffix of ["", "-wal", "-shm"]) rmSync(`${temp}${suffix}`, { force: true }); }
        finally { release(); }
      }
    }
  }
}
function statExists(path: string) {
  try { statSync(path); return true; } catch (e: any) { if (e.code === "ENOENT") return false; throw e; }
}
