// The activity feed (spec §11): every write an agent makes, approved or not.
// It is the audit trail for writes that needed no approval — memories,
// filing, shortlists, draft edits — so "silent when written" never means
// hidden.

import type { Database } from "./sqlite.ts";
import { displayAuthor, requireDeckOwner, type Actor } from "./actor.ts";

export interface ActivityEntry {
  id: number;
  ts: string;
  actor: string;
  user_id: number;
  actor_user_id: number;
  actor_via: "ui" | "agent";
  actor_label: string;
  actor_display?: string;
  action: string;
  deck_id: number | null;
  deck_name: string | null;
  summary: string;
}

/** Record an agent's write. The owner's own actions are not activity. */
export function recordActivity(
  db: Database,
  actor: Actor,
  action: string,
  deckId: number | null,
  summary: string,
): void {
  if (deckId != null) requireDeckOwner(db, actor, deckId);
  if (actor.via !== "agent") return;
  db.prepare("INSERT INTO activity (actor, action, deck_id, summary, user_id, actor_user_id, actor_via, actor_label) VALUES (?, ?, ?, ?, ?, ?, ?, ?)").run(
    actor.label,
    action,
    deckId,
    summary, actor.user_id, actor.user_id, actor.via, actor.label,
  );
}

export function listActivity(db: Database, actor: Actor, limit = 100): ActivityEntry[] {
  const rows = db
    .prepare(
      `SELECT a.id, a.user_id, a.actor_user_id, a.actor_via, a.actor_label, a.ts, a.actor, a.action, a.deck_id, d.name AS deck_name, a.summary
       FROM activity a LEFT JOIN decks d ON d.id = a.deck_id AND d.user_id = a.user_id WHERE a.user_id = ? ORDER BY a.id DESC LIMIT ?`,
    )
    .all(actor.user_id, Math.min(Math.max(limit, 1), 500)) as unknown as ActivityEntry[];
  return rows.map(row => ({ ...row, actor_display: displayAuthor(db, { author_user_id: row.actor_user_id, author_via: row.actor_via, author_label: row.actor_label }) }));
}
