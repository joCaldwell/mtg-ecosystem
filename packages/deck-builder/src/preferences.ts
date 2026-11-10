// First-run voice (§11.3), scoped to the authenticated principal.
import type { Database } from "./sqlite.ts";
import { requireActor, requireUi, SAMPLE_AGENT_LABEL, type Actor } from "./actor.ts";

import { getSetting, setSetting } from "./db.ts";
import { ServiceError } from "./errors.ts";

// One indexed probe per table (v15's partial agent-author indexes, and
// activity by user and label), each stopping at its first row. The sample
// deck's synthetic writes carry the reserved label and are demonstration
// data, not a connected agent (§11.3), including after the deck is deleted.
const PROBES = [
  "SELECT 1 FROM activity WHERE user_id=? AND actor_via='agent' AND actor_label<>? LIMIT 1",
  "SELECT 1 FROM memories WHERE user_id=? AND author_via='agent' AND author_label<>? LIMIT 1",
  "SELECT 1 FROM proposals WHERE author_user_id=? AND author_via='agent' AND author_label<>? LIMIT 1",
  "SELECT 1 FROM document_edits WHERE user_id=? AND author_via='agent' AND author_label<>? LIMIT 1",
  "SELECT 1 FROM log WHERE user_id=? AND author_via='agent' AND author_label<>? LIMIT 1",
];

export function uiPreferences(db: Database, actor: Actor) {
  requireActor(actor);
  const written = PROBES.some(sql => db.prepare(sql).get(actor.user_id, SAMPLE_AGENT_LABEL));
  return { has_agent_activity: written, sidebar_collapsed: getSetting(db, actor, "sidebar_collapsed") === "true" };
}

/** Desktop navigation only (§11.3); absent keys default to expanded. */
export function setSidebarCollapsed(db: Database, actor: Actor, value: unknown): void {
  requireUi(actor, "collapse the sidebar");
  if (typeof value !== "boolean") throw new ServiceError("Sidebar collapsed must be a boolean");
  setSetting(db, actor, "sidebar_collapsed", String(value));
}
