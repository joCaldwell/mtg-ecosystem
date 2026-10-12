import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { SCHEMA_VERSION, SchemaError, openDb } from "../src/db.ts";
import { proposeDocumentEdit, listDocumentEdits } from "../src/documents.ts";
import { getPower, parseTarget } from "../src/deck/power.ts";
import { AGENT } from "./helpers.ts";
import { OWNER } from "../src/actor.ts";
import { createDeck, createSlot, createTag, getDeck, organizeCard, setDeckCategory, shortlistCard } from "../src/deck/service.ts";
import { undo } from "../src/deck/membership.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { add, cut } from "./helpers.ts";

const dir = mkdtempSync(join(tmpdir(), "deck-builder-db-"));
test.after(() => rmSync(dir, { recursive: true, force: true }));

describe("opening a database", () => {
  test("a new file gets the schema and its version, and reopens", () => {
    const path = join(dir, "new.db");
    openDb(path).close();
    const db = openDb(path);
    const row = db.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as { value: string };
    assert.equal(row.value, String(SCHEMA_VERSION));
    assert.ok(!db.prepare("PRAGMA table_info(deck_cards)").all().some((c: any) => c.name === "in_paper"));
    db.close();
  });
  test("schema 5 discards assembly marks while preserving cards, tags and historical undo", () => {
    const path = join(dir, "v5.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const deck = createDeck(old, "Existing assembled deck", { status: "active" });
    setDeckCategory(old, OWNER, deck, "paper");
    const slot = createSlot(old, deck, "Ramp");
    const tag = createTag(old, deck, "Need to get");
    add(old, deck, "id-forest", { quantity: 3 });
    add(old, deck, "id-solring", { slotId: slot });
    organizeCard(old, deck, "id-solring", { tagIds: [tag] });
    shortlistCard(old, deck, "id-counterspell", "Later.");
    organizeCard(old, deck, "id-counterspell", { tagIds: [tag] });
    const entry = cut(old, deck, "id-solring")!;
    // Recreate v5's obsolete column and a true mark in a cut snapshot.
    old.exec("ALTER TABLE deck_cards ADD COLUMN in_paper INTEGER NOT NULL DEFAULT 0");
    old.prepare("UPDATE deck_cards SET in_paper = 1 WHERE deck_id = ?").run(deck);
    old.prepare("UPDATE log_cards SET before_json = json_set(before_json, '$.in_paper', 1) WHERE log_id = ?").run(entry);
    old.exec("UPDATE meta SET value = '5' WHERE key = 'schema_version'");
    const before = getDeck(old, deck);
    old.close();

    const migrated = openDb(path);
    assert.deepEqual(getDeck(migrated, deck), before, "no card fields, tags, category or revision change");
    assert.ok(!migrated.prepare("PRAGMA table_info(deck_cards)").all().some((c: any) => c.name === "in_paper"));
    const snapshot = migrated.prepare("SELECT before_json FROM log_cards WHERE log_id = ?").get(entry) as { before_json: string };
    assert.ok(!snapshot.before_json.includes("in_paper"));
    undo(migrated, OWNER, entry);
    const restored = getDeck(migrated, deck).cards.find((c) => c.oracle_id === "id-solring")!;
    assert.equal(restored.slot_id, slot);
    assert.deepEqual(restored.tag_ids, [tag]);
    assert.equal("in_paper" in restored, false);
    assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(), []);
    migrated.close();
    const reopened = openDb(path);
    assert.equal(getDeck(reopened, deck).cards.length, 3);
    assert.deepEqual(getDeck(reopened, deck).tags.map(t => ({ ...t })), [{ id: tag, name: "Need to get" }], "migration creates no replacement tag");
    reopened.close();
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
    assert.throws(() => openDb(path), new RegExp(`reports schema 99; this version uses ${SCHEMA_VERSION}`));
  });
  test("schema 4 preserves document edits and treats missing Game Changer metadata as unknown", () => {
    const path = join(dir, "v4.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const id = createDeck(old, "Power migration", { status: "active" });
    add(old, id, "id-solring");
    const edit = proposeDocumentEdit(old, AGENT, id, "thesis", { content: "Existing thesis" }, "Owner requested it.");
    old.exec("UPDATE sqlite_sequence SET seq = 100 WHERE name = 'document_edits'; ALTER TABLE cards DROP COLUMN game_changer; DROP TABLE power_assessments; DROP TABLE power_targets; UPDATE meta SET value = '4' WHERE key = 'schema_version'");
    old.close();
    const migrated = openDb(path);
    const power = getPower(migrated, id);
    assert.equal(power.target, null);
    assert.equal(power.assessment, null);
    assert.equal(power.game_changers.unknown_cards, 1);
    assert.equal(listDocumentEdits(migrated, { deckId: id })[0].id, edit.edit_id);
    assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(), []);
    const proposed = proposeDocumentEdit(migrated, AGENT, id, "power_target", { target: parseTarget({ profile: {} }) }, "Set a target.");
    assert.equal(proposed.edit_id, 101);
    migrated.close();
    const reopened = openDb(path);
    assert.equal(listDocumentEdits(reopened, { deckId: id }).length, 2);
    reopened.close();
  });
  test("schema 2 migrates without losing decks and keeps classification on reopen", () => {
    const path = join(dir, "v2.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const id = createDeck(old, "Existing deck", { status: "active" });
    add(old, id, "id-solring");
    const before = getDeck(old, id);
    old.exec("ALTER TABLE decks DROP COLUMN category; UPDATE meta SET value = '2' WHERE key = 'schema_version'");
    old.close();

    const migrated = openDb(path);
    const deck = getDeck(migrated, id).deck;
    assert.equal(deck.name, "Existing deck");
    assert.equal(deck.status, "active");
    assert.equal(deck.category, "theorycrafted");
    assert.equal(deck.revision, before.deck.revision);
    assert.deepEqual(getDeck(migrated, id).cards, before.cards);
    assert.equal((migrated.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as { value: string }).value, String(SCHEMA_VERSION));
    assert.throws(() => migrated.prepare("UPDATE decks SET category = 'unknown' WHERE id = ?").run(id), /CHECK/);
    setDeckCategory(migrated, OWNER, id, "legacy");
    migrated.close();

    const reopened = openDb(path);
    assert.equal(getDeck(reopened, id).deck.category, "legacy");
    reopened.close();
  });
  test("schema 3 preserves assignments and adds deck-scoped tag foreign keys", () => {
    const path = join(dir, "v3.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const deck = createDeck(old, "Tagged deck");
    const other = createDeck(old, "Other deck");
    const tag = createTag(old, deck, "Engine");
    const foreign = createTag(old, other, "Engine");
    add(old, deck, "id-solring");
    organizeCard(old, deck, "id-solring", { tagIds: [tag] });
    const before = getDeck(old, deck);
    // Recreate the actual v3 join table, which only constrained the tag ID.
    old.exec(`CREATE TABLE legacy_tags (
      deck_id INTEGER NOT NULL, oracle_id TEXT NOT NULL,
      tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
      PRIMARY KEY (deck_id, oracle_id, tag_id),
      FOREIGN KEY (deck_id, oracle_id) REFERENCES deck_cards(deck_id, oracle_id) ON DELETE CASCADE
    );
    INSERT INTO legacy_tags SELECT * FROM deck_card_tags;
    DROP TABLE deck_card_tags;
    ALTER TABLE legacy_tags RENAME TO deck_card_tags;
    DROP INDEX idx_tags_deck_id;
    UPDATE meta SET value = '3' WHERE key = 'schema_version';`);
    old.close();

    const migrated = openDb(path);
    assert.deepEqual(getDeck(migrated, deck), before);
    assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(), []);
    assert.throws(() => migrated.prepare("INSERT INTO deck_card_tags VALUES (?, ?, ?)").run(deck, "id-solring", foreign), /FOREIGN KEY/);
    migrated.close();
    const reopened = openDb(path);
    assert.deepEqual(getDeck(reopened, deck), before);
    reopened.prepare("DELETE FROM tags WHERE id = ?").run(tag);
    assert.deepEqual(getDeck(reopened, deck).cards[0].tag_ids, []);
    reopened.close();
  });
  test("an invalid legacy tag link aborts migration without deleting assignments", () => {
    const path = join(dir, "invalid-tags.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const deck = createDeck(old, "Invalid tag deck");
    const other = createDeck(old, "Tag owner");
    const tag = createTag(old, other, "Engine");
    add(old, deck, "id-solring");
    old.exec(`DROP TABLE deck_card_tags;
      CREATE TABLE deck_card_tags (
        deck_id INTEGER NOT NULL, oracle_id TEXT NOT NULL,
        tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        PRIMARY KEY (deck_id, oracle_id, tag_id),
        FOREIGN KEY (deck_id, oracle_id) REFERENCES deck_cards(deck_id, oracle_id) ON DELETE CASCADE
      );
      UPDATE meta SET value = '3' WHERE key = 'schema_version';`);
    old.prepare("INSERT INTO deck_card_tags VALUES (?, ?, ?)").run(deck, "id-solring", tag);
    old.close();
    assert.throws(() => openDb(path), /FOREIGN KEY/);
    const unchanged = new DatabaseSync(path);
    assert.equal((unchanged.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as { value: string }).value, "3");
    assert.equal((unchanged.prepare("SELECT tag_id FROM deck_card_tags WHERE deck_id = ?").get(deck) as { tag_id: number }).tag_id, tag);
    assert.equal(unchanged.prepare("SELECT 1 FROM sqlite_master WHERE name = 'deck_card_tags_v4'").get(), undefined);
    unchanged.close();
  });
});
