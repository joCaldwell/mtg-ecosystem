// Pre-promotion online snapshot. Runs INSIDE the production container as the
// node user; the operator pipes it through `railway ssh` (see OPERATIONS.md).
// It reads SQLite read-only, refuses an unclaimed owner, writes a 0600 copy to
// /data/backups and uploads it to archive/pre-promotion/ with overwrite
// protection, then verifies the stored size. Errors print only a safe name/code.
// Keep this file free of single quotes and of a line consisting of "JS": it is
// embedded in a remote heredoc.
import { Database, backup } from "node:sqlite";
import { chmodSync, createReadStream, mkdirSync, statSync } from "node:fs";
import { randomUUID } from "node:crypto";
import { HeadObjectCommand, PutObjectCommand, S3Client } from "@aws-sdk/client-s3";

async function snapshot() {
  const db = new Database(process.env.DECKBUILDER_DB ?? "/data/deck-builder.db", { readOnly: true, timeout: 5000 });
  const stamp = new Date().toISOString().replaceAll(":", "-").replace(".", "-");
  const name = `deck-builder-${stamp}-${randomUUID()}.db`;
  const file = `/data/backups/${name}`;
  const Key = `archive/pre-promotion/${name}`;
  try {
    if (!db.prepare("select auth_id from users where id = 1").get()?.auth_id) throw new Error("Owner must be claimed");
    mkdirSync("/data/backups", { recursive: true });
    await backup(db, file);
    chmodSync(file, 0o600);
  } finally {
    db.close();
  }

  const s3 = new S3Client({
    endpoint: process.env.BACKUP_S3_ENDPOINT,
    region: "auto",
    forcePathStyle: true,
    credentials: { accessKeyId: process.env.BACKUP_S3_ACCESS_KEY, secretAccessKey: process.env.BACKUP_S3_SECRET_KEY },
  });
  try {
    const Bucket = process.env.BACKUP_S3_BUCKET;
    const size = statSync(file).size;
    await s3.send(new PutObjectCommand({ Bucket, Key, Body: createReadStream(file), ContentLength: size, IfNoneMatch: "*" }));
    const head = await s3.send(new HeadObjectCommand({ Bucket, Key }));
    if (head.ContentLength !== size) throw new Error("Backup size mismatch");
    console.log(JSON.stringify({ key: Key, verified_bytes: head.ContentLength }));
  } finally {
    s3.destroy();
  }
}

await snapshot().catch(error => {
  const safe = value => typeof value === "string" && /^[A-Z][A-Za-z0-9_.$-]{0,63}$/.test(value) ? value : undefined;
  console.error(JSON.stringify({ event: "pre_promotion_backup_error", name: safe(error.name) ?? "Error", code: safe(error.code) }));
  process.exitCode = 1;
});
