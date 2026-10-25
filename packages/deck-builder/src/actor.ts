import type { Database } from "./sqlite.ts";
import { AppError, ServiceError } from "./errors.ts";
import type { DeckRow } from "./deck/service.ts";

/** The authenticated principal, independent of the way they act (§17). */
export type Actor = {
  user_id: number;
  via: "ui" | "agent";
  label: string;
  token_id?: number;
};
export type DeckRole = "owner" | "viewer" | "suggester";

/** Retained compact author field; structured attribution is authoritative. */
export function authorOf(actor: Actor): string {
  return actor.via === "ui" ? "owner" : actor.label;
}

export class PermissionError extends AppError {
  constructor(message: string) { super(message, 403); }
}

export function requireSelf(actor: Actor, userId: number): void {
  if (actor.user_id !== userId) throw new PermissionError("You can access only your own account data");
}

export function isDeckOwner(db: Database, actor: Actor, deckId: number): boolean {
  return !!db.prepare("SELECT 1 FROM decks WHERE id = ? AND user_id = ?").get(deckId, actor.user_id);
}

export function requireDeckOwner(db: Database, actor: Actor, deckId: number, what = "change this deck"): DeckRow {
  const deck = db.prepare("SELECT * FROM decks WHERE id = ?").get(deckId) as DeckRow | undefined;
  if (!deck) throw new ServiceError(`There is no deck ${deckId}. List the decks to see which ids exist.`, 404);
  if (deck.user_id !== actor.user_id) throw new PermissionError(`Only the deck owner can ${what}.`);
  return { ...deck, role: "owner" };
}

/** Human-only actions preserve the Paper approval gate for owner-agents. */
export function requireUi(actor: Actor, what: string): void {
  if (actor.via !== "ui")
    throw new PermissionError(`Only the owner can ${what}. Ask them to do it in the app.`);
}

export function requireAdmin(db: Database, actor: Actor): void {
  const user = db.prepare("SELECT is_admin FROM users WHERE id = ? AND deleted_at IS NULL").get(actor.user_id) as { is_admin: number } | undefined;
  if (!user?.is_admin) throw new PermissionError("Only an administrator can refresh card data");
}

export interface Attribution {
  author_user_id: number;
  author_via: "ui" | "agent";
  author_label: string;
  author_display?: string;
  author_token_id?: number | null;
}
/** Human-readable UI attribution; compact MCP author strings remain stable. */
export function displayAuthor(db: Database, row: Attribution): string {
  const user = db.prepare("SELECT handle FROM users WHERE id = ?").get(row.author_user_id) as { handle: string };
  return row.author_via === "ui" ? user.handle : `${user.handle} via ${row.author_label}`;
}

/** URL labels are attribution only. Authenticated agents own drafts by their
 * credential; trusted in-process actors without credentials retain label identity. */
export function isAuthor(actor: Actor, row: Attribution): boolean {
  return row.author_user_id === actor.user_id && row.author_via === actor.via &&
    (actor.via === "ui" || (actor.token_id != null ? row.author_token_id === actor.token_id : row.author_label === actor.label));
}
