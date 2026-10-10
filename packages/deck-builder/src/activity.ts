// The activity feed (spec §11): every write an agent makes, approved or not.
// It is the audit trail for writes that needed no approval — memories,
// filing, shortlists, draft edits — so "silent when written" never means
// hidden.

import type { DatabaseSync } from "node:sqlite";
import type { Actor } from "./actor.ts";

export interface ActivityEntry {
  id: number;
  ts: string;
  actor: string;
  action: string;
  deck_id: number | null;
  deck_name: string | null;
  summary: string;
}

/** Record an agent's write. The owner's own actions are not activity. */
export function recordActivity(
  db: DatabaseSync,
  actor: Actor,
  action: string,
  deckId: number | null,
  summary: string,
): void {
  if (actor.kind !== "agent") return;
  db.prepare("INSERT INTO activity (actor, action, deck_id, summary) VALUES (?, ?, ?, ?)").run(
    actor.name,
    action,
    deckId,
    summary,
  );
}

export function listActivity(db: DatabaseSync, limit = 100): ActivityEntry[] {
  return db
    .prepare(
      `SELECT a.id, a.ts, a.actor, a.action, a.deck_id, d.name AS deck_name, a.summary
       FROM activity a LEFT JOIN decks d ON d.id = a.deck_id ORDER BY a.id DESC LIMIT ?`,
    )
    .all(Math.min(Math.max(limit, 1), 500)) as unknown as ActivityEntry[];
}
