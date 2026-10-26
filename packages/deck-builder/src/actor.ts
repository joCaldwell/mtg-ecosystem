import type { Database } from "./sqlite.ts";
import { AppError, ServiceError } from "./errors.ts";
import type { DeckRow } from "./deck/service.ts";

/** The authenticated principal, independent of the way they act (§17). */
export type Actor = {
  user_id: number;
  via: "ui" | "agent";
  label: string;
  token_id?: number;
  oauth_client_id?: string;
};
export type DeckRole = "owner" | "viewer" | "suggester";

/** The label of the sample deck's synthetic writes (§11.3). It is reserved:
 * the MCP endpoint never gives it to a connected client, so it marks
 * demonstration data rather than a model that connected. */
export const SAMPLE_AGENT_LABEL = "sample-agent";

/** Only public reads accept a null viewer (§18.6); anything else refuses it
 * as an unauthenticated request rather than failing on the missing principal. */
export function requireActor(actor: Actor | null): asserts actor is Actor {
  if (!actor) throw new ServiceError("Sign in is required", 401);
}

/** Retained compact author field; structured attribution is authoritative. */
export function authorOf(actor: Actor): string {
  requireActor(actor);
  return actor.via === "ui" ? "owner" : actor.label;
}

export class PermissionError extends AppError {
  constructor(message: string) { super(message, 403); }
}

export function requireSelf(actor: Actor, userId: number): void {
  if (!actor || actor.user_id !== userId) throw new ServiceError("Account record not found", 404);
}

export function isDeckOwner(db: Database, actor: Actor, deckId: number): boolean {
  return !!actor && !!db.prepare("SELECT 1 FROM decks WHERE id = ? AND user_id = ?").get(deckId, actor.user_id);
}

export function requireDeckOwner(db: Database, actor: Actor, deckId: number, what = "change this deck"): DeckRow {
  const deck = db.prepare("SELECT * FROM decks WHERE id = ?").get(deckId) as DeckRow | undefined;
  if (!deck || !actor || deck.user_id !== actor.user_id) throw new ServiceError(`There is no deck ${deckId}. List the decks to see which ids exist.`, 404);
  return { ...deck, role: "owner" };
}

/** Human-only actions preserve the Paper approval gate for owner-agents. */
export function requireUi(actor: Actor, what: string): void {
  requireActor(actor);
  if (actor.via !== "ui")
    throw new PermissionError(`Only the owner can ${what}. Ask them to do it in the app.`);
}

export function requireAdmin(db: Database, actor: Actor, what = "refresh card data"): void {
  const user = db.prepare("SELECT is_admin FROM users WHERE id = ? AND deleted_at IS NULL").get(actor.user_id) as { is_admin: number } | undefined;
  if (!user?.is_admin) throw new PermissionError(`Only an administrator can ${what}`);
}

export interface Attribution {
  author_user_id: number;
  author_via: "ui" | "agent";
  author_label: string;
  author_display?: string;
  author_token_id?: number | null;
  author_oauth_client_id?: string | null;
}
/** Human-readable UI attribution; compact MCP author strings remain stable. */
export function displayAuthor(db: Database, row: Attribution): string {
  const user = db.prepare("SELECT handle FROM users WHERE id = ?").get(row.author_user_id) as { handle: string };
  if (row.author_via === "ui" && row.author_label === "migration") return "System migration";
  return row.author_via === "ui" ? user.handle : `${user.handle} via ${row.author_label}`;
}

/** URL labels are attribution only. Authenticated agents own drafts by their
 * credential; trusted in-process actors without credentials retain label identity. */
export function isAuthor(actor: Actor, row: Attribution): boolean {
  return row.author_user_id === actor.user_id && row.author_via === actor.via &&
    (actor.via === "ui" || (actor.token_id != null ? row.author_token_id === actor.token_id : actor.oauth_client_id != null ? row.author_oauth_client_id === actor.oauth_client_id : row.author_label === actor.label));
}
