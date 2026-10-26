import type { Actor } from "./actor.ts";
import { Database } from "./sqlite.ts";
import { mkdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { AppError } from "./errors.ts";

export const PKG_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
export const DATA_DIR = resolve(process.env.DECKBUILDER_DATA ?? join(PKG_ROOT, "data"));
export const DEFAULT_DB_PATH = resolve(process.env.DECKBUILDER_DB ?? join(DATA_DIR, "deck-builder.db"));

export const SCHEMA_VERSION = 13;

const SCHEMA = `
-- Explicit physical evidence survives deck membership, forks and deletion.
-- NULL quantities mean unknown, never a count derived from deck appearances.
CREATE TABLE IF NOT EXISTS ownership_evidence (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  oracle_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('owned', 'unowned', 'unknown')),
  quantity INTEGER CHECK (quantity >= 0),
  available_quantity INTEGER CHECK (available_quantity >= 0),
  basis TEXT NOT NULL,
  author TEXT NOT NULL,
  source_deck_id INTEGER,
  source_deck_name TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_ownership_card ON ownership_evidence(oracle_id, id);
CREATE TABLE IF NOT EXISTS cards (
  oracle_id        TEXT PRIMARY KEY,
  name             TEXT NOT NULL,
  mana_cost        TEXT,
  cmc              REAL NOT NULL,
  type_line        TEXT NOT NULL,
  oracle_text      TEXT NOT NULL,
  power            TEXT,
  toughness        TEXT,
  loyalty          TEXT,
  power_num        REAL,
  toughness_num    REAL,
  loyalty_num      REAL,
  colors           TEXT NOT NULL,
  color_identity   TEXT NOT NULL,
  colors_mask      INTEGER NOT NULL,
  ci_mask          INTEGER NOT NULL,
  colors_count     INTEGER NOT NULL,
  ci_count         INTEGER NOT NULL,
  commander_legality TEXT NOT NULL,
  is_commander     INTEGER NOT NULL,
  game_changer     INTEGER CHECK (game_changer IN (0, 1)),
  layout           TEXT NOT NULL,
  faces_json       TEXT,
  image_small      TEXT,
  image_normal     TEXT,
  image_art_crop   TEXT,
  retired          INTEGER NOT NULL DEFAULT 0 CHECK (retired IN (0, 1)),
  -- oracle text minus reminder text, faces joined; what o: searches
  search_text      TEXT NOT NULL,
  -- oracle text including reminder text; what fo: searches
  full_search_text TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cards_name ON cards(name);

-- Exact-resolution table (spec §4.3): full name plus each face name,
-- normalized. Never used for fuzzy matching.
CREATE TABLE IF NOT EXISTS card_names (
  name_norm TEXT NOT NULL,
  oracle_id TEXT NOT NULL REFERENCES cards(oracle_id) ON DELETE CASCADE,
  PRIMARY KEY (name_norm, oracle_id)
);

CREATE VIRTUAL TABLE IF NOT EXISTS cards_fts USING fts5(
  name, search_text, oracle_id UNINDEXED
);

-- Bootstrap is intentionally the historical USD table: supported old files
-- run this idempotent schema before ordered migrations. v13 below creates the
-- current per-store shape for both new and migrated files.
-- Spec §4.4: the cheapest playable paper printing of each card, replaced
-- wholesale by every refresh. No foreign key: see ingestPrices.
CREATE TABLE IF NOT EXISTS card_prices (
  oracle_id        TEXT PRIMARY KEY,
  usd_cents        INTEGER NOT NULL,
  -- Which printing the price is for.
  set_code         TEXT NOT NULL,
  collector_number TEXT NOT NULL,
  finish           TEXT NOT NULL CHECK (finish IN ('nonfoil', 'foil', 'etched'))
);

CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS decks (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
  -- Spec §5.2 and §5.6: lifecycle controls editing. Only the owner changes it.
  -- Paper requires card proposals; Theorycrafted permits direct card edits;
  -- Legacy is read-only.
  category   TEXT NOT NULL DEFAULT 'theorycrafted'
             CHECK (category IN ('paper', 'theorycrafted', 'legacy')),
  -- Spec §5.3: the deck this one was forked from. Kept when the parent is
  -- deleted would dangle, so it is cleared instead.
  parent_id  INTEGER REFERENCES decks(id) ON DELETE SET NULL,
  -- Denormalized from the commanders. NULL = no commanders yet, meaning
  -- searches are unfiltered; 0 is a real (colorless) identity.
  ci_mask    INTEGER,
  color_identity TEXT NOT NULL DEFAULT '',
  -- Bumped once per applied membership change (spec §5.1).
  revision   INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS slots (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id    INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  name       TEXT NOT NULL,
  target_min INTEGER,
  target_max INTEGER,
  position   INTEGER NOT NULL DEFAULT 0,
  UNIQUE (deck_id, name COLLATE NOCASE)
);

CREATE TABLE IF NOT EXISTS tags (
  id      INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  name    TEXT NOT NULL,
  UNIQUE (deck_id, name COLLATE NOCASE)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_slots_deck_id ON slots(deck_id, id);

-- Composite parent key prevents a card from carrying another deck's tag.
CREATE UNIQUE INDEX IF NOT EXISTS idx_tags_deck_id ON tags(deck_id, id);

CREATE TABLE IF NOT EXISTS deck_cards (
  deck_id   INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  oracle_id TEXT NOT NULL REFERENCES cards(oracle_id),
  slot_id   INTEGER REFERENCES slots(id) ON DELETE SET NULL,
  role      TEXT NOT NULL DEFAULT 'card' CHECK (role IN ('card', 'commander', 'companion')),
  -- The maybe list (spec §5.4): considered for the deck but not in the 100.
  -- A flag rather than a role or a slot because it is orthogonal to both — a
  -- parked card keeps the slot it would fill and the tags describing it.
  maybe     INTEGER NOT NULL DEFAULT 0,
  -- Why a card was shortlisted. Free text, linted on write (spec §10.5).
  maybe_note TEXT NOT NULL DEFAULT '',
  quantity  INTEGER NOT NULL DEFAULT 1 CHECK (quantity >= 1),
  added_at  TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (deck_id, oracle_id)
);

CREATE INDEX IF NOT EXISTS idx_deck_cards_card ON deck_cards(oracle_id);

-- slot_id is the primary purpose; additional purposes belong to the same
-- deck and survive parking, forks and membership undo (spec §5.4).
CREATE TABLE IF NOT EXISTS deck_card_secondary_slots (
  deck_id INTEGER NOT NULL,
  oracle_id TEXT NOT NULL,
  slot_id INTEGER NOT NULL,
  PRIMARY KEY (deck_id, oracle_id, slot_id),
  FOREIGN KEY (deck_id, slot_id) REFERENCES slots(deck_id, id) ON DELETE CASCADE,
  FOREIGN KEY (deck_id, oracle_id) REFERENCES deck_cards(deck_id, oracle_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_secondary_slots_slot ON deck_card_secondary_slots(deck_id, slot_id, oracle_id);
CREATE TRIGGER IF NOT EXISTS secondary_slot_not_primary
BEFORE INSERT ON deck_card_secondary_slots
WHEN EXISTS (SELECT 1 FROM deck_cards WHERE deck_id = NEW.deck_id AND oracle_id = NEW.oracle_id AND slot_id = NEW.slot_id)
BEGIN SELECT RAISE(ABORT, 'A primary slot cannot also be secondary'); END;
CREATE TRIGGER IF NOT EXISTS primary_slot_not_secondary
BEFORE UPDATE OF slot_id ON deck_cards
WHEN EXISTS (SELECT 1 FROM deck_card_secondary_slots WHERE deck_id = NEW.deck_id AND oracle_id = NEW.oracle_id AND slot_id = NEW.slot_id)
BEGIN SELECT RAISE(ABORT, 'A primary slot cannot also be secondary'); END;
CREATE TRIGGER IF NOT EXISTS secondary_slot_update_not_primary
BEFORE UPDATE ON deck_card_secondary_slots
WHEN EXISTS (SELECT 1 FROM deck_cards WHERE deck_id = NEW.deck_id AND oracle_id = NEW.oracle_id AND slot_id = NEW.slot_id)
BEGIN SELECT RAISE(ABORT, 'A primary slot cannot also be secondary'); END;

CREATE TABLE IF NOT EXISTS deck_card_tags (
  deck_id   INTEGER NOT NULL,
  oracle_id TEXT NOT NULL,
  tag_id    INTEGER NOT NULL,
  PRIMARY KEY (deck_id, oracle_id, tag_id),
  FOREIGN KEY (deck_id, tag_id) REFERENCES tags(deck_id, id) ON DELETE CASCADE,
  FOREIGN KEY (deck_id, oracle_id) REFERENCES deck_cards(deck_id, oracle_id) ON DELETE CASCADE
);

-- The brief (spec §6): thesis and constraints freeform, engines structured.
CREATE TABLE IF NOT EXISTS briefs (
  deck_id        INTEGER PRIMARY KEY REFERENCES decks(id) ON DELETE CASCADE,
  thesis         TEXT NOT NULL DEFAULT '',
  constraints_md TEXT NOT NULL DEFAULT '',
  updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS engines (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id     INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  UNIQUE (deck_id, name COLLATE NOCASE)
);

-- Pieces are card references so "is every piece present" is a check (§9).
CREATE TABLE IF NOT EXISTS engine_pieces (
  engine_id INTEGER NOT NULL REFERENCES engines(id) ON DELETE CASCADE,
  oracle_id TEXT NOT NULL REFERENCES cards(oracle_id),
  note      TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (engine_id, oracle_id)
);

-- The player profile (spec §6): one row, the owner's document.
CREATE TABLE IF NOT EXISTS profile (
  id         INTEGER PRIMARY KEY CHECK (id = 1),
  body_md    TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Agent-proposed edits to either document (spec §6). deck_id is NULL for the
-- profile. The owner sees a diff and rules on it.
CREATE TABLE IF NOT EXISTS document_edits (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id      INTEGER REFERENCES decks(id) ON DELETE CASCADE,
  kind         TEXT NOT NULL CHECK (kind IN ('profile', 'thesis', 'constraints', 'engine_set', 'engine_remove', 'power_target')),
  payload_json TEXT NOT NULL,
  reason       TEXT NOT NULL,
  author       TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
  created_at   TEXT NOT NULL DEFAULT (datetime('now')),
  resolved_at  TEXT
);

-- Spec §5.7: the target is an owner's document; assessments retain attribution
-- and the exact deck revision. Neither changes deck membership or revision.
CREATE TABLE IF NOT EXISTS power_targets (
  deck_id INTEGER PRIMARY KEY REFERENCES decks(id) ON DELETE CASCADE,
  target_json TEXT NOT NULL,
  author TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS power_assessments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  profile_json TEXT NOT NULL,
  game_changers_json TEXT NOT NULL,
  revision INTEGER NOT NULL,
  evidence TEXT NOT NULL CHECK (evidence IN ('list', 'goldfish', 'games')),
  confidence TEXT NOT NULL CHECK (confidence IN ('tentative', 'supported')),
  notes TEXT NOT NULL,
  author TEXT NOT NULL,
  owner_touched INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_power_assessments_deck ON power_assessments(deck_id, id);

CREATE TABLE IF NOT EXISTS proposals (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id    INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  summary    TEXT NOT NULL DEFAULT '',
  author     TEXT NOT NULL,
  status     TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS proposal_items (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  proposal_id INTEGER NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
  action      TEXT NOT NULL CHECK (action IN ('add', 'cut')),
  oracle_id   TEXT NOT NULL REFERENCES cards(oracle_id),
  slot_id     INTEGER,
  secondary_slot_ids_json TEXT,
  -- Spec §8.1: an accepted cut moves the card to the maybe list instead of
  -- out of the deck. Only ever set on a cut.
  to_maybe    INTEGER NOT NULL DEFAULT 0 CHECK (to_maybe IN (0, 1)),
  reason      TEXT NOT NULL,
  group_id    TEXT,
  status      TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
  resolved_at TEXT
);

-- The rulings log (spec §8.2) is the version history. One row per act: a
-- ruling on one or more proposal items, a direct edit, an import, an undo.
-- The cards an act touched are in log_cards, so "what has been decided about
-- this card in this deck" (§8.3) is one indexed query whatever the act was.
CREATE TABLE IF NOT EXISTS log (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  -- NULL only for rulings on profile edits, which belong to no deck.
  deck_id       INTEGER REFERENCES decks(id) ON DELETE CASCADE,
  -- Deck revision after the act (unchanged by rejections).
  revision      INTEGER NOT NULL DEFAULT 0,
  kind          TEXT NOT NULL CHECK (kind IN ('accept', 'reject', 'edit', 'import', 'undo')),
  -- What the act was, in one line. For rulings on document edits this names
  -- the edit; for imports and batch edits it summarizes the change.
  summary       TEXT NOT NULL DEFAULT '',
  -- The owner's optional free-text reason for a ruling (spec §8.2).
  ruling_reason TEXT NOT NULL DEFAULT '',
  proposal_id   INTEGER,
  document_edit_id INTEGER,
  author        TEXT NOT NULL,
  undo_of       INTEGER REFERENCES log(id),
  undone_by     INTEGER REFERENCES log(id),
  -- Spec §8.4: unset until an agent has read this entry.
  reviewed_at   TEXT,
  ts            TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_log_deck ON log(deck_id, id);
CREATE INDEX IF NOT EXISTS idx_log_unreviewed ON log(reviewed_at) WHERE reviewed_at IS NULL;

CREATE TABLE IF NOT EXISTS log_cards (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  log_id     INTEGER NOT NULL REFERENCES log(id) ON DELETE CASCADE,
  deck_id    INTEGER NOT NULL,
  oracle_id  TEXT NOT NULL,
  card_name  TEXT NOT NULL,
  -- The change in question. For a rejection it is what was proposed and not
  -- done; for every other kind it is what was applied.
  action     TEXT NOT NULL CHECK (action IN ('add', 'cut', 'role')),
  quantity   INTEGER NOT NULL DEFAULT 1,
  item_id    INTEGER,
  -- The proposer's reason for this item, kept beside the ruling.
  reason     TEXT NOT NULL DEFAULT '',
  -- Undo payload: for a cut, how the row looked before; for a role change,
  -- the role it had.
  before_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_log_cards_card ON log_cards(deck_id, oracle_id);
CREATE INDEX IF NOT EXISTS idx_log_cards_log ON log_cards(log_id);

-- Memories (spec §7). Rows, not files, so the app can attach one to a card
-- or a deck and surface it without the agent going looking.
CREATE TABLE IF NOT EXISTS memories (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  text          TEXT NOT NULL,
  scope         TEXT NOT NULL CHECK (scope IN ('global', 'deck')),
  deck_id       INTEGER REFERENCES decks(id) ON DELETE CASCADE,
  basis         TEXT NOT NULL CHECK (basis IN ('stated', 'inferred')),
  author        TEXT NOT NULL,
  -- Set once the owner has written or edited this memory. An agent cannot
  -- edit, supersede or retire it after that (spec §7.3).
  owner_touched INTEGER NOT NULL DEFAULT 0,
  source_log_id INTEGER,
  deck_revision INTEGER,
  status        TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'superseded', 'retired')),
  superseded_by INTEGER REFERENCES memories(id),
  created_at    TEXT NOT NULL DEFAULT (datetime('now')),
  revised_at    TEXT NOT NULL DEFAULT (datetime('now')),
  CHECK ((scope = 'deck') = (deck_id IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories(scope, deck_id, status);

CREATE TABLE IF NOT EXISTS memory_cards (
  memory_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
  oracle_id TEXT NOT NULL REFERENCES cards(oracle_id),
  PRIMARY KEY (memory_id, oracle_id)
);

CREATE INDEX IF NOT EXISTS idx_memory_cards_card ON memory_cards(oracle_id);

CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
  text, memory_id UNINDEXED
);

-- One resumable solo table per deck; no completed-game archive.
CREATE TABLE IF NOT EXISTS playtest_sessions (
  deck_id INTEGER PRIMARY KEY REFERENCES decks(id) ON DELETE CASCADE,
  payload_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS playtest_observations (
  deck_id INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  request_id TEXT NOT NULL,
  memory_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
  input_json TEXT NOT NULL,
  PRIMARY KEY(deck_id, request_id)
);

-- Every agent write, approved or not (spec §11): the audit trail for writes
-- that needed no approval.
CREATE TABLE IF NOT EXISTS activity (
  id      INTEGER PRIMARY KEY AUTOINCREMENT,
  ts      TEXT NOT NULL DEFAULT (datetime('now')),
  actor   TEXT NOT NULL,
  action  TEXT NOT NULL,
  deck_id INTEGER,
  summary TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_activity_ts ON activity(id);
`;

/** Thrown when the file on disk is not a supported schema version. */
export class SchemaError extends AppError {}

const MIGRATIONS: Array<{ version: number; up: (db: Database) => void }> = [
  { version: 3, up(db) {
    // Preserve existing decks; classification is
    // an owner's decision, so do not infer it from assembly progress.
    db.exec(`ALTER TABLE decks ADD COLUMN category TEXT NOT NULL DEFAULT 'theorycrafted'
             CHECK (category IN ('paper', 'theorycrafted', 'legacy'))`);

  } },
  { version: 4, up(db) {
    // Rebuild only the join table; valid assignments retain their IDs. An
    // invalid legacy link fails the transaction rather than silently losing data.
    db.exec(`CREATE TABLE deck_card_tags_v4 (
      deck_id INTEGER NOT NULL,
      oracle_id TEXT NOT NULL,
      tag_id INTEGER NOT NULL,
      PRIMARY KEY (deck_id, oracle_id, tag_id),
      FOREIGN KEY (deck_id, tag_id) REFERENCES tags(deck_id, id) ON DELETE CASCADE,
      FOREIGN KEY (deck_id, oracle_id) REFERENCES deck_cards(deck_id, oracle_id) ON DELETE CASCADE
    );
    INSERT INTO deck_card_tags_v4 SELECT deck_id, oracle_id, tag_id FROM deck_card_tags;
    DROP TABLE deck_card_tags;
    ALTER TABLE deck_card_tags_v4 RENAME TO deck_card_tags;`);

  } },
  { version: 5, up(db) {
    // NULL until a card refresh supplies Scryfall's optional metadata.
    const columns = db.prepare("PRAGMA table_info(cards)").all() as unknown as { name: string }[];
    if (!columns.some(c => c.name === "game_changer"))
      db.exec("ALTER TABLE cards ADD COLUMN game_changer INTEGER CHECK (game_changer IN (0, 1))");
    const sequence = db.prepare("SELECT seq FROM sqlite_sequence WHERE name = 'document_edits'").get() as { seq: number } | undefined;
    db.exec(`CREATE TABLE document_edits_v5 (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      deck_id INTEGER REFERENCES decks(id) ON DELETE CASCADE,
      kind TEXT NOT NULL CHECK (kind IN ('profile', 'thesis', 'constraints', 'engine_set', 'engine_remove', 'power_target')),
      payload_json TEXT NOT NULL, reason TEXT NOT NULL, author TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
      created_at TEXT NOT NULL DEFAULT (datetime('now')), resolved_at TEXT
    );
    INSERT INTO document_edits_v5 SELECT id, deck_id, kind, payload_json, reason, author, status, created_at, resolved_at FROM document_edits;
    DROP TABLE document_edits;
    ALTER TABLE document_edits_v5 RENAME TO document_edits;`);
    // Deleted decks may have consumed higher IDs than the surviving rows.
    // Keep AUTOINCREMENT monotonic so old references cannot name a new edit.
    if (sequence) {
      db.prepare("DELETE FROM sqlite_sequence WHERE name = 'document_edits'").run();
      db.prepare("INSERT INTO sqlite_sequence (name, seq) VALUES ('document_edits', ?)").run(sequence.seq);
    }

  } },
  { version: 6, up: removeAssemblyMarks },
  { version: 7, up: addMaybeDestination },
  { version: 8, up: addSecondaryPurposes },
  { version: 9, up: removeDeckStatus },
  { version: 10, up(db) {
    normalizeV10(db);
    // Only the explicit Have convention is ownership evidence. Keep/Ordered
    // and deck membership say nothing about physical ownership or quantity.
    db.exec(`INSERT INTO ownership_evidence
      (oracle_id, status, basis, author, source_deck_id, source_deck_name)
      SELECT DISTINCT ct.oracle_id, 'owned', 'Migrated explicit Have tag; quantity unknown',
        'migration', d.id, d.name
      FROM deck_card_tags ct JOIN tags t ON t.id = ct.tag_id
      JOIN decks d ON d.id = ct.deck_id WHERE lower(trim(t.name)) = 'have'`);

  } },
  { version: 11, up(db) { normalizeV10(db); migrateIdentity(db); } },
  { version: 12, up(db) {
    // Generated with auth@1.7.13 from our plugin config, never auto-migrated.
    db.exec(readFileSync(new URL("./migrations/auth-v12.sql", import.meta.url), "utf8"));
    db.exec(`ALTER TABLE proposals ADD COLUMN author_oauth_client_id TEXT;
      ALTER TABLE document_edits ADD COLUMN author_oauth_client_id TEXT;
      ALTER TABLE users ADD COLUMN needs_handle INTEGER NOT NULL DEFAULT 0;
      CREATE TABLE invites (
        code TEXT PRIMARY KEY, created_by INTEGER NOT NULL REFERENCES users(id),
        used_by INTEGER REFERENCES users(id), used_at TEXT,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
      );
      CREATE TABLE signup_intents (
        id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK(kind IN ('claim','invite','open')),
        code_hash TEXT NOT NULL, expires_at INTEGER NOT NULL
      );
      CREATE TABLE oauth_usage (
        user_id INTEGER NOT NULL REFERENCES users(id), client_id TEXT NOT NULL,
        label TEXT NOT NULL, last_used_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY(user_id, client_id)
      );`);
  } },
  { version: 13, up(db) {
    db.exec(`ALTER TABLE card_prices RENAME TO card_prices_usd;
      CREATE TABLE card_prices (
        oracle_id TEXT NOT NULL, store TEXT NOT NULL CHECK(store IN ('tcgplayer','cardmarket')),
        currency TEXT NOT NULL CHECK((store = 'tcgplayer' AND currency = 'USD') OR (store = 'cardmarket' AND currency = 'EUR')),
        cents INTEGER NOT NULL CHECK(cents >= 0), set_code TEXT NOT NULL, collector_number TEXT NOT NULL,
        finish TEXT NOT NULL CHECK(finish IN ('nonfoil','foil','etched')), PRIMARY KEY(oracle_id,store)
      );
      INSERT INTO card_prices SELECT oracle_id,'tcgplayer','USD',usd_cents,set_code,collector_number,finish FROM card_prices_usd;
      DROP TABLE card_prices_usd;
      CREATE INDEX idx_card_prices_currency ON card_prices(currency,store,cents);`);
  } },
];

function removeDeckStatus(db: Database) {
  const deckColumns = db.prepare("PRAGMA table_info(decks)").all() as unknown as { name: string }[];
  if (deckColumns.some(c => c.name === "status")) db.exec("ALTER TABLE decks DROP COLUMN status");
}
function removeAssemblyMarks(db: Database) {
  const cardColumns = db.prepare("PRAGMA table_info(deck_cards)").all() as unknown as { name: string }[];
  if (cardColumns.some((c) => c.name === "in_paper")) {
    // The owner chose to discard assembly marks and use ordinary tags.
    // Keep historical membership/filing snapshots usable for undo.
    db.exec(`ALTER TABLE deck_cards DROP COLUMN in_paper;
      UPDATE log_cards SET before_json = json_remove(before_json, '$.in_paper')
      WHERE json_valid(before_json) AND json_type(before_json, '$.in_paper') IS NOT NULL;`);
  }
}
function addMaybeDestination(db: Database) {
  addColumn(db, "proposal_items", "to_maybe", "INTEGER NOT NULL DEFAULT 0 CHECK (to_maybe IN (0, 1))");
}
function addSecondaryPurposes(db: Database) {
  addColumn(db, "proposal_items", "secondary_slot_ids_json", "TEXT");
}
function addColumn(db: Database, table: string, name: string, type: string) {
  const columns = db.prepare(`PRAGMA table_info(${table})`).all() as { name: string }[];
  if (!columns.some(c => c.name === name)) db.exec(`ALTER TABLE ${table} ADD COLUMN ${name} ${type}`);
}
function normalizeV10(db: Database) {
  // Added by presence, not by version: an existing file gains the column
  // the first time it is opened, whatever version it reports.
  // Category now owns permissions. Preserve each existing category and remove
  // the redundant flag; SQLite keeps IDs, foreign keys and dependent records.
  removeDeckStatus(db);
  addMaybeDestination(db);
  addSecondaryPurposes(db);
  const facts = db.prepare("PRAGMA table_info(cards)").all() as unknown as { name: string }[];
  for (const [name, type] of Object.entries({
    image_small: "TEXT", image_normal: "TEXT", image_art_crop: "TEXT",
    retired: "INTEGER NOT NULL DEFAULT 0 CHECK (retired IN (0, 1))",
  })) if (!facts.some(c => c.name === name)) db.exec(`ALTER TABLE cards ADD COLUMN ${name} ${type}`);
  removeAssemblyMarks(db);
  db.exec("CREATE INDEX IF NOT EXISTS idx_deck_card_tags_tag ON deck_card_tags(deck_id, tag_id, oracle_id)");
  for (const table of ["proposals", "document_edits"]) {
    const columns = db.prepare(`PRAGMA table_info(${table})`).all() as unknown as { name: string }[];
    for (const [name, type] of Object.entries({
      base_revision: "INTEGER", disposition: "TEXT NOT NULL DEFAULT 'current'",
      superseded_by: "INTEGER", ...(table === "document_edits" ? { group_id: "INTEGER" } : {}),
    })) if (!columns.some(c => c.name === name)) db.exec(`ALTER TABLE ${table} ADD COLUMN ${name} ${type}`);
  }
  // Older proposals lacked a baseline. Anchor them once, then preserve it.
  for (const table of ["proposals", "document_edits"])
    db.exec(`UPDATE ${table} SET base_revision = (SELECT revision FROM decks WHERE id = ${table}.deck_id)
      WHERE base_revision IS NULL AND deck_id IS NOT NULL`);
}

function migrateIdentity(db: Database) {
  db.exec(`CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    handle TEXT NOT NULL UNIQUE COLLATE NOCASE CHECK (length(handle) BETWEEN 3 AND 24 AND handle NOT GLOB '*[^a-z0-9_-]*'),
    display_name TEXT NOT NULL, auth_id TEXT UNIQUE,
    is_admin INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')), deleted_at TEXT
  );
  INSERT INTO users (id, handle, display_name, is_admin) VALUES (1, 'owner', 'Owner', 1);
  CREATE TABLE decks_v11 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    name TEXT NOT NULL COLLATE NOCASE,
    category TEXT NOT NULL DEFAULT 'theorycrafted' CHECK (category IN ('paper','theorycrafted','legacy')),
    parent_id INTEGER REFERENCES decks(id) ON DELETE SET NULL,
    ci_mask INTEGER, color_identity TEXT NOT NULL DEFAULT '', revision INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')), UNIQUE(user_id, name)
  );`);
  const sequence = db.prepare("SELECT seq FROM sqlite_sequence WHERE name = 'decks'").get() as { seq: number } | undefined;
  db.exec(`INSERT INTO decks_v11 SELECT id, 1, name, category, parent_id, ci_mask, color_identity, revision, created_at FROM decks;
    DROP TABLE decks;
    ALTER TABLE decks_v11 RENAME TO decks;`);
  if (sequence) db.prepare("UPDATE sqlite_sequence SET seq = ? WHERE name = 'decks'").run(sequence.seq);
  for (const table of ["memories", "document_edits", "log", "activity", "ownership_evidence"]) {
    db.exec(`ALTER TABLE ${table} ADD COLUMN user_id INTEGER NOT NULL DEFAULT 1 REFERENCES users(id);
      CREATE INDEX idx_${table}_user ON ${table}(user_id);`);
  }
  for (const table of ["memories", "document_edits", "log", "ownership_evidence", "power_targets", "power_assessments", "proposals"]) {
    db.exec(`ALTER TABLE ${table} ADD COLUMN author_user_id INTEGER NOT NULL DEFAULT 1 REFERENCES users(id);
      ALTER TABLE ${table} ADD COLUMN author_via TEXT NOT NULL DEFAULT 'ui' CHECK (author_via IN ('ui','agent'));
      ALTER TABLE ${table} ADD COLUMN author_label TEXT NOT NULL DEFAULT 'ui';
      UPDATE ${table} SET author_via = CASE WHEN author IN ('owner','owner (from v1)','migration') THEN 'ui' ELSE 'agent' END,
        author_label = CASE WHEN author IN ('owner','owner (from v1)') THEN 'ui' ELSE author END;`);
  }
  db.exec(`ALTER TABLE activity ADD COLUMN actor_user_id INTEGER NOT NULL DEFAULT 1 REFERENCES users(id);
    ALTER TABLE activity ADD COLUMN actor_via TEXT NOT NULL DEFAULT 'agent' CHECK (actor_via IN ('ui','agent'));
    ALTER TABLE activity ADD COLUMN actor_label TEXT NOT NULL DEFAULT 'agent';
    UPDATE activity SET actor_label = actor;
    CREATE TABLE profile_v11 (
      user_id INTEGER PRIMARY KEY REFERENCES users(id), body_md TEXT NOT NULL DEFAULT '',
      updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    INSERT INTO profile_v11 SELECT 1, body_md, updated_at FROM profile;
    INSERT OR IGNORE INTO profile_v11(user_id) VALUES (1);
    DROP TABLE profile; ALTER TABLE profile_v11 RENAME TO profile;
    CREATE TABLE playtest_sessions_v11 (
      deck_id INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
      user_id INTEGER NOT NULL REFERENCES users(id), payload_json TEXT NOT NULL,
      PRIMARY KEY(deck_id, user_id)
    );
    INSERT INTO playtest_sessions_v11 SELECT deck_id, 1, payload_json FROM playtest_sessions;
    DROP TABLE playtest_sessions; ALTER TABLE playtest_sessions_v11 RENAME TO playtest_sessions;
    CREATE TABLE user_settings (
      user_id INTEGER NOT NULL REFERENCES users(id), key TEXT NOT NULL, value TEXT NOT NULL,
      PRIMARY KEY(user_id, key)
    );
    INSERT INTO user_settings SELECT 1, substr(key, 9), value FROM meta WHERE key LIKE 'setting:%';
    DELETE FROM meta WHERE key LIKE 'setting:%';
    CREATE TABLE agent_tokens (
      id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL REFERENCES users(id),
      label TEXT NOT NULL, prefix TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE,
      created_at TEXT NOT NULL DEFAULT (datetime('now')), last_used_at TEXT, revoked_at TEXT
    );
    CREATE INDEX idx_agent_tokens_prefix ON agent_tokens(prefix);
    CREATE INDEX idx_agent_tokens_user ON agent_tokens(user_id);
    ALTER TABLE proposals ADD COLUMN author_token_id INTEGER REFERENCES agent_tokens(id);
    ALTER TABLE document_edits ADD COLUMN author_token_id INTEGER REFERENCES agent_tokens(id);
  `);
}

export function openDb(path: string = DEFAULT_DB_PATH): Database {
  if (path !== ":memory:") mkdirSync(dirname(path), { recursive: true });
  const db = new Database(path, { timeout: 5000 });
  try {
    // Reject unrelated files before changing journal mode. This is validation
    // only: the version used for migration is re-read under the write lock.
    assertOurs(db, path);
    db.exec("PRAGMA journal_mode = WAL");
    // Rebuilding referenced tables must not cascade deletes. Check all FKs
    // before committing, then re-enable enforcement for normal operations.
    // https://www.sqlite.org/lang_altertable.html#making_other_kinds_of_table_schema_changes
    db.exec("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = ON");
    db.exec("BEGIN IMMEDIATE");
    try {
      const version = assertOurs(db, path);
      if (version !== String(SCHEMA_VERSION)) {
        db.exec(SCHEMA);
        for (const step of MIGRATIONS) {
          if (step.version > Number(version ?? 10)) step.up(db);
        }
        const broken = db.prepare("PRAGMA foreign_key_check").all() as { table: string; rowid: number | null; parent: string }[];
        if (broken.length) {
          const first = broken.slice(0, 5).map(({ table, rowid, parent }) => ({ table, rowid, parent }));
          throw new SchemaError(`Migration found broken FOREIGN KEY references (first ${first.length} of ${broken.length}): ${JSON.stringify(first)}; all changes were rolled back`);
        }
        db.prepare("INSERT INTO meta (key, value) VALUES ('schema_version', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
          .run(String(SCHEMA_VERSION));
      }
      db.exec("COMMIT");
    } catch (error) { db.exec("ROLLBACK"); throw error; }
    db.exec("PRAGMA legacy_alter_table = OFF; PRAGMA foreign_keys = ON");
  } catch (error) { db.close(); throw error; }
  return db;
}

// The schema is created with IF NOT EXISTS, so pointed at a database with a
// different shape it would add its tables beside the others and leave a file
// nothing can read. A file that already holds tables must say it is this
// version (or a version we can migrate); otherwise this refuses before writing anything.
function assertOurs(db: Database, path: string) {
  const tables = db.prepare("SELECT COUNT(*) n FROM sqlite_master WHERE type = 'table'").get() as {
    n: number;
  };
  if (!tables.n) return null; // a new file
  let found: string | null = null;
  try {
    found =
      (db.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as
        | { value: string }
        | undefined)?.value ?? null;
  } catch {
    /* no meta table at all */
  }
  if (found === String(SCHEMA_VERSION) || found === "2" || found === "3" || found === "4" || found === "5" || found === "6" || found === "7" || found === "8" || found === "9" || found === "10" || found === "11" || found === "12") return found;
  throw new SchemaError(
    `${path} is not a database this version of the deck builder can open ` +
      `(it reports schema ${found ?? "unknown"}; this version uses ${SCHEMA_VERSION}). ` +
      `It was left untouched. Set DECKBUILDER_DB to another file to start fresh.`,
  );
}

// Savepoints, not BEGIN, so domain operations compose: a withTransaction
// function may freely call another one (BEGIN cannot nest and throws). An
// inner failure rolls back only the inner writes; whether the outer work
// survives is decided by whoever catches the error.
const txDepth = new WeakMap<Database, number>();

export function withTransaction<T>(db: Database, fn: () => T): T {
  const depth = txDepth.get(db) ?? 0;
  const name = `tx_${depth}`;
  db.exec(`SAVEPOINT ${name}`);
  txDepth.set(db, depth + 1);
  try {
    const result = fn();
    db.exec(`RELEASE ${name}`);
    return result;
  } catch (e) {
    // ROLLBACK TO undoes the writes but keeps the savepoint on the stack;
    // RELEASE pops it so the connection is clean for the next caller.
    db.exec(`ROLLBACK TO ${name}`);
    db.exec(`RELEASE ${name}`);
    throw e;
  } finally {
    txDepth.set(db, depth);
  }
}

// ---------- settings (per-user key/value) ----------

export function getSetting(db: Database, actor: Actor, key: string): string | null {
  const row = db.prepare("SELECT value FROM user_settings WHERE user_id = ? AND key = ?").get(actor.user_id, key) as
    | { value: string }
    | undefined;
  return row?.value ?? null;
}

export function setSetting(db: Database, actor: Actor, key: string, value: string): void {
  db.prepare(
    "INSERT INTO user_settings (user_id, key, value) VALUES (?, ?, ?) ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value",
  ).run(actor.user_id, key, value);
}

export function normalizeName(name: string): string {
  return name.trim().toLowerCase().normalize("NFC");
}
