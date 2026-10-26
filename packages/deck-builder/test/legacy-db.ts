import type { Database } from "../src/sqlite.ts";

/** Tests seed through current services, then reconstruct a real pre-identity
 * file before applying their historical schema edits. Never just relabel v11. */
export function removeIdentity(db: Database): void {
  db.exec("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = ON");
  removePriceSources(db);
  // Strip phase-2 additions too: historical fixtures must be real old shapes.
  for (const name of ["user","session","account","verification","jwks","oauthClient","oauthResource","oauthClientResource","oauthRefreshToken","oauthAccessToken","oauthConsent","oauthClientAssertion","invites","signup_intents","oauth_usage"]) db.exec(`DROP TABLE IF EXISTS "${name}"`);
  const decks = (db.prepare("SELECT sql FROM sqlite_master WHERE name = 'decks'").get() as { sql: string }).sql;
  const columns = (db.prepare("PRAGMA table_info(decks)").all() as { name: string }[]).map(r => r.name).filter(n => n !== "user_id");
  const legacy = decks.replace(/CREATE TABLE (?:"decks"|decks)/, "CREATE TABLE decks_old")
    .replace(/\s*user_id INTEGER NOT NULL REFERENCES users\(id\),/, "")
    .replace(/UNIQUE\(user_id, name\)/, "UNIQUE(name)");
  db.exec(`${legacy}; INSERT INTO decks_old (${columns.join(',')}) SELECT ${columns.join(',')} FROM decks;
    DROP TABLE decks; ALTER TABLE decks_old RENAME TO decks;
    CREATE TABLE profile_old (id INTEGER PRIMARY KEY CHECK(id = 1), body_md TEXT NOT NULL DEFAULT '', updated_at TEXT NOT NULL DEFAULT (datetime('now')));
    INSERT INTO profile_old SELECT 1, body_md, updated_at FROM profile WHERE user_id = 1;
    DROP TABLE profile; ALTER TABLE profile_old RENAME TO profile;
    CREATE TABLE playtest_sessions_old (deck_id INTEGER PRIMARY KEY REFERENCES decks(id) ON DELETE CASCADE, payload_json TEXT NOT NULL);
    INSERT INTO playtest_sessions_old SELECT deck_id, payload_json FROM playtest_sessions;
    DROP TABLE playtest_sessions; ALTER TABLE playtest_sessions_old RENAME TO playtest_sessions;
    INSERT INTO meta SELECT 'setting:' || key, value FROM user_settings;
    DROP TABLE user_settings; DROP TABLE agent_tokens;`);
  for (const table of ["memories", "document_edits", "log", "activity", "ownership_evidence", "power_targets", "power_assessments", "proposals"]) {
    const cols = db.prepare(`PRAGMA table_info(${table})`).all() as { name: string }[];
    for (const name of ["user_id", "author_user_id", "author_via", "author_label", "actor_user_id", "actor_via", "actor_label", "author_token_id", "author_oauth_client_id"]) {
      if (!cols.some(c => c.name === name)) continue;
      db.exec(`DROP INDEX IF EXISTS idx_${table}_user; ALTER TABLE ${table} DROP COLUMN ${name}`);
    }
  }
  db.exec("DROP TABLE users; PRAGMA legacy_alter_table = OFF; PRAGMA foreign_keys = ON");
}

/** Reconstruct the USD-only price table when testing historical migrations. */
export function removePriceSources(db: Database): void {
  db.exec(`CREATE TABLE card_prices_legacy (
    oracle_id TEXT PRIMARY KEY, usd_cents INTEGER NOT NULL, set_code TEXT NOT NULL,
    collector_number TEXT NOT NULL, finish TEXT NOT NULL CHECK(finish IN ('nonfoil','foil','etched'))
  );
  INSERT INTO card_prices_legacy SELECT oracle_id,cents,set_code,collector_number,finish FROM card_prices WHERE store='tcgplayer';
  DROP TABLE card_prices; ALTER TABLE card_prices_legacy RENAME TO card_prices;
  DELETE FROM meta WHERE key='price_stores_version';`);
}
