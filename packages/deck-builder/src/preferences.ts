// First-run voice and progressive disclosure (§11.3). Preferences always
// belong to the authenticated principal; agents cannot change the UI.
import type { Database } from "./sqlite.ts";
import { requireActor, requireUi, type Actor } from "./actor.ts";
import { getSetting, setSetting } from "./db.ts";
import { ServiceError } from "./errors.ts";

export function uiPreferences(db: Database, actor: Actor) {
  requireActor(actor);
  const written = db.prepare(`SELECT 1 FROM activity WHERE user_id=? LIMIT 1`).get(actor.user_id)
    ?? db.prepare(`SELECT 1 FROM memories WHERE user_id=? AND author_via='agent' LIMIT 1`).get(actor.user_id)
    ?? db.prepare(`SELECT 1 FROM proposals WHERE author_user_id=? AND author_via='agent' LIMIT 1`).get(actor.user_id)
    ?? db.prepare(`SELECT 1 FROM document_edits WHERE user_id=? AND author_via='agent' LIMIT 1`).get(actor.user_id)
    ?? db.prepare(`SELECT 1 FROM log WHERE user_id=? AND author_via='agent' LIMIT 1`).get(actor.user_id);
  return { has_agent_activity: !!written, advanced_open: getSetting(db, actor, "advanced_open") === "true" };
}

export function setAdvancedOpen(db: Database, actor: Actor, value: unknown): void {
  requireUi(actor, "change Advanced controls");
  if (typeof value !== "boolean") throw new ServiceError("Advanced must be open or closed");
  setSetting(db, actor, "advanced_open", String(value));
}
