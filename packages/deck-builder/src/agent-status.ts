// MCP is stateless: credentials establish authorization, and last use proves
// contact, not a currently open connection. Keep reads out of the write feed.
import type { Database } from "./sqlite.ts";
import { requireUi, type Actor } from "./actor.ts";

export interface AgentConnection {
  kind: "token" | "oauth";
  label: string;
  last_used_at: string | null;
}

export function agentConnections(db: Database, actor: Actor): AgentConnection[] {
  requireUi(actor, "view agent connections");
  return db.prepare(`SELECT kind, label, last_used_at FROM (
    SELECT 'token' AS kind, label, last_used_at FROM agent_tokens
      WHERE user_id = ? AND revoked_at IS NULL
    UNION ALL
    SELECT 'oauth' AS kind, COALESCE(c.name, 'OAuth client') AS label, u.last_used_at
      FROM oauthConsent g JOIN users p ON p.auth_id = g.userId
      JOIN oauthClient c ON c.clientId = g.clientId
      LEFT JOIN oauth_usage u ON u.user_id = p.id AND u.client_id = c.clientId
      WHERE p.id = ? AND (c.disabled IS NULL OR c.disabled = 0)
  ) ORDER BY julianday(last_used_at) DESC, kind, label`).all(actor.user_id, actor.user_id) as unknown as AgentConnection[];
}
