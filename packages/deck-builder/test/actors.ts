import type { Actor } from "../src/actor.ts";
export const OWNER: Actor = { user_id: 1, via: "ui", label: "ui" };
export function agent(name: string, user_id = 1): Actor {
  return { user_id, via: "agent", label: name.trim() || "agent" };
}
