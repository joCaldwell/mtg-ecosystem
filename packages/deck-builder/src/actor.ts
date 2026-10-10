import { AppError } from "./errors.ts";

// Who is acting. Every capability takes one, and permissions are enforced
// below both front doors (spec §1, rule 3): the UI passes OWNER, the agent
// interface passes the calling agent. Nothing above this layer decides what
// an agent may do.
export type Actor = { kind: "owner" } | { kind: "agent"; name: string };

export const OWNER: Actor = { kind: "owner" };

export function agent(name: string): Actor {
  return { kind: "agent", name: name.trim() || "agent" };
}

/** The name stored as `author` on anything this actor writes. */
export function authorOf(actor: Actor): string {
  return actor.kind === "owner" ? "owner" : actor.name;
}

export class PermissionError extends AppError {
  constructor(message: string) {
    super(message, 403);
  }
}

/** Spec §10.3: some actions belong to the owner and have no agent path. */
export function requireOwner(actor: Actor, what: string): void {
  if (actor.kind !== "owner")
    throw new PermissionError(`Only the owner can ${what}. Ask them to do it in the app.`);
}
