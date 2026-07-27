import type { DatabaseSync } from "node:sqlite";
import { ServiceError } from "./service.ts";

export interface EngineView {
  id: number;
  name: string;
  description: string;
  pieces: Array<{ oracle_id: string; name: string; note: string; in_deck: boolean }>;
}

export interface BriefView {
  thesis: string;
  constraints_md: string;
  engines: EngineView[];
  updated_at: string | null;
}

function requireDeck(db: DatabaseSync, deckId: number) {
  if (!db.prepare("SELECT 1 FROM decks WHERE id = ?").get(deckId))
    throw new ServiceError(`Deck ${deckId} not found`, 404);
}

export function getBrief(db: DatabaseSync, deckId: number): BriefView {
  requireDeck(db, deckId);
  const row = db
    .prepare("SELECT thesis, constraints_md, updated_at FROM briefs WHERE deck_id = ?")
    .get(deckId) as { thesis: string; constraints_md: string; updated_at: string } | undefined;

  const engines = db
    .prepare("SELECT id, name, description FROM engines WHERE deck_id = ? ORDER BY name")
    .all(deckId) as unknown as Array<{ id: number; name: string; description: string }>;

  const pieceStmt = db.prepare(
    `SELECT ep.oracle_id, c.name, ep.note,
            EXISTS(SELECT 1 FROM deck_cards dc WHERE dc.deck_id = ? AND dc.oracle_id = ep.oracle_id) AS in_deck
     FROM engine_pieces ep JOIN cards c ON c.oracle_id = ep.oracle_id
     WHERE ep.engine_id = ? ORDER BY c.name`,
  );

  return {
    thesis: row?.thesis ?? "",
    constraints_md: row?.constraints_md ?? "",
    updated_at: row?.updated_at ?? null,
    engines: engines.map((e) => ({
      ...e,
      pieces: (pieceStmt.all(deckId, e.id) as unknown as Array<{
        oracle_id: string;
        name: string;
        note: string;
        in_deck: number;
      }>).map((p) => ({ ...p, in_deck: !!p.in_deck })),
    })),
  };
}

// Direct edits by the user — no gate (spec §5: "editable by me directly").
export function updateBrief(
  db: DatabaseSync,
  deckId: number,
  patch: { thesis?: string; constraints_md?: string },
): void {
  requireDeck(db, deckId);
  const current = getBrief(db, deckId);
  db.prepare(
    `INSERT INTO briefs (deck_id, thesis, constraints_md, updated_at)
     VALUES (?, ?, ?, datetime('now'))
     ON CONFLICT(deck_id) DO UPDATE SET thesis = excluded.thesis,
       constraints_md = excluded.constraints_md, updated_at = excluded.updated_at`,
  ).run(deckId, patch.thesis ?? current.thesis, patch.constraints_md ?? current.constraints_md);
}

function validatePieces(db: DatabaseSync, oracleIds: string[]) {
  const stmt = db.prepare("SELECT name FROM cards WHERE oracle_id = ?");
  for (const oid of oracleIds) {
    if (!stmt.get(oid)) throw new ServiceError(`Unknown oracle_id ${oid} in engine pieces`, 404);
  }
}

export function setEngine(
  db: DatabaseSync,
  deckId: number,
  name: string,
  description: string,
  pieces: Array<{ oracle_id: string; note?: string }>,
): number {
  requireDeck(db, deckId);
  if (!name.trim()) throw new ServiceError("Engine name cannot be empty");
  validatePieces(db, pieces.map((p) => p.oracle_id));

  db.exec("BEGIN");
  try {
    const existing = db
      .prepare("SELECT id FROM engines WHERE deck_id = ? AND name = ? COLLATE NOCASE")
      .get(deckId, name.trim()) as { id: number } | undefined;
    let engineId: number;
    if (existing) {
      engineId = existing.id;
      db.prepare("UPDATE engines SET name = ?, description = ? WHERE id = ?").run(
        name.trim(),
        description,
        engineId,
      );
      db.prepare("DELETE FROM engine_pieces WHERE engine_id = ?").run(engineId);
    } else {
      const r = db
        .prepare("INSERT INTO engines (deck_id, name, description) VALUES (?, ?, ?)")
        .run(deckId, name.trim(), description);
      engineId = Number(r.lastInsertRowid);
    }
    const ins = db.prepare(
      "INSERT OR IGNORE INTO engine_pieces (engine_id, oracle_id, note) VALUES (?, ?, ?)",
    );
    for (const p of pieces) ins.run(engineId, p.oracle_id, p.note ?? "");
    db.exec("COMMIT");
    return engineId;
  } catch (e) {
    db.exec("ROLLBACK");
    throw e;
  }
}

export function removeEngine(db: DatabaseSync, deckId: number, engineId: number): void {
  const r = db.prepare("DELETE FROM engines WHERE id = ? AND deck_id = ?").run(engineId, deckId);
  if (!r.changes) throw new ServiceError(`Engine ${engineId} not found in deck ${deckId}`, 404);
}

// ---------- agent-proposed edits (approval gate, spec §5) ----------

export type BriefEditKind = "thesis" | "constraints" | "engine_set" | "engine_remove";

export interface BriefEditPayload {
  content?: string; // thesis / constraints
  engine_name?: string;
  description?: string;
  pieces?: Array<{ oracle_id: string; note?: string }>;
}

export function proposeBriefEdit(
  db: DatabaseSync,
  deckId: number,
  kind: BriefEditKind,
  payload: BriefEditPayload,
  rationale: string,
  source: "agent" | "consolidation" = "agent",
): number {
  requireDeck(db, deckId);
  if (!rationale?.trim()) throw new ServiceError("A brief edit needs a rationale");
  if ((kind === "thesis" || kind === "constraints") && payload.content === undefined)
    throw new ServiceError(`A ${kind} edit needs 'content'`);
  if (kind === "engine_set") {
    if (!payload.engine_name?.trim()) throw new ServiceError("engine_set needs 'engine_name'");
    validatePieces(db, (payload.pieces ?? []).map((p) => p.oracle_id));
  }
  if (kind === "engine_remove" && !payload.engine_name?.trim())
    throw new ServiceError("engine_remove needs 'engine_name'");

  const r = db
    .prepare(
      "INSERT INTO brief_edits (deck_id, kind, payload_json, rationale, source) VALUES (?, ?, ?, ?, ?)",
    )
    .run(deckId, kind, JSON.stringify(payload), rationale.trim(), source);
  return Number(r.lastInsertRowid);
}

export function listBriefEdits(db: DatabaseSync, deckId: number, status?: string) {
  return db
    .prepare(
      `SELECT id, kind, payload_json, rationale, source, status, created_at FROM brief_edits
       WHERE deck_id = ? ${status ? "AND status = ?" : ""} ORDER BY id DESC`,
    )
    .all(...(status ? [deckId, status] : [deckId])) as unknown as Array<{
    id: number;
    kind: BriefEditKind;
    payload_json: string;
    rationale: string;
    source: string;
    status: string;
    created_at: string;
  }>;
}

function getPendingEdit(db: DatabaseSync, deckId: number, editId: number) {
  const edit = db
    .prepare("SELECT * FROM brief_edits WHERE id = ? AND deck_id = ?")
    .get(editId, deckId) as
    | { id: number; kind: BriefEditKind; payload_json: string; rationale: string; status: string }
    | undefined;
  if (!edit) throw new ServiceError(`Brief edit ${editId} not found`, 404);
  if (edit.status !== "pending") throw new ServiceError(`Brief edit ${editId} is already ${edit.status}`);
  return edit;
}

function logBriefDecision(
  db: DatabaseSync,
  deckId: number,
  kind: "accept" | "reject",
  summary: string,
  rejection?: { type: string; reason: string },
) {
  const { revision } = db.prepare("SELECT revision FROM decks WHERE id = ?").get(deckId) as {
    revision: number;
  };
  db.prepare(
    `INSERT INTO decision_log (deck_id, revision, kind, rationale, rejection_type, rejection_reason, brief_flag)
     VALUES (?, ?, ?, ?, ?, ?, 0)`,
  ).run(deckId, revision, kind, summary, rejection?.type ?? null, rejection?.reason ?? null);
}

export function acceptBriefEdit(db: DatabaseSync, deckId: number, editId: number): void {
  const edit = getPendingEdit(db, deckId, editId);
  const payload = JSON.parse(edit.payload_json) as BriefEditPayload;

  db.exec("BEGIN");
  try {
    switch (edit.kind) {
      case "thesis":
        updateBrief(db, deckId, { thesis: payload.content ?? "" });
        break;
      case "constraints":
        updateBrief(db, deckId, { constraints_md: payload.content ?? "" });
        break;
      case "engine_set":
        setEngine(db, deckId, payload.engine_name!, payload.description ?? "", payload.pieces ?? []);
        break;
      case "engine_remove": {
        const engine = db
          .prepare("SELECT id FROM engines WHERE deck_id = ? AND name = ? COLLATE NOCASE")
          .get(deckId, payload.engine_name!) as { id: number } | undefined;
        if (engine) removeEngine(db, deckId, engine.id);
        break;
      }
    }
    db.prepare(
      "UPDATE brief_edits SET status = 'accepted', resolved_at = datetime('now') WHERE id = ?",
    ).run(editId);
    logBriefDecision(db, deckId, "accept", `Brief edit accepted (${edit.kind}): ${edit.rationale}`);
    db.exec("COMMIT");
  } catch (e) {
    db.exec("ROLLBACK");
    throw e;
  }
}

export function rejectBriefEdit(
  db: DatabaseSync,
  deckId: number,
  editId: number,
  type: string,
  reason: string,
): void {
  if (!reason?.trim()) throw new ServiceError("Rejecting a brief edit requires a reason");
  const edit = getPendingEdit(db, deckId, editId);
  db.exec("BEGIN");
  try {
    db.prepare(
      "UPDATE brief_edits SET status = 'rejected', resolved_at = datetime('now') WHERE id = ?",
    ).run(editId);
    logBriefDecision(
      db,
      deckId,
      "reject",
      `Brief edit rejected (${edit.kind}): ${edit.rationale}`,
      { type, reason: reason.trim() },
    );
    db.exec("COMMIT");
  } catch (e) {
    db.exec("ROLLBACK");
    throw e;
  }
}
