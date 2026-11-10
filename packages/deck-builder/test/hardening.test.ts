import { after, test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { OWNER } from "./actors.ts";
import { backupDatabase, restoreDatabase } from "../src/backups.ts";
import { openDb } from "../src/db.ts";
import { createDeck } from "../src/deck/service.ts";
import { acquireProcessLock } from "../src/process-lock.ts";


const dir = mkdtempSync(join(tmpdir(), "db-hardening-"));
after(() => rmSync(dir, { recursive: true, force: true }));
test("backup captures committed WAL, retains 14 files, and restores a verified snapshot", async () => {
  const target = join(dir, "restore.db"), data = join(dir, "backups-test");
  const source = openDb(target); createDeck(source, OWNER, "Before backup");
  const { file } = await backupDatabase(source, data);
  createDeck(source, OWNER, "After backup"); source.close();
  const held = acquireProcessLock(`${target}.server-lock`);
  await assert.rejects(restoreDatabase(file, target), /running/); held();
  assert.deepEqual(await restoreDatabase(file, target), { decks: 1 });
  const restored = openDb(target);
  assert.equal((restored.prepare("SELECT COUNT(*) n FROM decks").get() as { n: number }).n, 1);
  restored.close();
  const corrupt = join(dir, "not-a-db"); writeFileSync(corrupt, "bad");
  await assert.rejects(restoreDatabase(corrupt, target));
  const stillThere = openDb(target); assert.equal((stillThere.prepare("SELECT name FROM decks").get() as { name: string }).name, "Before backup");
  for (let day = 1; day <= 16; day++) await backupDatabase(stillThere, data, new Date(Date.UTC(2026, 0, day)));
  stillThere.close();
  assert.equal(readdirSync(join(data, "backups")).filter(f => f.endsWith(".db")).length, 14);
});
