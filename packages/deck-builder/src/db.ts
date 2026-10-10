import { DatabaseSync } from "node:sqlite";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { AppError } from "./errors.ts";

export const PKG_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
export const DATA_DIR = process.env.DECKBUILDER_DATA ?? join(PKG_ROOT, "data");
export const DEFAULT_DB_PATH = process.env.DECKBUILDER_DB ?? join(DATA_DIR, "deck-builder.db");

export const SCHEMA_VERSION = 2;

const SCHEMA = `
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
  layout           TEXT NOT NULL,
  faces_json       TEXT,
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

CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS decks (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
  -- Spec §5.2. Agents change membership of a draft directly and of an active
  -- deck only by proposal. Only the owner flips this.
  status     TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active')),
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
  -- Spec §5.5: the physical card is in the physical deck. Not "owned".
  in_paper  INTEGER NOT NULL DEFAULT 0,
  quantity  INTEGER NOT NULL DEFAULT 1 CHECK (quantity >= 1),
  added_at  TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (deck_id, oracle_id)
);

CREATE INDEX IF NOT EXISTS idx_deck_cards_card ON deck_cards(oracle_id);

CREATE TABLE IF NOT EXISTS deck_card_tags (
  deck_id   INTEGER NOT NULL,
  oracle_id TEXT NOT NULL,
  tag_id    INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (deck_id, oracle_id, tag_id),
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
  kind         TEXT NOT NULL CHECK (kind IN ('profile', 'thesis', 'constraints', 'engine_set', 'engine_remove')),
  payload_json TEXT NOT NULL,
  reason       TEXT NOT NULL,
  author       TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
  created_at   TEXT NOT NULL DEFAULT (datetime('now')),
  resolved_at  TEXT
);

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

/** Thrown when the file on disk was not made by this schema version. */
export class SchemaError extends AppError {}

export function openDb(path: string = DEFAULT_DB_PATH): DatabaseSync {
  if (path !== ":memory:") mkdirSync(dirname(path), { recursive: true });
  const db = new DatabaseSync(path);
  assertOurs(db, path);
  db.exec("PRAGMA journal_mode = WAL");
  db.exec("PRAGMA foreign_keys = ON");
  db.exec(SCHEMA);
  db.prepare("INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', ?)").run(
    String(SCHEMA_VERSION),
  );
  db.prepare("INSERT OR IGNORE INTO profile (id) VALUES (1)").run();
  return db;
}

// The schema is created with IF NOT EXISTS, so pointed at a database with a
// different shape it would add its tables beside the others and leave a file
// nothing can read. A file that already holds tables must say it is this
// version; otherwise this refuses before writing anything.
function assertOurs(db: DatabaseSync, path: string) {
  const tables = db.prepare("SELECT COUNT(*) n FROM sqlite_master WHERE type = 'table'").get() as {
    n: number;
  };
  if (!tables.n) return; // a new file
  let found: string | null = null;
  try {
    found =
      (db.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as
        | { value: string }
        | undefined)?.value ?? null;
  } catch {
    /* no meta table at all */
  }
  if (found === String(SCHEMA_VERSION)) return;
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
const txDepth = new WeakMap<DatabaseSync, number>();

export function withTransaction<T>(db: DatabaseSync, fn: () => T): T {
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

export function getSetting(db: DatabaseSync, key: string): string | null {
  const row = db.prepare("SELECT value FROM meta WHERE key = ?").get(`setting:${key}`) as
    | { value: string }
    | undefined;
  return row?.value ?? null;
}

export function setSetting(db: DatabaseSync, key: string, value: string): void {
  db.prepare(
    "INSERT INTO meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
  ).run(`setting:${key}`, value);
}

export function normalizeName(name: string): string {
  return name.trim().toLowerCase().normalize("NFC");
}
