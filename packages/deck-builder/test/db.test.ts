import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { SCHEMA_VERSION, SchemaError, openDb } from "../src/db.ts";

const dir = mkdtempSync(join(tmpdir(), "deck-builder-db-"));
test.after(() => rmSync(dir, { recursive: true, force: true }));

describe("opening a database", () => {
  test("a new file gets the schema and its version, and reopens", () => {
    const path = join(dir, "new.db");
    openDb(path).close();
    const db = openDb(path);
    const row = db.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as { value: string };
    assert.equal(row.value, String(SCHEMA_VERSION));
    db.close();
  });
  // The schema is CREATE IF NOT EXISTS: without this check it would add its
  // tables beside a stranger's and leave a file nothing can read.
  test("a file with another shape is refused and left byte-for-byte as it was", () => {
    const path = join(dir, "other.db");
    const other = new DatabaseSync(path);
    other.exec("CREATE TABLE decks (id INTEGER PRIMARY KEY, name TEXT); INSERT INTO decks (name) VALUES ('mine');");
    other.close();
    const before = readFileSync(path);
    assert.throws(() => openDb(path), (e: Error) => e instanceof SchemaError && /left untouched/.test(e.message));
    assert.deepEqual(readFileSync(path), before);
  });
  test("a file from another schema version is refused too", () => {
    const path = join(dir, "future.db");
    openDb(path).close();
    const db = new DatabaseSync(path);
    db.exec("UPDATE meta SET value = '99' WHERE key = 'schema_version'");
    db.close();
    assert.throws(() => openDb(path), /reports schema 99; this version uses 2/);
  });
});
