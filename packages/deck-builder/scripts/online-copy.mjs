#!/usr/bin/env node
// Copy a SQLite database through the online backup API without writing to the
// source. Usage: node scripts/online-copy.mjs SOURCE.db DEST.db
// The destination must not exist; its directory is created mode 0700 and the
// copy is left mode 0600. Never point DEST at a live data/ directory.
import { Database, backup } from "node:sqlite";
import { chmodSync, existsSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

const [source, dest] = process.argv.slice(2).map(path => path && resolve(path));
if (!source || !dest) {
  console.error("usage: node scripts/online-copy.mjs SOURCE.db DEST.db");
  process.exit(2);
}
if (existsSync(dest)) throw new Error(`Refusing to overwrite ${dest}`);
mkdirSync(dirname(dest), { recursive: true, mode: 0o700 });

const db = new Database(source, { readOnly: true, timeout: 5000 });
try {
  await backup(db, dest);
} finally {
  db.close();
}
chmodSync(dest, 0o600);
console.log(JSON.stringify({ copied: dest }));
