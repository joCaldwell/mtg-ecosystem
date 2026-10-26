import { backup, Database } from "./sqlite.ts";
import { createReadStream, mkdirSync, readdirSync, renameSync, rmSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { randomUUID } from "node:crypto";
import { DATA_DIR, DEFAULT_DB_PATH, openDb } from "./db.ts";
import { acquireJobLock } from "./jobs.ts";
import { acquireProcessLock } from "./process-lock.ts";

type BucketObject = { Key?: string; LastModified?: Date };
type BackupStore = { upload(file: string, key: string): Promise<void>; list(): Promise<BucketObject[]>; remove(keys: string[]): Promise<void>; close(): void };

// The newest snapshot of each of the latest 14 UTC days, plus the newest
// snapshot of each of the latest six UTC calendar months. Only our backup
// namespace is managed; archive/ and unrelated objects are never touched.
function expiredBackupKeys(objects: BucketObject[], now: Date): string[] {
  const snapshots = objects.flatMap(object => {
    const match = object.Key?.match(/^backups\/deck-builder-(\d{4}-\d{2}-\d{2})(?:T(\d{2}-\d{2}-\d{2}-\d{3})Z-[\da-f-]+)?\.db$/);
    if (!match) return [];
    const date = match[1], stamp = match[2] ? `${date}T${match[2].slice(0, 8).replaceAll("-", ":")}.${match[2].slice(9)}Z` : `${date}T00:00:00.000Z`;
    return [{ key: object.Key!, date, month: date.slice(0, 7), stamp }];
  }).sort((a, b) => b.stamp.localeCompare(a.stamp) || b.key.localeCompare(a.key));
  const keep = new Set<string>();
  const today = now.toISOString().slice(0, 10);
  const oldestDay = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() - 13)).toISOString().slice(0, 10);
  const oldestMonth = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() - 5, 1)).toISOString().slice(0, 7);
  const days = new Set<string>(), months = new Set<string>();
  for (const s of snapshots) {
    if (s.date > today) { keep.add(s.key); continue; } // don't prune future-dated objects after a clock correction
    if (s.date >= oldestDay && !days.has(s.date)) { keep.add(s.key); days.add(s.date); }
    if (s.month >= oldestMonth && !months.has(s.month)) { keep.add(s.key); months.add(s.month); }
  }
  return snapshots.filter(s => !keep.has(s.key)).map(s => s.key);
}

async function s3Store(): Promise<BackupStore> {
  const { S3Client, PutObjectCommand, ListObjectsV2Command, DeleteObjectsCommand } = await import("@aws-sdk/client-s3");
  const Bucket = process.env.BACKUP_S3_BUCKET!;
  const client = new S3Client({ endpoint: process.env.BACKUP_S3_ENDPOINT, region: "auto", forcePathStyle: true,
    credentials: { accessKeyId: process.env.BACKUP_S3_ACCESS_KEY!, secretAccessKey: process.env.BACKUP_S3_SECRET_KEY! } });
  return {
    async upload(file, Key) { await client.send(new PutObjectCommand({ Bucket, Key, Body: createReadStream(file), ContentLength: statSync(file).size, ContentType: "application/vnd.sqlite3", IfNoneMatch: "*" })); },
    async list() {
      const objects: BucketObject[] = []; let ContinuationToken: string | undefined;
      do {
        const page = await client.send(new ListObjectsV2Command({ Bucket, Prefix: "backups/", ContinuationToken }));
        objects.push(...page.Contents ?? []);
        ContinuationToken = page.IsTruncated ? page.NextContinuationToken : undefined;
        if (page.IsTruncated && !ContinuationToken) throw new Error("Backup listing was truncated without a continuation token");
      } while (ContinuationToken);
      return objects;
    },
    async remove(keys) {
      for (let i = 0; i < keys.length; i += 1000) {
        const result = await client.send(new DeleteObjectsCommand({ Bucket, Delete: { Objects: keys.slice(i, i + 1000).map(Key => ({ Key })), Quiet: true } }));
        if (result.Errors?.length) throw new Error("Backup retention deletion failed");
      }
    },
    close() { client.destroy(); },
  };
}

/** Online SQLite backup includes committed WAL content. Never copy the .db. */
export async function backupDatabase(db: Database, dataDir = DATA_DIR, now = new Date(), store?: BackupStore) {
  const dir = join(dataDir, "backups");
  mkdirSync(dir, { recursive: true });
  const file = join(dir, `deck-builder-${now.toISOString().replaceAll(":", "-").replace(".", "-")}-${randomUUID()}.db`);
  const temp = `${file}.${randomUUID()}.tmp`;
  try {
    // https://nodejs.org/api/sqlite.html#sqlitebackupsourceDb-path-options
    await backup(db, temp);
    renameSync(temp, file);
  } finally { rmSync(temp, { force: true }); }
  const names = readdirSync(dir).filter(name => /^deck-builder-\d{4}-\d{2}-\d{2}(?:T[\d-]+Z-[\da-f-]+)?\.db$/.test(name) && name !== file.split("/").at(-1)).sort().reverse();
  for (const old of names.slice(13)) rmSync(join(dir, old));
  const keys = ["BACKUP_S3_ENDPOINT", "BACKUP_S3_BUCKET", "BACKUP_S3_ACCESS_KEY", "BACKUP_S3_SECRET_KEY"] as const;
  if (store || keys.some(k => process.env[k])) {
    if (!store && !keys.every(k => process.env[k])) throw new Error(`S3 backup needs all of ${keys.join(", ")}`);
    // https://docs.aws.amazon.com/sdk-for-javascript/v3/developer-guide/javascript_s3_code_examples.html
    const client = store ?? await s3Store();
    try {
      const key = `backups/${file.split("/").at(-1)}`;
      await client.upload(file, key);
      // Retention runs only after a durable upload and complete listing.
      // Always protect this upload, even if a newer snapshot already exists.
      const expired = expiredBackupKeys(await client.list(), now).filter(old => old !== key);
      if (expired.length) await client.remove(expired);
    } finally { client.close(); }
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
