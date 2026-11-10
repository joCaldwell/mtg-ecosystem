import { requireDeckOwner } from "../actor.ts";
// The deck brief (spec §6): thesis and constraints as freeform markdown,
// engines as structured records whose pieces are card references — so "is
// every piece of this engine in the deck" is a check, not a judgement.
//
// Direct brief writes are owner-only; agents always propose through documents.ts.

import type { Database } from "../sqlite.ts";
import { ServiceError } from "../errors.ts";
import { type Actor, requireUi } from "../actor.ts";
import { withTransaction } from "../db.ts";
import { requireCardById } from "../cards.ts";
import { requireDeck, requireEditableDeck } from "./service.ts";

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

export function getBrief(db: Database, actor: Actor, deckId: number): BriefView {
  requireDeck(db, actor, deckId);
  const row = db
    .prepare("SELECT thesis, constraints_md, updated_at FROM briefs WHERE deck_id = ?")
    .get(deckId) as { thesis: string; constraints_md: string; updated_at: string } | undefined;

  const engines = db
    .prepare("SELECT id, name, description FROM engines WHERE deck_id = ? ORDER BY name")
    .all(deckId) as unknown as Array<{ id: number; name: string; description: string }>;

  // A piece parked on the maybe list is not in the deck.
  const pieceStmt = db.prepare(
    `SELECT ep.oracle_id, c.name, ep.note,
            EXISTS(SELECT 1 FROM deck_cards dc
                   WHERE dc.deck_id = ? AND dc.oracle_id = ep.oracle_id AND dc.maybe = 0) AS in_deck
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

export function updateBrief(
  db: Database,
  actor: Actor,
  deckId: number,
  patch: { thesis?: string; constraints_md?: string },
): void {
  requireEditableDeck(db, actor, deckId);
  requireUi(actor, "edit a deck brief directly (use propose_brief_edit)");
  const current = getBrief(db, actor, deckId);
  db.prepare(
    `INSERT INTO briefs (deck_id, thesis, constraints_md, updated_at)
     VALUES (?, ?, ?, datetime('now'))
     ON CONFLICT(deck_id) DO UPDATE SET thesis = excluded.thesis,
       constraints_md = excluded.constraints_md, updated_at = excluded.updated_at`,
  ).run(deckId, patch.thesis ?? current.thesis, patch.constraints_md ?? current.constraints_md);
}

export interface EnginePieceInput {
  oracle_id: string;
  note?: string;
}

/** Create the engine, or replace the one of the same name. */
export function setEngine(
  db: Database,
  actor: Actor,
  deckId: number,
  name: string,
  description: string,
  pieces: EnginePieceInput[],
): number {
  requireEditableDeck(db, actor, deckId);
  requireUi(actor, "edit a deck brief directly (use propose_brief_edit)");
  const trimmed = name.trim();
  if (!trimmed) throw new ServiceError("Engine name cannot be empty");
  for (const p of pieces) requireCardById(db, p.oracle_id);

  return withTransaction(db, () => {
    const existing = db
      .prepare("SELECT id FROM engines WHERE deck_id = ? AND name = ? COLLATE NOCASE")
      .get(deckId, trimmed) as { id: number } | undefined;
    let engineId: number;
    if (existing) {
      engineId = existing.id;
      db.prepare("UPDATE engines SET name = ?, description = ? WHERE id = ? AND deck_id = ?").run(
        trimmed,
        description,
        engineId, deckId,
      );
      db.prepare("DELETE FROM engine_pieces WHERE engine_id = ?").run(engineId);
    } else {
      const r = db
        .prepare("INSERT INTO engines (deck_id, name, description) VALUES (?, ?, ?)")
        .run(deckId, trimmed, description);
      engineId = Number(r.lastInsertRowid);
    }
    const ins = db.prepare(
      "INSERT OR IGNORE INTO engine_pieces (engine_id, oracle_id, note) VALUES (?, ?, ?)",
    );
    for (const p of pieces) ins.run(engineId, p.oracle_id, p.note ?? "");
    db.prepare(
      `INSERT INTO briefs (deck_id) VALUES (?)
       ON CONFLICT(deck_id) DO UPDATE SET updated_at = datetime('now')`,
    ).run(deckId);
    return engineId;
  });
}

export function removeEngine(db: Database, actor: Actor, deckId: number, engineId: number): void {
  requireEditableDeck(db, actor, deckId);
  requireUi(actor, "edit a deck brief directly (use propose_brief_edit)");
  const r = db.prepare("DELETE FROM engines WHERE id = ? AND deck_id = ?").run(engineId, deckId);
  if (!r.changes) throw new ServiceError(`Engine ${engineId} not found in deck ${deckId}`, 404);
}

export function removeEngineByName(db: Database, actor: Actor, deckId: number, name: string): void {
  requireEditableDeck(db, actor, deckId);
  requireUi(actor, "edit a deck brief directly (use propose_brief_edit)");
  const engine = db
    .prepare("SELECT id FROM engines WHERE deck_id = ? AND name = ? COLLATE NOCASE")
    .get(deckId, name.trim()) as { id: number } | undefined;
  if (!engine) {
    const names = getBrief(db, actor, deckId).engines.map((e) => e.name);
    throw new ServiceError(
      `This deck has no engine named '${name}'.` +
        (names.length ? ` Its engines are: ${names.join(", ")}.` : " It defines none."),
      404,
    );
  }
  removeEngine(db, actor, deckId, engine.id);
}
