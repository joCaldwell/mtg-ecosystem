// The v1 database schema, verbatim from src/db.ts at commit 93538dc (the last
// v1 commit), so the migration is tested against the tables it will meet.
export const V1_SCHEMA = `
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

-- Exact-resolution table for [[Card Name]] linting: full name plus each
-- face name, normalized. Never used for fuzzy matching.
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
  -- Denormalized from the commanders (spec §3). NULL = no commanders yet,
  -- meaning searches are unfiltered; 0 is a real (colorless) identity.
  ci_mask    INTEGER,
  color_identity TEXT NOT NULL DEFAULT '',
  -- Bumped on every applied mutation (accepted item, undo, manual edit).
  revision   INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS proposals (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id    INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  source     TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'agent', 'audit', 'import')),
  note       TEXT NOT NULL DEFAULT '',
  status     TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS proposal_items (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  proposal_id INTEGER NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
  action      TEXT NOT NULL CHECK (action IN ('add', 'cut', 'maybe')),
  oracle_id   TEXT NOT NULL REFERENCES cards(oracle_id),
  slot_id     INTEGER,
  rationale   TEXT NOT NULL,
  group_id    TEXT,
  status      TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
  resolved_at TEXT
);

-- The decision log IS the version history (spec §7.4). Never compacted by
-- anything except the retention policy for what is *resident in context*.
CREATE TABLE IF NOT EXISTS decision_log (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id         INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  revision        INTEGER NOT NULL,
  kind            TEXT NOT NULL CHECK (kind IN ('accept', 'reject', 'undo', 'filter_removed', 'maybe_move')),
  action          TEXT,
  oracle_id       TEXT,
  card_name       TEXT,
  rationale       TEXT,
  rejection_type  TEXT CHECK (rejection_type IN ('hard_filter', 'thesis_change', 'playtest_finding', 'soft')),
  rejection_reason TEXT,
  proposal_id     INTEGER,
  item_id         INTEGER,
  undo_of         INTEGER REFERENCES decision_log(id),
  undone_by       INTEGER REFERENCES decision_log(id),
  snapshot_json   TEXT,
  brief_flag      INTEGER NOT NULL DEFAULT 0,
  ts              TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_log_deck ON decision_log(deck_id, id);
CREATE INDEX IF NOT EXISTS idx_log_card ON decision_log(deck_id, oracle_id);

CREATE TABLE IF NOT EXISTS hard_filters (
  deck_id    INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  oracle_id  TEXT NOT NULL,
  card_name  TEXT NOT NULL,
  reason     TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (deck_id, oracle_id)
);

-- The brief (spec §5): hybrid — thesis/constraints freeform, engines structured.
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

-- Engine pieces are keyed by oracle_id (settled knob) so the audit can
-- check presence deterministically.
CREATE TABLE IF NOT EXISTS engine_pieces (
  engine_id INTEGER NOT NULL REFERENCES engines(id) ON DELETE CASCADE,
  oracle_id TEXT NOT NULL REFERENCES cards(oracle_id),
  note      TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (engine_id, oracle_id)
);

-- Agent edits to the brief go through the approval gate (spec §5).
CREATE TABLE IF NOT EXISTS brief_edits (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id      INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  kind         TEXT NOT NULL CHECK (kind IN ('thesis', 'constraints', 'engine_set', 'engine_remove')),
  payload_json TEXT NOT NULL,
  rationale    TEXT NOT NULL,
  source       TEXT NOT NULL DEFAULT 'agent' CHECK (source IN ('agent', 'consolidation')),
  status       TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
  created_at   TEXT NOT NULL DEFAULT (datetime('now')),
  resolved_at  TEXT
);

-- One chat per deck, forever (spec §11). Messages stored in OpenAI
-- chat-completions shape (role + content/tool_calls) so turns replay
-- verbatim; 'system' rows are mid-transcript corrections, the assembled
-- system prompt itself is rebuilt per turn and never stored.
CREATE TABLE IF NOT EXISTS chat_messages (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id      INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  role         TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'tool', 'system')),
  content_json TEXT NOT NULL,
  -- Set when an accepted consolidation moved this message out of context
  -- (spec §11). The row is NEVER deleted — "keep the raw transcript on disk".
  compacted_at TEXT,
  created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_chat_deck ON chat_messages(deck_id, id);

-- Compaction runs (spec §11). A run is a PROPOSAL until the owner accepts:
-- it holds the replacement summary, what it says it discarded, and what it
-- rescued (the diagnostic). Accepting one supersedes the previous one, so at
-- most one summary is ever resident in context.
CREATE TABLE IF NOT EXISTS consolidations (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id            INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  status             TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
  summary            TEXT NOT NULL,
  discarded_json     TEXT NOT NULL DEFAULT '[]',
  rescued_json       TEXT NOT NULL DEFAULT '[]',
  -- Compaction zone: chat_messages with id <= this, for this deck.
  through_message_id INTEGER NOT NULL,
  message_count      INTEGER NOT NULL,
  brief_edit_ids     TEXT NOT NULL DEFAULT '[]',
  superseded_by      INTEGER REFERENCES consolidations(id),
  created_at         TEXT NOT NULL DEFAULT (datetime('now')),
  resolved_at        TEXT
);

-- Playtest notes (spec §9): freeform, stamped to the deck version that was
-- exported. cards_json pins the exact list the note is about, so the note
-- stays attached to that 100 even after the deck moves on.
CREATE TABLE IF NOT EXISTS playtest_notes (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id    INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  revision   INTEGER NOT NULL,
  note       TEXT NOT NULL,
  cards_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_playtest_deck ON playtest_notes(deck_id, id);

-- Audit runs are the audit's memory (spec §8). A run is inserted as
-- 'running' the moment it starts and finished asynchronously, so the reasoning
-- pass survives the browser closing; only the newest AUDIT_RUN_RETENTION runs
-- per deck are kept.
CREATE TABLE IF NOT EXISTS audit_runs (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id      INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  run_number   INTEGER NOT NULL,
  revision     INTEGER NOT NULL,
  instructions TEXT NOT NULL DEFAULT '',
  findings_json TEXT NOT NULL,
  reasoning_json TEXT,
  status       TEXT NOT NULL DEFAULT 'done' CHECK (status IN ('running', 'done', 'error')),
  error        TEXT,
  finished_at  TEXT,
  created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_audit_runs_deck ON audit_runs(deck_id, id);

-- Dismissed audit findings, keyed by the finding's stable key so the audit
-- doesn't report the same four things forever (spec §8.3).
CREATE TABLE IF NOT EXISTS audit_dismissals (
  deck_id     INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  finding_key TEXT NOT NULL,
  type        TEXT NOT NULL CHECK (type IN ('hard_filter', 'thesis_change', 'playtest_finding', 'soft')),
  reason      TEXT NOT NULL,
  revision    INTEGER NOT NULL,
  created_at  TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (deck_id, finding_key)
);

-- Durable playtest findings (strongest rejection type, kept forever).
CREATE TABLE IF NOT EXISTS card_notes (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id    INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  oracle_id  TEXT NOT NULL,
  card_name  TEXT NOT NULL,
  note       TEXT NOT NULL,
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
  -- The maybe list (spec §4.1): considered for the deck but not in it. A flag
  -- rather than a role or a slot, because it is orthogonal to both — a parked
  -- card keeps the slot it would fill and the tags describing it, so moving it
  -- back is a one-bit change and nothing has to be re-entered.
  maybeboard INTEGER NOT NULL DEFAULT 0,
  owned     INTEGER NOT NULL DEFAULT 0,
  quantity  INTEGER NOT NULL DEFAULT 1 CHECK (quantity >= 1),
  added_at  TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (deck_id, oracle_id)
);

CREATE TABLE IF NOT EXISTS deck_card_tags (
  deck_id   INTEGER NOT NULL,
  oracle_id TEXT NOT NULL,
  tag_id    INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (deck_id, oracle_id, tag_id),
  FOREIGN KEY (deck_id, oracle_id) REFERENCES deck_cards(deck_id, oracle_id) ON DELETE CASCADE
);
`;
