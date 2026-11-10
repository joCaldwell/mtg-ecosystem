// Additive invalidation vocabulary shared by both front doors and the UI.
export const RESOURCES = ["decks", "deck", "inbox", "memories", "profile", "activity", "preferences", "settings", "agents", "focus", "stats", "cards", "playtest", "jobs"] as const;
export type Resource = typeof RESOURCES[number];
export interface Change { kind: string; deck_id?: number; deck_ids?: number[]; resources?: Resource[]; global_resources?: Resource[] }

export function commandResources(operation: string): Resource[] {
  if (/focus/.test(operation)) return ["focus"];
  if (/tokens|oauth/.test(operation)) return ["agents", "settings"];
  if (/settings|preferences/.test(operation)) return ["settings", "preferences", "deck", "decks", "stats", "memories", "cards"];
  if (/playtest/.test(operation) && !/observations/.test(operation)) return ["playtest"];
  if (/memory|memories|remember|observations/.test(operation)) return ["memories", "activity", "preferences", "settings"];
  if (/profile/.test(operation)) return ["profile", "inbox", "activity", "preferences"];
  if (/proposal|propose|document|brief|power_target|decisions_reviewed|items\/rule|edits/.test(operation)) return ["deck", "decks", "inbox", "activity", "preferences", "profile", "stats"];
  if (/assess|power/.test(operation)) return ["deck", "activity", "preferences"];
  if (/record_ownership/.test(operation)) return ["deck", "activity", "preferences"];
  if (/jobs|refresh/.test(operation)) return ["jobs", "settings", "cards", "deck", "decks", "stats"];
  if (/^(?:create_deck|fork_deck|delete_deck|rename_deck)$/.test(operation) || /^\w+ \/api\/(?:decks(?:\/\d+)?$|demo$|.*\/fork$)/.test(operation))
    return ["deck", "decks", "inbox", "memories", "settings", "activity", "preferences", "stats", "focus"];
  return ["deck", "decks", "inbox", "activity", "preferences", "stats"];
}
