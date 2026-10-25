import { displayAuthor } from "./actor.ts";
// The two documents (spec §6) and the gate in front of them. The player
// profile and each deck's brief are the owner's voice: the owner edits either
// directly, and an agent proposes an edit the owner sees as a diff and rules
// on. This applies to all editable decks; Legacy decks are read-only.

import type { Database } from "./sqlite.ts";
import { recordActivity } from "./activity.ts";
import { type PowerTarget, getPowerTarget, parseTarget, targetText, writePowerTarget } from "./deck/power.ts";
import { ServiceError } from "./errors.ts";
import { withTransaction } from "./db.ts";
import { type Actor, authorOf, isAuthor, requireUi, requireSelf, PermissionError } from "./actor.ts";
import { lintProse, requireCardById } from "./cards.ts";
import { writeLog } from "./deck/log.ts";
import { requireDeck, requireEditableDeck, requireRevision } from "./deck/service.ts";
import {
  type EnginePieceInput,
  getBrief,
  removeEngineByName,
  setEngine,
  updateBrief,
} from "./deck/brief.ts";

// ---------- the profile ----------

/** Spec §6: a soft cap. Over it the app flags the profile; it never truncates. */
export const PROFILE_WORD_CAP = 600;

export interface ProfileView {
  body_md: string;
  updated_at: string;
  word_count: number;
  over_cap: boolean;
}

export function getProfile(db: Database, actor: Actor): ProfileView {
  const row = db.prepare("SELECT body_md, updated_at FROM profile WHERE user_id = ?").get(actor.user_id) as {
    body_md: string;
    updated_at: string;
  };
  const value = row ?? { body_md: "", updated_at: "" };
  const words = value.body_md.trim() ? value.body_md.trim().split(/\s+/).length : 0;
  return { ...value, word_count: words, over_cap: words > PROFILE_WORD_CAP };
}

export function setProfile(db: Database, actor: Actor, bodyMd: string): void {
  requireUi(actor, "edit the profile directly");
  writeProfile(db, actor, bodyMd);
}

function writeProfile(db: Database, actor: Actor, bodyMd: string) {
  db.prepare("INSERT INTO profile (user_id, body_md) VALUES (?, ?) ON CONFLICT(user_id) DO UPDATE SET body_md = excluded.body_md, updated_at = datetime('now')").run(actor.user_id, bodyMd.trim());
}

// ---------- proposed edits ----------

export type DocumentEditKind = "profile" | "thesis" | "constraints" | "engine_set" | "engine_remove" | "power_target";

export interface DocumentEditPayload {
  /** Replacement text for profile / thesis / constraints. */
  content?: string;
  /** Structured power target, null to clear it. Text diff is derived by the app. */
  target?: PowerTarget | null;
  /** What the text was when the edit was proposed, so the diff stays honest. */
  base?: string;
  engine_name?: string;
  description?: string;
  pieces?: EnginePieceInput[];
}

export interface DocumentEditView {
  id: number;
  deck_id: number | null;
  deck_name: string | null;
  read_only: boolean;
  kind: DocumentEditKind;
  payload: DocumentEditPayload;
  reason: string;
  author: string;
  author_user_id: number;
  author_via: "ui" | "agent";
  author_label: string;
  author_display?: string;
  author_token_id: number | null;
  status: "pending" | "accepted" | "rejected";
  created_at: string;
  /** The document changed after this was proposed; accepting overwrites that. */
  stale: boolean;
  /** For a text edit: the document's text right now. */
  current: string;
  /** Engine pieces by name, for display. */
  piece_names: string[];
  base_revision: number | null;
  current_revision: number | null;
  disposition: "current" | "withdrawn" | "superseded";
  superseded_by: number | null;
  supersedes: number[];
  group_id: number | null;
  group_edit_ids: number[];
  is_current: boolean;
  group_edits: DocumentEditView[];
  current_engine: ReturnType<typeof getBrief>["engines"][number] | null;
}

function currentText(db: Database, actor: Actor, kind: DocumentEditKind, deckId: number | null, engineName?: string): string {
  if (kind === "profile") return getProfile(db, actor).body_md;
  if (kind === "power_target") return targetText(getPowerTarget(db, actor, deckId!));
  const brief = getBrief(db, actor, deckId!);
  if (kind === "engine_set" || kind === "engine_remove") {
    const engine = brief.engines.find(e => e.name.toLowerCase() === engineName?.trim().toLowerCase());
    return JSON.stringify(engine ? { name: engine.name, description: engine.description,
      pieces: engine.pieces.map(p => ({ oracle_id: p.oracle_id, note: p.note })) } : null);
  }
  return kind === "thesis" ? brief.thesis : kind === "constraints" ? brief.constraints_md : "";
}

function validateEdit(
  db: Database, actor: Actor,
  kind: DocumentEditKind,
  deckId: number | null,
  payload: DocumentEditPayload,
) {
  if (kind === "profile" || kind === "thesis" || kind === "constraints") {
    if (typeof payload.content !== "string")
      throw new ServiceError(`A ${kind} edit needs the full replacement text`);
    lintProse(db, payload.content);
  } else if (kind === "power_target") {
    if (payload.target === undefined) throw new ServiceError("A power target edit needs a target, or null to clear it");
    payload.target = payload.target == null ? null : parseTarget(payload.target);
    if (payload.target) lintProse(db, payload.target.notes);
    payload.content = targetText(payload.target);
  } else if (kind === "engine_set") {
    if (!payload.engine_name?.trim()) throw new ServiceError("An engine needs a name");
    lintProse(db, payload.engine_name);
    for (const p of payload.pieces ?? []) requireCardById(db, p.oracle_id);
    lintProse(db, payload.description ?? "");
    for (const p of payload.pieces ?? []) lintProse(db, p.note ?? "");
  } else if (kind === "engine_remove") {
    if (!payload.engine_name?.trim()) throw new ServiceError("Name the engine to remove");
    const names = getBrief(db, actor, deckId!).engines.map((e) => e.name.toLowerCase());
    if (!names.includes(payload.engine_name.trim().toLowerCase()))
      throw new ServiceError(`This deck has no engine named '${payload.engine_name}'`, 404);
  } else {
    throw new ServiceError(`Unknown document edit kind '${kind}'`);
  }
}

function applyEdit(
  db: Database,
  actor: Actor,
  kind: DocumentEditKind,
  deckId: number | null,
  payload: DocumentEditPayload,
) {
  switch (kind) {
    case "power_target":
      return writePowerTarget(db, actor, deckId!, payload.target);
    case "profile":
      return writeProfile(db, actor, payload.content ?? "");
    case "thesis":
      return updateBrief(db, actor, deckId!, { thesis: payload.content ?? "" });
    case "constraints":
      return updateBrief(db, actor, deckId!, { constraints_md: payload.content ?? "" });
    case "engine_set":
      setEngine(db, actor, deckId!, payload.engine_name!, payload.description ?? "", payload.pieces ?? []);
      return;
    case "engine_remove":
      return removeEngineByName(db, actor, deckId!, payload.engine_name!);
  }
}

export interface ProposeResult {
  /** True when the owner applied the edit directly. */
  applied: boolean;
  edit_id: number | null;
}

/**
 * Propose an edit to the profile (deckId null) or to a deck's brief. On a
 * Paper or Theorycrafted deck an agent edit waits for the owner. The reason
 * is required: it is what the owner rules on.
 */
export function proposeDocumentEdit(
  db: Database,
  actor: Actor,
  deckId: number | null,
  kind: DocumentEditKind,
  payload: DocumentEditPayload,
  reason: string,
  options: { expected_revision?: number; proposal_only?: boolean } = {},
): ProposeResult {
  if ((kind === "profile") !== (deckId == null))
    throw new ServiceError("A profile edit has no deck; a brief edit needs one");
  if (deckId != null) requireEditableDeck(db, actor, deckId);
  if (deckId != null) requireRevision(db, actor, deckId, options.expected_revision);
  validateEdit(db, actor, kind, deckId, payload);
  if (kind === "power_target") {
    if (actor.via === "agent" && !reason?.trim()) throw new ServiceError("A power target needs a reason grounded in the owner's preferences");
    lintProse(db, reason);
  }

  if (actor.via === "ui" && !options.proposal_only) {
    applyEdit(db, actor, kind, deckId, payload);
    if (kind === "power_target") recordActivity(db, actor, "propose_power_target", deckId, `Set power target: ${reason}`);
    return { applied: true, edit_id: null };
  }

  if (!reason?.trim())
    throw new ServiceError("A proposed edit needs a reason citing what prompted it");
  lintProse(db, reason);
  const stored: DocumentEditPayload = { ...payload };
  stored.base = currentText(db, actor, kind, deckId, payload.engine_name);
  const conflict = listDocumentEdits(db, actor, { status: "pending", ...(deckId == null ? { profileOnly: true } : { deckId }) })
    .find(e => isAuthor(actor, e) && editKey(e.kind, e.payload) === editKey(kind, payload));
  if (conflict) throw new ServiceError(`Edit #${conflict.id} is already current for this document. Amend or explicitly supersede it.`);
  const r = db
    .prepare(
      "INSERT INTO document_edits (deck_id, kind, payload_json, reason, author, base_revision, user_id, author_user_id, author_via, author_label, author_token_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .run(deckId, kind, JSON.stringify(stored), reason.trim(), authorOf(actor), deckId == null ? null : requireDeck(db, actor, deckId).revision, actor.user_id, actor.user_id, actor.via, actor.label, actor.token_id ?? null);
  if (kind === "power_target") recordActivity(db, actor, "propose_power_target", deckId, `Proposed power target edit #${r.lastInsertRowid}: ${reason}`);
  return { applied: false, edit_id: Number(r.lastInsertRowid) };
}

interface EditRow {
  id: number;
  deck_id: number | null;
  deck_name: string | null;
  kind: DocumentEditKind;
  payload_json: string;
  reason: string;
  author: string;
  author_user_id: number;
  author_via: "ui" | "agent";
  author_label: string;
  author_display?: string;
  author_token_id: number | null;
  status: "pending" | "accepted" | "rejected";
  created_at: string;
  base_revision: number | null;
  disposition: DocumentEditView["disposition"];
  superseded_by: number | null;
  group_id: number | null;
}

const EDIT_SELECT = `SELECT e.id, e.deck_id, d.name AS deck_name, e.kind, e.payload_json, e.reason,
       e.author, e.author_user_id, e.author_via, e.author_label, e.author_token_id, e.status, e.created_at, e.base_revision, e.disposition, e.superseded_by, e.group_id
  FROM document_edits e LEFT JOIN decks d ON d.id = e.deck_id`;

function toView(db: Database, actor: Actor, row: EditRow, includeGroup = true): DocumentEditView {
  const { payload_json, ...rest } = row;
  const payload = JSON.parse(payload_json) as DocumentEditPayload;
  const currentRevision = row.deck_id == null ? null : requireDeck(db, actor, row.deck_id).revision;
  const stale =
    row.status === "pending" &&
    (payload.base === undefined || payload.base !== currentText(db, actor, row.kind, row.deck_id, payload.engine_name) ||
      (row.deck_id != null && row.base_revision !== currentRevision));
  const nameOf = db.prepare("SELECT name FROM cards WHERE oracle_id = ?");
  return {
    ...rest,
    author_display: displayAuthor(db, row),
    current_revision: currentRevision,
    current_engine: row.kind.startsWith("engine_") && row.deck_id != null
      ? getBrief(db, actor, row.deck_id).engines.find(e => e.name.toLowerCase() === payload.engine_name?.trim().toLowerCase()) ?? null : null,
    is_current: row.status === "pending" && row.disposition === "current",
    group_edits: includeGroup && row.group_id != null
      ? (db.prepare(`${EDIT_SELECT} WHERE e.group_id = ? AND e.user_id = ? ORDER BY e.id`).all(row.group_id, actor.user_id) as unknown as EditRow[]).map(e => toView(db, actor, e, false)) : [],
    supersedes: (db.prepare("SELECT id FROM document_edits WHERE superseded_by = ? AND user_id = ? ORDER BY id").all(row.id, actor.user_id) as unknown as { id: number }[]).map(e => e.id),
    group_edit_ids: row.group_id == null ? [row.id] : (db.prepare("SELECT id FROM document_edits WHERE group_id = ? AND user_id = ? ORDER BY id").all(row.group_id, actor.user_id) as unknown as { id: number }[]).map(e => e.id),
    read_only: row.deck_id != null && requireDeck(db, actor, row.deck_id).category === "legacy",
    payload,
    stale,
    current: currentText(db, actor, row.kind, row.deck_id, payload.engine_name),
    piece_names: (payload.pieces ?? []).map(
      (p) => (nameOf.get(p.oracle_id) as { name: string } | undefined)?.name ?? p.oracle_id,
    ),
  };
}

export function getDocumentEdit(db: Database, actor: Actor, editId: number, forWrite = false): DocumentEditView | null {
  if (forWrite) {
    const scope = db.prepare("SELECT user_id FROM document_edits WHERE id = ?").get(editId) as { user_id: number } | undefined;
    if (scope) requireSelf(actor, scope.user_id);
  }
  const row = db.prepare(`${EDIT_SELECT} WHERE e.id = ? AND e.user_id = ?`).get(editId, actor.user_id) as EditRow | undefined;
  if (!row) throw new ServiceError(`Proposed edit ${editId} not found`, 404);
  return toView(db, actor, row);
}

/** Pending edits for one deck, for the profile ("profile"), or everywhere (omit scope). */
export function listDocumentEdits(
  db: Database, actor: Actor,
  opts: { deckId?: number; profileOnly?: boolean; status?: string } = {},
): DocumentEditView[] {
  if (opts.deckId !== undefined) requireDeck(db, actor, opts.deckId);
  const where: string[] = ["e.user_id = ?"];
  const params: Array<string | number> = [actor.user_id];
  if (opts.deckId !== undefined) {
    where.push("e.deck_id = ?");
    params.push(opts.deckId);
  }
  if (opts.profileOnly) where.push("e.deck_id IS NULL");
  if (opts.status) {
    where.push("e.status = ?");
    params.push(opts.status);
  }
  return (
    db
      .prepare(
        `${EDIT_SELECT} ${where.length ? `WHERE ${where.join(" AND ")}` : ""} ORDER BY e.id DESC LIMIT 200`,
      )
      .all(...params) as unknown as EditRow[]
  ).map((r) => toView(db, actor, r));
}

function describe(edit: EditRow, payload: DocumentEditPayload): string {
  switch (edit.kind) {
    case "power_target":
      return "Power target edit";
    case "profile":
      return "Profile edit";
    case "thesis":
      return "Brief edit: thesis";
    case "constraints":
      return "Brief edit: constraints";
    case "engine_set":
      return `Brief edit: engine "${payload.engine_name}"`;
    case "engine_remove":
      return `Brief edit: remove engine "${payload.engine_name}"`;
  }
}

function editKey(kind: DocumentEditKind, payload: DocumentEditPayload): string {
  return kind.startsWith("engine_") ? `engine:${payload.engine_name?.trim().toLowerCase()}` : kind;
}

export interface DocumentChangeInput { kind: DocumentEditKind; payload: DocumentEditPayload; reason: string }

function editablePendingGroup(db: Database, actor: Actor, editId: number): EditRow[] {
  const row = db.prepare(`${EDIT_SELECT} WHERE e.id = ?`).get(editId) as EditRow | undefined;
  if (!row) throw new ServiceError(`Proposed edit ${editId} not found`, 404);
  requireSelf(actor, row.author_user_id);
  const rows = row.group_id == null ? [row] : db.prepare(`${EDIT_SELECT} WHERE e.group_id = ? AND e.user_id = ? ORDER BY e.id`).all(row.group_id, actor.user_id) as unknown as EditRow[];
  for (const edit of rows) {
    if (edit.deck_id != null) requireEditableDeck(db, actor, edit.deck_id);
    if (actor.via !== "ui" && !isAuthor(actor, edit))
      throw new PermissionError("Agents can amend or withdraw only their own pending proposals");
    if (edit.status !== "pending" || edit.disposition !== "current")
      throw new ServiceError(`Proposed edit ${edit.id} is already ${edit.disposition === "current" ? edit.status : edit.disposition}`);
  }
  return rows;
}

/** Replacements are new immutable proposals, linked in both directions on read.
 * Naming any grouped edit replaces/withdraws the entire group, never half a plan. */
export function proposeDocumentGroup(db: Database, actor: Actor, deckId: number | null,
  changes: DocumentChangeInput[], options: { supersedes?: number[]; expected_revision?: number } = {}) {
  if (deckId != null) {
    requireEditableDeck(db, actor, deckId);
    requireRevision(db, actor, deckId, options.expected_revision);
  }
  if (!changes.length) throw new ServiceError("A document proposal needs at least one change");
  if (new Set(changes.map(c => editKey(c.kind, c.payload))).size !== changes.length)
    throw new ServiceError("A group may change each document field or named engine only once");
  return withTransaction(db, () => {
    const previous = new Map<number, EditRow>();
    for (const id of options.supersedes ?? []) for (const row of editablePendingGroup(db, actor, id)) {
      if (row.deck_id !== deckId) throw new ServiceError("Replacement proposals must have the same deck/profile scope");
      previous.set(row.id, row);
    }
    // Retire first inside the transaction, so duplicate detection permits the replacement.
    for (const row of previous.values()) db.prepare("UPDATE document_edits SET status = 'rejected', disposition = 'superseded', resolved_at = datetime('now') WHERE id = ?").run(row.id);
    const editIds = changes.map(c => proposeDocumentEdit(db, actor, deckId, c.kind, { ...c.payload }, c.reason, { ...options, proposal_only: true }).edit_id);
    const groupId = editIds[0]!;
    for (const id of editIds) db.prepare("UPDATE document_edits SET group_id = ? WHERE id = ?").run(groupId, id);
    for (const row of previous.values()) db.prepare("UPDATE document_edits SET superseded_by = ? WHERE id = ?").run(groupId, row.id);
    recordActivity(db, actor, "propose_document_group", deckId, `Current document group #${groupId}; supersedes edits ${[...previous.keys()].join(", ") || "none"}`);
    return { applied: false, group_id: groupId, edit_ids: editIds as number[] };
  });
}

export function withdrawDocumentEdit(db: Database, actor: Actor, editId: number, reason: string, expectedRevision?: number): void {
  if (!reason?.trim()) throw new ServiceError("Withdrawing a proposal needs a reason");
  lintProse(db, reason);
  withTransaction(db, () => {
    const rows = editablePendingGroup(db, actor, editId);
    if (rows[0].deck_id != null) requireRevision(db, actor, rows[0].deck_id, expectedRevision);
    for (const row of rows) db.prepare("UPDATE document_edits SET status = 'rejected', disposition = 'withdrawn', resolved_at = datetime('now') WHERE id = ?").run(row.id);
    recordActivity(db, actor, "withdraw_document_edit", rows[0].deck_id, `Withdrew edits ${rows.map(r => r.id).join(", ")}: ${reason}`);
  });
}

/** The owner's ruling expands to the entire group and validates every base
 * before applying anything. Stale acceptance requires deliberate acknowledgment
 * against the current revision; it can never silently overwrite newer work. */
export function ruleDocumentEdit(
  db: Database, actor: Actor, editId: number, outcome: "accept" | "reject", reason = "",
  options: { expected_revision?: number; acknowledge_stale?: boolean; expected_documents?: Record<string, string> } = {},
): number {
  requireUi(actor, "rule on a proposed edit");
  if (outcome !== "accept" && outcome !== "reject") throw new ServiceError("A ruling's outcome is 'accept' or 'reject'");
  return withTransaction(db, () => {
    const rows = editablePendingGroup(db, actor, editId);
    const deckId = rows[0].deck_id;
    if (deckId != null) requireRevision(db, actor, deckId, options.expected_revision);
    if (outcome === "accept" && options.expected_documents && rows.some(row => options.expected_documents![row.id] !== toView(db, actor, row).current))
      throw new ServiceError("The document changed since it was reviewed. Reload and review the current group.", 409);
    if (outcome === "accept" && rows.some(row => toView(db, actor, row).stale) &&
        (options.acknowledge_stale !== true || (deckId != null && options.expected_revision === undefined)))
      throw new ServiceError("Proposal is stale: the deck or document changed. Review current state, then explicitly acknowledge_stale with expected_revision, or request an amended proposal.", 409);
    let logId = 0;
    for (const edit of rows) {
      const payload = JSON.parse(edit.payload_json) as DocumentEditPayload;
      if (outcome === "accept") applyEdit(db, actor, edit.kind, edit.deck_id, payload);
      db.prepare("UPDATE document_edits SET status = ?, resolved_at = datetime('now') WHERE id = ?")
        .run(outcome === "accept" ? "accepted" : "rejected", edit.id);
      logId = writeLog(db, actor, {
        deck_id: edit.deck_id, kind: outcome,
        summary: `${describe(edit, payload)} ${outcome === "accept" ? "accepted" : "rejected"} — proposed because: ${edit.reason}`,
        ruling_reason: reason, document_edit_id: edit.id,
      });
    }
    return logId;
  });
}
