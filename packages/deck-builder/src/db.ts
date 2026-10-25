import { Database } from "./sqlite.ts";
import { mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { AppError } from "./errors.ts";

export const PKG_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
export const DATA_DIR = resolve(process.env.DECKBUILDER_DATA ?? join(PKG_ROOT, "data"));
export const DEFAULT_DB_PATH = resolve(process.env.DECKBUILDER_DB ?? join(DATA_DIR, "deck-builder.db"));

export const SCHEMA_VERSION = 10;

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

CREATE INDEX IF NOT EXISTS idx_card_prices_usd ON card_prices(usd_cents);

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

export function openDb(path: string = DEFAULT_DB_PATH): Database {
  if (path !== ":memory:") mkdirSync(dirname(path), { recursive: true });
  const db = new Database(path, { timeout: 5000 });
  const version = assertOurs(db, path);
  db.exec("PRAGMA journal_mode = WAL");
  db.exec("PRAGMA foreign_keys = ON");
  try {
    withTransaction(db, () => {
      db.exec(SCHEMA);
      if (version === "2") {
        // Preserve existing decks; classification is
        // an owner's decision, so do not infer it from assembly progress.
        db.exec(`ALTER TABLE decks ADD COLUMN category TEXT NOT NULL DEFAULT 'theorycrafted'
                 CHECK (category IN ('paper', 'theorycrafted', 'legacy'))`);
      }
      if (version === "2" || version === "3") {
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
      }
      if (version != null && Number(version) < 5) {
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
      }
      // Added by presence, not by version: an existing file gains the column
      // the first time it is opened, whatever version it reports.
      // Category now owns permissions. Preserve each existing category and remove
      // the redundant flag; SQLite keeps IDs, foreign keys and dependent records.
      const deckColumns = db.prepare("PRAGMA table_info(decks)").all() as unknown as { name: string }[];
      if (deckColumns.some(c => c.name === "status")) db.exec("ALTER TABLE decks DROP COLUMN status");
      const itemColumns = db.prepare("PRAGMA table_info(proposal_items)").all() as unknown as { name: string }[];
      if (!itemColumns.some((c) => c.name === "to_maybe"))
        db.exec("ALTER TABLE proposal_items ADD COLUMN to_maybe INTEGER NOT NULL DEFAULT 0 CHECK (to_maybe IN (0, 1))");
      if (!itemColumns.some((c) => c.name === "secondary_slot_ids_json"))
        db.exec("ALTER TABLE proposal_items ADD COLUMN secondary_slot_ids_json TEXT");
      const facts = db.prepare("PRAGMA table_info(cards)").all() as unknown as { name: string }[];
      for (const [name, type] of Object.entries({
        image_small: "TEXT", image_normal: "TEXT", image_art_crop: "TEXT",
        retired: "INTEGER NOT NULL DEFAULT 0 CHECK (retired IN (0, 1))",
      })) if (!facts.some(c => c.name === name)) db.exec(`ALTER TABLE cards ADD COLUMN ${name} ${type}`);
      const cardColumns = db.prepare("PRAGMA table_info(deck_cards)").all() as unknown as { name: string }[];
      if (cardColumns.some((c) => c.name === "in_paper")) {
        // The owner chose to discard assembly marks and use ordinary tags.
        // Keep historical membership/filing snapshots usable for undo.
        db.exec(`ALTER TABLE deck_cards DROP COLUMN in_paper;
          UPDATE log_cards SET before_json = json_remove(before_json, '$.in_paper')
          WHERE json_valid(before_json) AND json_type(before_json, '$.in_paper') IS NOT NULL;`);
      }
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
      if (version != null && Number(version) < 10) {
        // Only the explicit Have convention is ownership evidence. Keep/Ordered
        // and deck membership say nothing about physical ownership or quantity.
        db.exec(`INSERT INTO ownership_evidence
          (oracle_id, status, basis, author, source_deck_id, source_deck_name)
          SELECT DISTINCT ct.oracle_id, 'owned', 'Migrated explicit Have tag; quantity unknown',
            'migration', d.id, d.name
          FROM deck_card_tags ct JOIN tags t ON t.id = ct.tag_id
          JOIN decks d ON d.id = ct.deck_id WHERE lower(trim(t.name)) = 'have'`);
      }
      db.prepare("INSERT INTO meta (key, value) VALUES ('schema_version', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value").run(
        String(SCHEMA_VERSION),
      );
      db.prepare("INSERT OR IGNORE INTO profile (id) VALUES (1)").run();
    });
  } catch (error) {
    db.close();
    throw error;
  }
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
  if (found === String(SCHEMA_VERSION) || found === "2" || found === "3" || found === "4" || found === "5" || found === "6" || found === "7" || found === "8" || found === "9") return found;
  db.close();
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

// ---------- settings (meta key/value) ----------

export function getSetting(db: Database, key: string): string | null {
  const row = db.prepare("SELECT value FROM meta WHERE key = ?").get(`setting:${key}`) as
    | { value: string }
    | undefined;
  return row?.value ?? null;
}

export function setSetting(db: Database, key: string, value: string): void {
  db.prepare(
    "INSERT INTO meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
  ).run(`setting:${key}`, value);
}

export function normalizeName(name: string): string {
  return name.trim().toLowerCase().normalize("NFC");
}
