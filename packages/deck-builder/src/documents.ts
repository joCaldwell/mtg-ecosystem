// The two documents (spec §6) and the gate in front of them. The player
// profile and each deck's brief are the owner's voice: the owner edits either
// directly, and an agent proposes an edit the owner sees as a diff and rules
// on. This applies to all editable decks; Legacy decks are read-only.

import type { DatabaseSync } from "node:sqlite";
import { recordActivity } from "./activity.ts";
import { type PowerTarget, getPowerTarget, parseTarget, targetText, writePowerTarget } from "./deck/power.ts";
import { ServiceError } from "./errors.ts";
import { withTransaction } from "./db.ts";
import { type Actor, authorOf, requireOwner } from "./actor.ts";
import { lintProse, requireCardById } from "./cards.ts";
import { writeLog } from "./deck/log.ts";
import { requireDeck, requireEditableDeck } from "./deck/service.ts";
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

export function getProfile(db: DatabaseSync): ProfileView {
  const row = db.prepare("SELECT body_md, updated_at FROM profile WHERE id = 1").get() as {
    body_md: string;
    updated_at: string;
  };
  const words = row.body_md.trim() ? row.body_md.trim().split(/\s+/).length : 0;
  return { ...row, word_count: words, over_cap: words > PROFILE_WORD_CAP };
}

export function setProfile(db: DatabaseSync, actor: Actor, bodyMd: string): void {
  requireOwner(actor, "edit the profile directly");
  writeProfile(db, bodyMd);
}

function writeProfile(db: DatabaseSync, bodyMd: string) {
  db.prepare("UPDATE profile SET body_md = ?, updated_at = datetime('now') WHERE id = 1").run(
    bodyMd.trim(),
  );
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
  status: "pending" | "accepted" | "rejected";
  created_at: string;
  /** The document changed after this was proposed; accepting overwrites that. */
  stale: boolean;
  /** For a text edit: the document's text right now. */
  current: string;
  /** Engine pieces by name, for display. */
  piece_names: string[];
}

function currentText(db: DatabaseSync, kind: DocumentEditKind, deckId: number | null): string {
  if (kind === "profile") return getProfile(db).body_md;
  if (kind === "power_target") return targetText(getPowerTarget(db, deckId!));
  const brief = getBrief(db, deckId!);
  return kind === "thesis" ? brief.thesis : kind === "constraints" ? brief.constraints_md : "";
}

function validateEdit(
  db: DatabaseSync,
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
    for (const p of payload.pieces ?? []) requireCardById(db, p.oracle_id);
    lintProse(db, payload.description ?? "");
    for (const p of payload.pieces ?? []) lintProse(db, p.note ?? "");
  } else if (kind === "engine_remove") {
    if (!payload.engine_name?.trim()) throw new ServiceError("Name the engine to remove");
    const names = getBrief(db, deckId!).engines.map((e) => e.name.toLowerCase());
    if (!names.includes(payload.engine_name.trim().toLowerCase()))
      throw new ServiceError(`This deck has no engine named '${payload.engine_name}'`, 404);
  } else {
    throw new ServiceError(`Unknown document edit kind '${kind}'`);
  }
}

function applyEdit(
  db: DatabaseSync,
  actor: Actor,
  kind: DocumentEditKind,
  deckId: number | null,
  payload: DocumentEditPayload,
) {
  switch (kind) {
    case "power_target":
      return writePowerTarget(db, actor, deckId!, payload.target);
    case "profile":
      return writeProfile(db, payload.content ?? "");
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
  db: DatabaseSync,
  actor: Actor,
  deckId: number | null,
  kind: DocumentEditKind,
  payload: DocumentEditPayload,
  reason: string,
): ProposeResult {
  if ((kind === "profile") !== (deckId == null))
    throw new ServiceError("A profile edit has no deck; a brief edit needs one");
  if (deckId != null) requireEditableDeck(db, deckId);
  validateEdit(db, kind, deckId, payload);
  if (kind === "power_target") {
    if (actor.kind === "agent" && !reason?.trim()) throw new ServiceError("A power target needs a reason grounded in the owner's preferences");
    lintProse(db, reason);
  }

  if (actor.kind === "owner") {
    applyEdit(db, actor, kind, deckId, payload);
    if (kind === "power_target") recordActivity(db, actor, "propose_power_target", deckId, `Set power target: ${reason}`);
    return { applied: true, edit_id: null };
  }

  if (!reason?.trim())
    throw new ServiceError("A proposed edit needs a reason citing what prompted it");
  lintProse(db, reason);
  const stored: DocumentEditPayload = { ...payload };
  if (stored.content !== undefined) stored.base = currentText(db, kind, deckId);
  const r = db
    .prepare(
      "INSERT INTO document_edits (deck_id, kind, payload_json, reason, author) VALUES (?, ?, ?, ?, ?)",
    )
    .run(deckId, kind, JSON.stringify(stored), reason.trim(), authorOf(actor));
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
  status: "pending" | "accepted" | "rejected";
  created_at: string;
}

const EDIT_SELECT = `SELECT e.id, e.deck_id, d.name AS deck_name, e.kind, e.payload_json, e.reason,
       e.author, e.status, e.created_at
  FROM document_edits e LEFT JOIN decks d ON d.id = e.deck_id`;

function toView(db: DatabaseSync, row: EditRow): DocumentEditView {
  const { payload_json, ...rest } = row;
  const payload = JSON.parse(payload_json) as DocumentEditPayload;
  const stale =
    row.status === "pending" &&
    payload.base !== undefined &&
    payload.base !== currentText(db, row.kind, row.deck_id);
  const nameOf = db.prepare("SELECT name FROM cards WHERE oracle_id = ?");
  return {
    ...rest,
    read_only: row.deck_id != null && requireDeck(db, row.deck_id).category === "legacy",
    payload,
    stale,
    current: payload.content === undefined ? "" : currentText(db, row.kind, row.deck_id),
    piece_names: (payload.pieces ?? []).map(
      (p) => (nameOf.get(p.oracle_id) as { name: string } | undefined)?.name ?? p.oracle_id,
    ),
  };
}

/** Pending edits for one deck, for the profile ("profile"), or everywhere (omit scope). */
export function listDocumentEdits(
  db: DatabaseSync,
  opts: { deckId?: number; profileOnly?: boolean; status?: string } = {},
): DocumentEditView[] {
  const where: string[] = [];
  const params: Array<string | number> = [];
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
  ).map((r) => toView(db, r));
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

/** The owner's ruling on a proposed edit. Logged like any other ruling. */
export function ruleDocumentEdit(
  db: DatabaseSync,
  actor: Actor,
  editId: number,
  outcome: "accept" | "reject",
  reason = "",
): number {
  requireOwner(actor, "rule on a proposed edit");
  const edit = db.prepare(`${EDIT_SELECT} WHERE e.id = ?`).get(editId) as EditRow | undefined;
  if (!edit) throw new ServiceError(`Proposed edit ${editId} not found`, 404);
  if (edit.status !== "pending") throw new ServiceError(`Proposed edit ${editId} is already ${edit.status}`);
  if (edit.deck_id != null) requireEditableDeck(db, edit.deck_id);
  const payload = JSON.parse(edit.payload_json) as DocumentEditPayload;

  return withTransaction(db, () => {
    if (outcome === "accept") applyEdit(db, actor, edit.kind, edit.deck_id, payload);
    db.prepare(
      "UPDATE document_edits SET status = ?, resolved_at = datetime('now') WHERE id = ?",
    ).run(outcome === "accept" ? "accepted" : "rejected", editId);
    return writeLog(db, actor, {
      deck_id: edit.deck_id,
      kind: outcome,
      summary: `${describe(edit, payload)} ${outcome === "accept" ? "accepted" : "rejected"} — proposed because: ${edit.reason}`,
      ruling_reason: reason,
      document_edit_id: editId,
    });
  });
}
