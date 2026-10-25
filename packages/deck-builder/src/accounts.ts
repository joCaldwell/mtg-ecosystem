import { randomBytes, createHash } from "node:crypto";
import type { Database } from "./sqlite.ts";
import { type Actor, requireUi, requireAdmin, PermissionError } from "./actor.ts";
import { withTransaction } from "./db.ts";
import { ServiceError, isUniqueViolation } from "./errors.ts";
import { exportDeck } from "./deck/interop.ts";

export function accountInfo(db: Database, actor: Actor) {
  requireUi(actor, "manage your account");
  const row = db.prepare("SELECT id,handle,display_name,is_admin,needs_handle,created_at FROM users WHERE id=? AND deleted_at IS NULL").get(actor.user_id) as { id: number; handle: string; display_name: string; is_admin: number; needs_handle: number; created_at: string } | undefined;
  if (!row) throw new ServiceError("Account not found", 404);
  const admins = (db.prepare("SELECT count(*) n FROM users WHERE is_admin=1 AND deleted_at IS NULL").get() as { n: number }).n;
  return { ...row, can_delete: !row.is_admin || admins > 1, deletion_reason: row.is_admin && admins === 1 ? "The last administrator cannot delete their account: someone must remain to manage invites and jobs." : null };
}
export function chooseHandle(db: Database, actor: Actor, handle: string) {
  requireUi(actor, "choose your handle");
  if (!/^[a-z0-9_-]{3,24}$/.test(handle)) throw new ServiceError("Use 3–24 lowercase letters, digits, underscores or hyphens");
  try { db.prepare("UPDATE users SET handle=?,needs_handle=0 WHERE id=?").run(handle, actor.user_id); }
  catch (e) { if (isUniqueViolation(e)) throw new ServiceError("That handle is already taken", 409); throw e; }
  return accountInfo(db, actor);
}
export function createInvite(db: Database, actor: Actor) {
  requireUi(actor, "create invites"); requireAdmin(db, actor, "create invites");
  const code = randomBytes(24).toString("base64url");
  db.prepare("INSERT INTO invites(code,created_by) VALUES(?,?)").run(code, actor.user_id);
  return { code };
}
export function adminInfo(db: Database, actor: Actor) {
  requireUi(actor, "manage this service"); requireAdmin(db, actor, "manage this service");
  return {
    invites: db.prepare("SELECT code,created_at,used_at FROM invites WHERE created_by=? ORDER BY created_at DESC").all(actor.user_id) as { code: string; created_at: string; used_at: string | null }[],
    users: (db.prepare("SELECT count(*) n FROM users WHERE deleted_at IS NULL").get() as { n: number }).n,
    database_bytes: (db.prepare("PRAGMA page_count").get() as { page_count: number }).page_count * (db.prepare("PRAGMA page_size").get() as { page_size: number }).page_size,
  };
}

const quote = (s: string) => `"${s.replaceAll('"', '""')}"`;
// All global tables are explicit. Unknown tables must lead to a tenant root
// through a real FK or known principal column; otherwise lifecycle fails closed.
const globals = new Set(["cards", "card_names", "card_prices", "card_prices_staging", "meta", "jwks", "oauthResource", "oauthClientAssertion", "signup_intents", "sqlite_sequence"]);
const isIndex = (name: string) => /^(cards_fts|memories_fts)(_|$)/.test(name);
const secretColumns = new Set(["token", "token_hash", "clientSecret", "password", "accessToken", "refreshToken", "idToken", "privateKey", "rotationReplayResponse"]);

type Scope = { table: string; where: string };
/** Inventory derived from sqlite_master/FKs, also used by lifecycle tests.
 * Binds :uid and :aid, never interpolates a principal into SQL. */
export function accountScopes(db: Database): Scope[] {
  const names = (db.prepare("SELECT name FROM sqlite_master WHERE type='table'").all() as { name: string }[]).map(x => x.name).filter(x => !globals.has(x) && !isIndex(x));
  const scopes = new Map<string, string>([["users", "id=:uid"], ["user", "id=:aid"], ["verification", "0"]]);
  const visit = (name: string, visiting = new Set<string>()): string | undefined => {
    if (scopes.has(name)) return scopes.get(name);
    if (visiting.has(name) || globals.has(name) || isIndex(name)) return undefined;
    const next = new Set(visiting).add(name);
    const columns = db.prepare(`PRAGMA table_info(${quote(name)})`).all() as { name: string }[];
    const parts = columns.filter(c => ["user_id", "author_user_id", "actor_user_id", "created_by", "used_by"].includes(c.name)).map(c => `${quote(c.name)}=:uid`);
    if (columns.some(c => c.name === "userId")) parts.push('"userId"=:aid');
    // A direct tenant key is authoritative. Tables without one inherit through
    // their FK (deck children, OAuth client-resource links, memory card links).
    if (!parts.length) {
      const groups = new Map<number, { id:number; seq:number; table:string; from:string; to:string }[]>();
      for (const fk of db.prepare(`PRAGMA foreign_key_list(${quote(name)})`).all() as { id:number; seq:number; table:string; from:string; to:string }[]) {
        const group=groups.get(fk.id) ?? []; group.push(fk); groups.set(fk.id,group);
      }
      for (const group of groups.values()) {
        group.sort((a,b)=>a.seq-b.seq);
        const parent=visit(group[0].table,next);
        // Composite FKs are one relation. Matching their columns separately
        // would select other tenants who happen to use the same oracle_id.
        if(parent) parts.push(`(${group.map(f=>quote(f.from)).join(",")}) IN (SELECT ${group.map(f=>quote(f.to)).join(",")} FROM ${quote(group[0].table)} WHERE ${parent})`);
      }
    }
    if (!parts.length) return undefined;
    const where = parts.map(p => `(${p})`).join(" OR "); scopes.set(name, where); return where;
  };
  return names.map(table => {
    const where = visit(table);
    if (!where) throw new Error(`Account lifecycle has no classification for table ${table}`);
    return { table, where };
  });
}
function relatedVerification(db: Database, authId: string) {
  const contains = (v: unknown): boolean => v === authId || (v !== null && typeof v === "object" && Object.values(v).some(contains));
  return (db.prepare("SELECT id,identifier,value FROM verification").all() as { id: string; identifier: string; value: string }[]).filter(r => {
    if (r.identifier === authId) return true;
    try { return contains(JSON.parse(r.value)); } catch { return false; }
  });
}
function ownedRows(db: Database, actor: Actor) {
  const user = db.prepare("SELECT auth_id FROM users WHERE id=?").get(actor.user_id) as { auth_id: string | null };
  const params = { uid: actor.user_id, aid: user.auth_id ?? "" };
  // SQLite permits extra named params only when explicitly configured.
  const rows = Object.fromEntries(accountScopes(db).map(({ table, where }) => {
    const q = db.prepare(`SELECT * FROM ${quote(table)} WHERE ${where}`); q.setAllowUnknownNamedParameters(true);
    return [table, q.all(params) as Record<string, any>[]];
  }));
  rows.verification = user.auth_id ? relatedVerification(db, user.auth_id) : [];
  rows.memories_fts = db.prepare("SELECT rowid,* FROM memories_fts WHERE memory_id IN (SELECT id FROM memories WHERE user_id=?)").all(actor.user_id) as Record<string, any>[];
  return rows;
}
export function exportAccount(db: Database, actor: Actor) {
  accountInfo(db, actor);
  const rows = ownedRows(db, actor);
  const tables = Object.fromEntries(Object.entries(rows).map(([table, data]) => [table, data.map(row => Object.fromEntries(Object.entries(row).filter(([key]) => !secretColumns.has(key))))]));
  // Verification payloads are opaque OAuth codes/state and may contain tokens.
  tables.verification = rows.verification.map(({ id, identifier }) => ({ id, identifier }));
  return { exported_at: new Date().toISOString(), tables, decks: (db.prepare("SELECT id,name FROM decks WHERE user_id=?").all(actor.user_id) as { id: number; name: string }[]).map(d => ({ ...d, archidekt: exportDeck(db, actor, d.id, { includeMaybe: true }).text })) };
}
export function deleteAccount(db: Database, actor: Actor, handle: string): void {
  const info = accountInfo(db, actor);
  if (handle !== info.handle) throw new ServiceError("Type your handle to confirm deletion");
  if (!info.can_delete) throw new PermissionError(info.deletion_reason!);
  withTransaction(db, () => {
    const rows = ownedRows(db, actor);
    // Remove referencing rows before parents, handling self references through
    // deferred FK checking inside this transaction. Never disable FK checks.
    db.exec("PRAGMA defer_foreign_keys=ON");
    for (const row of rows.memories_fts) db.prepare("DELETE FROM memories_fts WHERE rowid=?").run(row.rowid);
    const tables = Object.keys(rows).filter(t => t !== "memories_fts");
    // Snapshot PKs first: parent cascades must not change later predicates.
    for (const table of tables) {
      const keys = (db.prepare(`PRAGMA table_info(${quote(table)})`).all() as { name: string; pk: number }[]).filter(x => x.pk).sort((a,b) => a.pk - b.pk);
      for (const row of rows[table]) db.prepare(`DELETE FROM ${quote(table)} WHERE ${keys.map(k => `${quote(k.name)}=?`).join(" AND ")}`).run(...keys.map(k => row[k.name]));
    }
    // Clear intents for an invite removed by this deletion, without storing
    // the plaintext code in the ticket table.
    for (const invite of rows.invites) db.prepare("DELETE FROM signup_intents WHERE code_hash=?").run(createHash("sha256").update(invite.code).digest("hex"));
    if (db.prepare("PRAGMA foreign_key_check").get()) throw new Error("Account deletion left a foreign key reference");
  });
}
