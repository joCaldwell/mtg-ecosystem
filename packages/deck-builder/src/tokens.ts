// Personal credentials (§10.1, §17). Raw secrets leave this module only on create.
import type { Database } from "./sqlite.ts";
import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import { chmodSync, existsSync, readFileSync, renameSync } from "node:fs";
import { dirname, join } from "node:path";
import { DATA_DIR, withTransaction } from "./db.ts";
import { type Actor, requireSelf, requireUi } from "./actor.ts";
import { ServiceError } from "./errors.ts";

export interface AgentToken {
  id: number; user_id: number; label: string; prefix: string;
  created_at: string; last_used_at: string | null; revoked_at: string | null;
}
const hash = (secret: string) => createHash("sha256").update(secret).digest("hex");
const prefixOf = (secret: string) => secret.slice(0, 12);
const columns = "id, user_id, label, prefix, created_at, last_used_at, revoked_at";

export function createToken(db: Database, actor: Actor, label: string): { token: string; credential: AgentToken } {
  requireUi(actor, "create an agent token");
  const name = label.trim();
  if (!name || name.length > 80) throw new ServiceError("A token label must contain 1–80 characters");
  const token = `dbk_${randomBytes(32).toString("base64url")}`;
  const row = db.prepare(`INSERT INTO agent_tokens (user_id, label, prefix, token_hash) VALUES (?, ?, ?, ?)`)
    .run(actor.user_id, name, prefixOf(token), hash(token));
  const credential = db.prepare(`SELECT ${columns} FROM agent_tokens WHERE id = ?`).get(row.lastInsertRowid) as unknown as AgentToken;
  return { token, credential };
}

export function listTokens(db: Database, actor: Actor): AgentToken[] {
  requireUi(actor, "manage agent tokens");
  return db.prepare(`SELECT ${columns} FROM agent_tokens WHERE user_id = ? ORDER BY id DESC`).all(actor.user_id) as unknown as AgentToken[];
}

export function revokeToken(db: Database, actor: Actor, id: number): void {
  requireUi(actor, "revoke an agent token");
  const row = db.prepare("SELECT user_id FROM agent_tokens WHERE id = ?").get(id) as { user_id: number } | undefined;
  if (!row) throw new ServiceError("Agent token not found", 404);
  requireSelf(actor, row.user_id);
  db.prepare("UPDATE agent_tokens SET revoked_at = COALESCE(revoked_at, datetime('now')) WHERE id = ? AND user_id = ?").run(id, actor.user_id);
}

/** Phase 2's OAuth JWT verifier will sit beside this personal-token path. */
export function verifyToken(db: Database, secret: string, now = Date.now()): Actor | null {
  if (!secret || secret.length > 256) return null;
  const digest = Buffer.from(hash(secret), "hex");
  const rows = db.prepare(`SELECT t.id, t.user_id, t.label, t.token_hash FROM agent_tokens t
    JOIN users u ON u.id = t.user_id WHERE t.prefix = ? AND t.revoked_at IS NULL AND u.deleted_at IS NULL`)
    .all(prefixOf(secret)) as unknown as { id: number; user_id: number; label: string; token_hash: string }[];
  for (const row of rows) {
    const stored = Buffer.from(row.token_hash, "hex");
    if (stored.length !== digest.length || !timingSafeEqual(stored, digest)) continue;
    db.prepare(`UPDATE agent_tokens SET last_used_at = ? WHERE id = ?
      AND (last_used_at IS NULL OR unixepoch(last_used_at) <= ?)`)
      .run(new Date(now).toISOString(), row.id, Math.floor(now / 1000) - 60);
    return { user_id: row.user_id, via: "agent", label: row.label, token_id: row.id };
  }
  return null;
}

/** Local boot: commit before renaming the existing plaintext shim credential.
 * A saved stdio file also rehydrates credentials after restoring a v10 backup. */
export function migrateLegacyToken(db: Database, file = join(DATA_DIR, "agent-token")): void {
  const saved = join(dirname(file), "stdio-token");
  const source = existsSync(file) ? file : saved;
  if (!existsSync(source)) return;
  const secret = readFileSync(source, "utf8").trim();
  if (!secret) throw new Error("Legacy agent-token file is empty; it was preserved");
  withTransaction(db, () => {
    const digest = hash(secret);
    db.prepare(`INSERT INTO agent_tokens (user_id, label, prefix, token_hash)
      VALUES (1, 'Migrated local token', ?, ?) ON CONFLICT(token_hash) DO NOTHING`).run(prefixOf(secret), digest);
    const token = db.prepare("SELECT id FROM agent_tokens WHERE user_id = 1 AND token_hash = ?").get(digest) as { id: number };
    for (const table of ["proposals", "document_edits"])
      db.prepare(`UPDATE ${table} SET author_token_id = ? WHERE author_user_id = 1 AND author_via = 'agent' AND author_token_id IS NULL`).run(token.id);
  });
  chmodSync(source, 0o600);
  if (source !== saved) renameSync(source, saved);
}
