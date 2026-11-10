import { AsyncLocalStorage } from "node:async_hooks";
// The activity feed (spec §11): every write an agent makes, approved or not.
// It is the audit trail for writes that needed no approval — memories,
// filing, shortlists, draft edits — so "silent when written" never means
// hidden.

import type { Database } from "./sqlite.ts";
import { displayAuthor, requireActor, requireDeckOwner, SAMPLE_AGENT_LABEL, type Actor } from "./actor.ts";
import { withTransaction } from "./db.ts";

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

type Draft = { action: string; deckId: number | null; summary: string };
const batch = new AsyncLocalStorage<{ db: Database; actor: Actor; entries: Draft[] }>();

/** One command may compose many capabilities; attribute it once. */
export function batchActivity<T, R>(db: Database, actor: Actor, action: string, work: () => T, summarize: (value: T, deckIds: number[]) => R) {
  const scope = { db, actor, entries: [] as Draft[] };
  const value = batch.run(scope, work);
  const entries = scope.entries;
  const deck_ids = [...new Set(entries.flatMap(e => e.deckId == null ? [] : [e.deckId]))];
  // Compute the result before the Activity insert invalidates request-local reads.
  const result = summarize(value, deck_ids);
  const activity_ids: number[] = [];
  if (actor.via === "agent" && entries.length) {
    const deckId = deck_ids.length === 1 && db.prepare("SELECT 1 FROM decks WHERE id=? AND user_id=?").get(deck_ids[0], actor.user_id) ? deck_ids[0] : null;
    activity_ids.push(insertActivity(db, actor, action, deckId, [...new Set(entries.map(e => e.summary))].join("; ")));
  }
  return { value: result, activity_ids };
}

/** Attribution belongs to the capability, including calls outside a transport. */
export function withActivity<T>(db: Database, actor: Actor, action: string, work: () => T, describe: (value: T) => { deckId: number | null; summary: string } | null): T {
  return withTransaction(db, () => {
    const value = work();
    const description = describe(value);
    if (description) recordActivity(db, actor, action, description.deckId, description.summary);
    return value;
  });
}

function insertActivity(db: Database, actor: Actor, action: string, deckId: number | null, summary: string): number {
  return Number(db.prepare("INSERT INTO activity (actor, action, deck_id, summary, user_id, actor_user_id, actor_via, actor_label) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
    .run(actor.label, action, deckId, summary, actor.user_id, actor.user_id, actor.via, actor.label).lastInsertRowid);
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
  const scope = batch.getStore();
  if (scope?.db === db && scope.actor === actor) {
    scope.entries.push({ action, deckId, summary });
    return;
  }
  if (actor.via === "agent") insertActivity(db, actor, action, deckId, summary);
}

export interface ActivityQuery {
  limit?: number;
  /** Keyset cursor: only entries older than this id (§11.3). */
  beforeId?: number;
  /** Only this agent label. Applied before the limit. */
  actorLabel?: string;
  /** Leave out the sample deck's synthetic writes. */
  excludeSample?: boolean;
}

/** Newest first, this principal's only. */
export function listActivity(db: Database, actor: Actor, q: ActivityQuery = {}): ActivityEntry[] {
  requireActor(actor);
  const where = ["a.user_id = ?"];
  const params: Array<string | number> = [actor.user_id];
  if (q.beforeId !== undefined) { where.push("a.id < ?"); params.push(q.beforeId); }
  if (q.actorLabel !== undefined) { where.push("a.actor_label = ?"); params.push(q.actorLabel); }
  if (q.excludeSample) { where.push("a.actor_label <> ?"); params.push(SAMPLE_AGENT_LABEL); }
  const rows = db
    .prepare(
      `SELECT a.id, a.user_id, a.actor_user_id, a.actor_via, a.actor_label, a.ts, a.actor, a.action, a.deck_id, d.name AS deck_name, a.summary
       FROM activity a LEFT JOIN decks d ON d.id = a.deck_id AND d.user_id = a.user_id WHERE ${where.join(" AND ")} ORDER BY a.id DESC LIMIT ?`,
    )
    .all(...params, Math.min(Math.max(q.limit ?? 100, 1), 500)) as unknown as ActivityEntry[];
  return rows.map(row => ({ ...row, actor_display: displayAuthor(db, { author_user_id: row.actor_user_id, author_via: row.actor_via, author_label: row.actor_label }) }));
}

/** The sidebar presence (§11.3): the latest write of a real agent, or null. */
export function latestActivity(db: Database, actor: Actor): ActivityEntry | null {
  return listActivity(db, actor, { limit: 1, excludeSample: true })[0] ?? null;
}

/** One page of the feed, its continuation and every agent label it can be filtered by. */
export function activityPage(db: Database, actor: Actor, q: Omit<ActivityQuery, "limit" | "excludeSample"> & { limit?: number } = {}) {
  const limit = Math.min(Math.max(q.limit ?? 100, 1), 100);
  const rows = listActivity(db, actor, { ...q, limit: limit + 1 });
  const actors = (db.prepare("SELECT DISTINCT actor_label FROM activity WHERE user_id = ? ORDER BY actor_label").all(actor.user_id) as { actor_label: string }[])
    .map(r => r.actor_label);
  return { entries: rows.slice(0, limit), has_more: rows.length > limit, actors };
}
