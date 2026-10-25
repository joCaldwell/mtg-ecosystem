import { removeIdentity } from "./legacy-db.ts";
import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Database } from "../src/sqlite.ts";
import { SCHEMA_VERSION, SchemaError, openDb } from "../src/db.ts";
import { proposeDocumentEdit, listDocumentEdits } from "../src/documents.ts";
import { getPower, parseTarget } from "../src/deck/power.ts";
import { AGENT } from "./helpers.ts";
import { OWNER } from "./actors.ts";
import { createDeck, createSlot, createTag, getDeck, organizeCard, setDeckCategory, shortlistCard } from "../src/deck/service.ts";
import { undo } from "../src/deck/membership.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { add, cut } from "./helpers.ts";

const dir = mkdtempSync(join(tmpdir(), "deck-builder-db-"));
test.after(() => rmSync(dir, { recursive: true, force: true }));

describe("opening a database", () => {
  test("schema 8 drops Draft/Active and preserves categories, IDs and related records", () => {
    const path = join(dir, "v8-lifecycle.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const ids = (["paper", "theorycrafted", "legacy"] as const).map((category, index) => {
      const id = createDeck(old, OWNER, `Lifecycle ${index}`);
      const slot = createSlot(old, OWNER, id, "Utility");
      const tag = createTag(old, OWNER, id, "Have");
      add(old, id, "id-solring", { slotId: slot });
      organizeCard(old, OWNER, id, "id-solring", { tagIds: [tag] });
      setDeckCategory(old, OWNER, id, category);
      return id;
    });
    const before = ids.map(id => getDeck(old, OWNER, id));
    old.exec("ALTER TABLE decks ADD COLUMN status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('draft', 'active')); UPDATE meta SET value = '8' WHERE key = 'schema_version'");
    old.prepare("UPDATE decks SET status = 'draft' WHERE id = ?").run(ids[0]);
    removeIdentity(old);
    old.close();
    const migrated = openDb(path);
    assert.deepEqual(ids.map(id => getDeck(migrated, OWNER, id)), before);
    assert.ok(!migrated.prepare("PRAGMA table_info(decks)").all().some((c: any) => c.name === "status"));
    assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(), []);
    assert.ok(createDeck(migrated, OWNER, "Next lifecycle deck") > ids[2]);
    migrated.close();
    const reopened = openDb(path);
    assert.deepEqual(ids.map(id => getDeck(reopened, OWNER, id)), before);
    reopened.close();
  });
  test("schema 7 gains secondary purposes without changing existing decks or historical undo", () => {
    const path = join(dir, "v7.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const deck = createDeck(old, OWNER, "Existing purposes");
    const primary = createSlot(old, OWNER, deck, "Ramp", 1, 2);
    const tag = createTag(old, OWNER, deck, "Have");
    add(old, deck, "id-solring", { slotId: primary });
    organizeCard(old, OWNER, deck, "id-solring", { tagIds: [tag] });
    const cutId = cut(old, deck, "id-solring")!;
    const before = getDeck(old, OWNER, deck).deck;
    old.prepare("UPDATE log_cards SET before_json = json_remove(before_json, '$.secondary_slot_ids') WHERE log_id = ?").run(cutId);
    old.exec(`DROP TRIGGER primary_slot_not_secondary;
      DROP TABLE deck_card_secondary_slots;
      ALTER TABLE proposal_items DROP COLUMN secondary_slot_ids_json;
      UPDATE meta SET value = '7' WHERE key = 'schema_version'`);
    removeIdentity(old);
    old.close();
    const migrated = openDb(path);
    assert.deepEqual(getDeck(migrated, OWNER, deck).deck, before);
    undo(migrated, OWNER, cutId);
    const card = getDeck(migrated, OWNER, deck).cards[0];
    assert.equal(card.slot_id, primary);
    assert.deepEqual(card.tag_ids, [tag]);
    assert.deepEqual(card.secondary_slot_ids, []);
    assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(), []);
    migrated.close();
    openDb(path).close();
  });
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
    const deck = createDeck(old, OWNER, "Existing assembled deck", { category: "paper" });
    setDeckCategory(old, OWNER, deck, "paper");
    const slot = createSlot(old, OWNER, deck, "Ramp");
    const tag = createTag(old, OWNER, deck, "Need to get");
    add(old, deck, "id-forest", { quantity: 3 });
    add(old, deck, "id-solring", { slotId: slot });
    organizeCard(old, OWNER, deck, "id-solring", { tagIds: [tag] });
    shortlistCard(old, OWNER, deck, "id-counterspell", "Later.");
    organizeCard(old, OWNER, deck, "id-counterspell", { tagIds: [tag] });
    const entry = cut(old, deck, "id-solring")!;
    // Recreate v5's obsolete column and a true mark in a cut snapshot.
    old.exec("ALTER TABLE deck_cards ADD COLUMN in_paper INTEGER NOT NULL DEFAULT 0");
    old.prepare("UPDATE deck_cards SET in_paper = 1 WHERE deck_id = ?").run(deck);
    old.prepare("UPDATE log_cards SET before_json = json_set(before_json, '$.in_paper', 1) WHERE log_id = ?").run(entry);
    old.exec("UPDATE meta SET value = '5' WHERE key = 'schema_version'");
    const before = getDeck(old, OWNER, deck);
    removeIdentity(old);
    old.close();

    const migrated = openDb(path);
    assert.deepEqual(getDeck(migrated, OWNER, deck), before, "no card fields, tags, category or revision change");
    assert.ok(!migrated.prepare("PRAGMA table_info(deck_cards)").all().some((c: any) => c.name === "in_paper"));
    const snapshot = migrated.prepare("SELECT before_json FROM log_cards WHERE log_id = ?").get(entry) as { before_json: string };
    assert.ok(!snapshot.before_json.includes("in_paper"));
    undo(migrated, OWNER, entry);
    const restored = getDeck(migrated, OWNER, deck).cards.find((c) => c.oracle_id === "id-solring")!;
    assert.equal(restored.slot_id, slot);
    assert.deepEqual(restored.tag_ids, [tag]);
    assert.equal("in_paper" in restored, false);
    assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(), []);
    migrated.close();
    const reopened = openDb(path);
    assert.equal(getDeck(reopened, OWNER, deck).cards.length, 3);
    assert.deepEqual(getDeck(reopened, OWNER, deck).tags.map(t => ({ ...t })), [{ id: tag, name: "Need to get" }], "migration creates no replacement tag");
    reopened.close();
  });
  // The schema is CREATE IF NOT EXISTS: without this check it would add its
  // tables beside a stranger's and leave a file nothing can read.
  test("a file with another shape is refused and left byte-for-byte as it was", () => {
    const path = join(dir, "other.db");
    const other = new Database(path);
    other.exec("CREATE TABLE decks (id INTEGER PRIMARY KEY, name TEXT); INSERT INTO decks (name) VALUES ('mine');");
    other.close();
    const before = readFileSync(path);
    assert.throws(() => openDb(path), (e: Error) => e instanceof SchemaError && /left untouched/.test(e.message));
    assert.deepEqual(readFileSync(path), before);
  });
  test("a file from another schema version is refused too", () => {
    const path = join(dir, "future.db");
    openDb(path).close();
    const db = new Database(path);
    db.exec("UPDATE meta SET value = '99' WHERE key = 'schema_version'");
    db.close();
    assert.throws(() => openDb(path), new RegExp(`reports schema 99; this version uses ${SCHEMA_VERSION}`));
  });
  test("schema 4 preserves document edits and treats missing Game Changer metadata as unknown", () => {
    const path = join(dir, "v4.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const id = createDeck(old, OWNER, "Power migration", { category: "paper" });
    add(old, id, "id-solring");
    const edit = proposeDocumentEdit(old, AGENT, id, "thesis", { content: "Existing thesis" }, "Owner requested it.");
    old.exec("UPDATE sqlite_sequence SET seq = 100 WHERE name = 'document_edits'; ALTER TABLE cards DROP COLUMN game_changer; DROP TABLE power_assessments; DROP TABLE power_targets; UPDATE meta SET value = '4' WHERE key = 'schema_version'");
    removeIdentity(old);
    old.close();
    const migrated = openDb(path);
    const power = getPower(migrated, OWNER, id);
    assert.equal(power.target, null);
    assert.equal(power.assessment, null);
    assert.equal(power.game_changers.unknown_cards, 1);
    assert.equal(listDocumentEdits(migrated, OWNER, { deckId: id })[0].id, edit.edit_id);
    assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(), []);
    const proposed = proposeDocumentEdit(migrated, AGENT, id, "power_target", { target: parseTarget({ profile: {} }) }, "Set a target.");
    assert.equal(proposed.edit_id, 101);
    migrated.close();
    const reopened = openDb(path);
    assert.equal(listDocumentEdits(reopened, OWNER, { deckId: id }).length, 2);
    reopened.close();
  });
  test("schema 2 migrates without losing decks and keeps classification on reopen", () => {
    const path = join(dir, "v2.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const id = createDeck(old, OWNER, "Existing deck", { category: "paper" });
    add(old, id, "id-solring");
    const before = getDeck(old, OWNER, id);
    old.exec("ALTER TABLE decks DROP COLUMN category; UPDATE meta SET value = '2' WHERE key = 'schema_version'");
    removeIdentity(old);
    old.close();

    const migrated = openDb(path);
    const deck = getDeck(migrated, OWNER, id).deck;
    assert.equal(deck.name, "Existing deck");
    assert.ok(!("status" in deck));
    assert.equal(deck.category, "theorycrafted");
    assert.equal(deck.revision, before.deck.revision);
    assert.deepEqual(getDeck(migrated, OWNER, id).cards, before.cards);
    assert.equal((migrated.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as { value: string }).value, String(SCHEMA_VERSION));
    assert.throws(() => migrated.prepare("UPDATE decks SET category = 'unknown' WHERE id = ?").run(id), /CHECK/);
    setDeckCategory(migrated, OWNER, id, "legacy");
    migrated.close();

    const reopened = openDb(path);
    assert.equal(getDeck(reopened, OWNER, id).deck.category, "legacy");
    reopened.close();
  });
  test("schema 3 preserves assignments and adds deck-scoped tag foreign keys", () => {
    const path = join(dir, "v3.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const deck = createDeck(old, OWNER, "Tagged deck");
    const other = createDeck(old, OWNER, "Other deck");
    const tag = createTag(old, OWNER, deck, "Engine");
    const foreign = createTag(old, OWNER, other, "Engine");
    add(old, deck, "id-solring");
    organizeCard(old, OWNER, deck, "id-solring", { tagIds: [tag] });
    const before = getDeck(old, OWNER, deck);
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
    removeIdentity(old);
    old.close();

    const migrated = openDb(path);
    assert.deepEqual(getDeck(migrated, OWNER, deck), before);
    assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(), []);
    assert.throws(() => migrated.prepare("INSERT INTO deck_card_tags VALUES (?, ?, ?)").run(deck, "id-solring", foreign), /FOREIGN KEY/);
    migrated.close();
    const reopened = openDb(path);
    assert.deepEqual(getDeck(reopened, OWNER, deck), before);
    reopened.prepare("DELETE FROM tags WHERE id = ?").run(tag);
    assert.deepEqual(getDeck(reopened, OWNER, deck).cards[0].tag_ids, []);
    reopened.close();
  });
  test("an invalid legacy tag link aborts migration without deleting assignments", () => {
    const path = join(dir, "invalid-tags.db");
    const old = openDb(path);
    ingestCards(old, FIXTURES);
    const deck = createDeck(old, OWNER, "Invalid tag deck");
    const other = createDeck(old, OWNER, "Tag owner");
    const tag = createTag(old, OWNER, other, "Engine");
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
    removeIdentity(old);
    old.close();
    assert.throws(() => openDb(path), /FOREIGN KEY/);
    const unchanged = new Database(path);
    assert.equal((unchanged.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as { value: string }).value, "3");
    assert.equal((unchanged.prepare("SELECT tag_id FROM deck_card_tags WHERE deck_id = ?").get(deck) as { tag_id: number }).tag_id, tag);
    assert.equal(unchanged.prepare("SELECT 1 FROM sqlite_master WHERE name = 'deck_card_tags_v4'").get(), undefined);
    unchanged.close();
  });
});

test("schema 9 migrates Have evidence and proposal bases conservatively, preserving IDs and rerunning safely", () => {
  const path = join(dir, "v9-sourcing-proposals.db");
  const old = openDb(path);
  ingestCards(old, FIXTURES);
  const id = createDeck(old, OWNER, "Old sourcing list");
  add(old, id, "id-solring");
  const have = createTag(old, OWNER, id, "Have");
  organizeCard(old, OWNER, id, "id-solring", { tagIds: [have] });
  const editId = proposeDocumentEdit(old, AGENT, id, "thesis", { content: "Pending plan" }, "Requested").edit_id;
  const before = getDeck(old, OWNER, id);
  old.exec(`DROP TABLE ownership_evidence;
    ALTER TABLE proposals DROP COLUMN base_revision;
    ALTER TABLE proposals DROP COLUMN disposition;
    ALTER TABLE proposals DROP COLUMN superseded_by;
    ALTER TABLE document_edits DROP COLUMN base_revision;
    ALTER TABLE document_edits DROP COLUMN disposition;
    ALTER TABLE document_edits DROP COLUMN superseded_by;
    ALTER TABLE document_edits DROP COLUMN group_id;
    UPDATE meta SET value = '9' WHERE key = 'schema_version'`);
  removeIdentity(old);
  old.close();
  const migrated = openDb(path);
  assert.deepEqual(getDeck(migrated, OWNER, id), before);
  const evidence = migrated.prepare("SELECT * FROM ownership_evidence").all() as any[];
  assert.equal(evidence.length, 1);
  assert.equal(evidence[0].status, "owned");
  assert.equal(evidence[0].quantity, null);
  assert.equal(evidence[0].available_quantity, null);
  const [edit] = listDocumentEdits(migrated, OWNER, { deckId: id });
  assert.equal(edit.id, editId);
  assert.equal(edit.base_revision, before.deck.revision);
  assert.equal(edit.stale, false);
  cut(migrated, id, "id-solring");
  assert.deepEqual(migrated.prepare("SELECT * FROM ownership_evidence").all(), evidence);
  assert.deepEqual(migrated.prepare("PRAGMA foreign_key_check").all(), []);
  migrated.close();
  const reopened = openDb(path);
  assert.deepEqual(reopened.prepare("SELECT * FROM ownership_evidence").all(), evidence);
  reopened.close();
});

test("v10 identity migration preserves data, sequences and structured authors, with tenant keys and settings", async () => {
  const { setProfile, getProfile } = await import("../src/documents.ts");
  const { remember, listMemories, setBudgets, getBudgets } = await import("../src/memory.ts");
  const { setFocus, getFocus } = await import("../src/focus.ts");
  const { listDecks } = await import("../src/deck/service.ts");
  const { createProposal } = await import("../src/deck/proposals.ts");
  const { startPlaytest, getPlaytest } = await import("../src/playtest/service.ts");
  const { setPowerTarget, recordPowerAssessment } = await import("../src/deck/power.ts");
  const path = join(dir, "identity-v10.db"), old = openDb(path);
  ingestCards(old, FIXTURES);
  const deck = createDeck(old, OWNER, "Existing tenant deck", { category: "paper" });
  add(old, deck, "id-forest", { quantity: 7 });
  const tag = createTag(old, OWNER, deck, "Have");
  organizeCard(old, OWNER, deck, "id-forest", { tagIds: [tag] });
  setProfile(old, OWNER, "Existing player profile");
  setBudgets(old, OWNER, { global: 1234 }); setFocus(old, OWNER, deck);
  remember(old, OWNER, { scope: "global", text: "Existing global preference" });
  remember(old, AGENT, { scope: "deck", deck_id: deck, text: "Existing deck preference" });
  proposeDocumentEdit(old, AGENT, null, "profile", { content: "Proposed profile" }, "Existing reason");
  createProposal(old, AGENT, deck, { items: [{ action: "add", oracle_id: "id-solring", reason: "Existing reason" }] });
  setPowerTarget(old, OWNER, deck, { profile: {} });
  recordPowerAssessment(old, AGENT, deck, { profile: {}, revision: 1, evidence: "list", confidence: "tentative", notes: "Existing assessment" });
  const session = startPlaytest(old, OWNER, deck, { mode: "practice" });
  const before = { deck: getDeck(old, OWNER, deck), profile: getProfile(old, OWNER), memories: listMemories(old, OWNER, { status: "any" }) };
  removeIdentity(old);
  old.exec("UPDATE meta SET value = '10' WHERE key = 'schema_version'; UPDATE sqlite_sequence SET seq = 100 WHERE name = 'decks'");
  assert.equal(old.prepare("SELECT 1 FROM sqlite_master WHERE name = 'users'").get(), undefined, "fixture is truly v10");
  old.close();
  const db = openDb(path);
  try {
    assert.equal((db.prepare("SELECT value FROM meta WHERE key='schema_version'").get() as { value: string }).value, "11");
    assert.deepEqual(getDeck(db, OWNER, deck), before.deck);
    assert.deepEqual(getProfile(db, OWNER), before.profile);
    assert.deepEqual(listMemories(db, OWNER, { status: "any" }), before.memories);
    assert.equal(getBudgets(db, OWNER).global, 1234); assert.equal(getFocus(db, OWNER)!.deck_id, deck);
    assert.deepEqual(getPlaytest(db, OWNER, deck), session);
    assert.equal((db.prepare("SELECT COUNT(*) n FROM meta WHERE key LIKE 'setting:%'").get() as { n:number }).n, 0);
    for (const table of ["decks", "memories", "document_edits", "log", "activity", "ownership_evidence", "profile", "playtest_sessions"]) {
      assert.equal((db.prepare(`SELECT COUNT(*) n FROM ${table} WHERE user_id != 1`).get() as { n: number }).n, 0, table);
    }
    for (const table of ["memories", "document_edits", "log", "ownership_evidence", "proposals", "power_targets", "power_assessments"]) {
      const rows = db.prepare(`SELECT author, author_user_id, author_via, author_label FROM ${table}`).all() as any[];
      assert.ok(rows.length, table);
      for (const row of rows) assert.deepEqual({ ...row }, { author: row.author, author_user_id: 1, author_via: row.author === "owner" ? "ui" : "agent", author_label: row.author === "owner" ? "ui" : row.author });
    }
    const columns = db.prepare("PRAGMA table_info(playtest_sessions)").all() as { name: string; pk: number }[];
    assert.deepEqual(columns.filter(c => c.pk).map(c => c.name), ["deck_id", "user_id"]);
    assert.ok(createDeck(db, OWNER, "Next monotonic deck") > 100);
    db.exec("INSERT INTO users (id,handle,display_name) VALUES (2,'second','Second')");
    const second = { ...OWNER, user_id: 2 };
    createDeck(db, second, "Existing tenant deck");
    assert.equal(listDecks(db, second).length, 1);
    assert.equal(listMemories(db, second).length, 0);
    assert.equal(getProfile(db, second).body_md, "");
    setProfile(db, second, "Second profile"); assert.equal(getProfile(db, OWNER).body_md, before.profile.body_md);
    assert.deepEqual(db.prepare("PRAGMA foreign_key_check").all(), []);
  } finally { db.close(); }
  openDb(path).close();
});

test("a v10 FK failure rolls back every identity table and the version together", () => {
  const path = join(dir, "identity-invalid.db"), old = openDb(path);
  removeIdentity(old);
  old.exec("PRAGMA foreign_keys = OFF; UPDATE meta SET value = '10' WHERE key = 'schema_version'; INSERT INTO memories(text,scope,deck_id,basis,author) VALUES ('Broken link','deck',999,'inferred','agent')");
  old.close();
  assert.throws(() => openDb(path), /FOREIGN KEY/);
  const unchanged = new Database(path);
  try {
    assert.equal((unchanged.prepare("SELECT value FROM meta WHERE key='schema_version'").get() as { value:string }).value, "10");
    assert.equal(unchanged.prepare("SELECT 1 FROM sqlite_master WHERE name = 'users'").get(), undefined);
    assert.ok(!(unchanged.prepare("PRAGMA table_info(memories)").all() as { name:string }[]).some(c => c.name === "user_id"));
    assert.equal((unchanged.prepare("SELECT deck_id FROM memories").get() as { deck_id:number }).deck_id, 999);
  } finally { unchanged.close(); }
});
