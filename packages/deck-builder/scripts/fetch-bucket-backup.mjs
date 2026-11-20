#!/usr/bin/env node
// Download one snapshot from a Railway backup bucket and verify it is a
// post-claim database before reporting success.
//
//   node packages/deck-builder/scripts/fetch-bucket-backup.mjs \
//     [--environment production] [--bucket backups-production] \
//     [--key backups/deck-builder-....db] [--out FILE]
//
// Without --key the newest `backups/deck-builder-*.db` object is chosen. Without
// --out the file goes to ~/.local/share/deck-builder-offline/<YYYY-MM>/. Bucket
// credentials come from the logged-in Railway CLI and stay in process memory;
// nothing is printed or deleted, and an existing local file is never replaced.
import { execFileSync } from "node:child_process";
import { chmodSync, createWriteStream, mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { pipeline } from "node:stream/promises";
import { parseArgs } from "node:util";
import { Database } from "node:sqlite";
import { GetObjectCommand, ListObjectsV2Command, S3Client } from "@aws-sdk/client-s3";

const { values: args } = parseArgs({
  options: {
    environment: { type: "string", default: "production" },
    bucket: { type: "string", default: "backups-production" },
    key: { type: "string" },
    out: { type: "string" },
  },
});

const credentials = JSON.parse(execFileSync("railway", [
  "bucket", "--environment", args.environment, "credentials", "--bucket", args.bucket, "--json",
], { encoding: "utf8" }));
const s3 = new S3Client({
  endpoint: credentials.endpoint,
  region: "auto",
  forcePathStyle: true,
  credentials: { accessKeyId: credentials.accessKeyId, secretAccessKey: credentials.secretAccessKey },
});

async function newestSnapshotKey() {
  const objects = [];
  let ContinuationToken;
  do {
    const page = await s3.send(new ListObjectsV2Command({ Bucket: credentials.bucketName, Prefix: "backups/", ContinuationToken }));
    objects.push(...page.Contents ?? []);
    ContinuationToken = page.IsTruncated ? page.NextContinuationToken : undefined;
  } while (ContinuationToken);
  const latest = objects
    .filter(object => /^backups\/deck-builder-\d{4}-\d{2}-\d{2}.*\.db$/.test(object.Key))
    .sort((a, b) => new Date(b.LastModified) - new Date(a.LastModified))[0];
  if (!latest) throw new Error("No snapshot found");
  return latest.Key;
}

try {
  const key = args.key ?? await newestSnapshotKey();
  const defaultDir = join(homedir(), ".local/share/deck-builder-offline", new Date().toISOString().slice(0, 7));
  const file = resolve(args.out ?? join(defaultDir, basename(key)));
  mkdirSync(dirname(file), { recursive: true, mode: 0o700 });
  if (!args.out) chmodSync(defaultDir, 0o700);
  const object = await s3.send(new GetObjectCommand({ Bucket: credentials.bucketName, Key: key }));
  await pipeline(object.Body, createWriteStream(file, { flags: "wx", mode: 0o600 }));

  const db = new Database(file, { readOnly: true });
  try {
    const integrity = db.prepare("pragma integrity_check").get().integrity_check;
    const owner = db.prepare("select auth_id from users where id = 1").get();
    if (integrity !== "ok" || !owner?.auth_id) throw new Error("Not a verified post-claim backup");
    const decks = db.prepare("select count(*) n from decks").get().n;
    console.log(JSON.stringify({ key, file, decks }));
  } finally {
    db.close();
  }
} finally {
  s3.destroy();
}
