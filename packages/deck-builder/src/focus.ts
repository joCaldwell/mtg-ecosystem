import { requireDeckOwner } from "./actor.ts";
import type { Actor } from "./actor.ts";
// Focus (spec §10.3): what the owner has open in the app. The interface is
// stateless and every call names its deck, so this is how "this deck" in
// conversation comes to mean the one on screen.

import type { Database } from "./sqlite.ts";
import { getSetting, setSetting } from "./db.ts";

export interface Focus {
  deck_id: number;
  deck_name: string;
  /** Cards selected in the deck view, by name. */
  selected: string[];
  /** Minutes since the app last reported it. Old focus may be a closed tab. */
  age_minutes: number;
}

export function setFocus(db: Database, actor: Actor, deckId: number | null, selected: string[] = []): void {
  if (deckId != null) requireDeckOwner(db, actor, deckId);
  setSetting(
    db, actor,
    "focus",
    JSON.stringify({ deck_id: deckId, selected: selected.slice(0, 50), at: Date.now() }),
  );
}

export function getFocus(db: Database, actor: Actor): Focus | null {
  const raw = getSetting(db, actor, "focus");
  if (!raw) return null;
  let parsed: { deck_id: number | null; selected: string[]; at: number };
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (parsed.deck_id == null) return null;
  const deck = db.prepare("SELECT id, name FROM decks WHERE id = ? AND user_id = ?").get(parsed.deck_id, actor.user_id) as
    | { id: number; name: string }
    | undefined;
  if (!deck) return null;
  const nameOf = db.prepare("SELECT name FROM cards WHERE oracle_id = ?");
  return {
    deck_id: deck.id,
    deck_name: deck.name,
    selected: (parsed.selected ?? [])
      .map((id) => (nameOf.get(id) as { name: string } | undefined)?.name)
      .filter((n): n is string => !!n),
    age_minutes: Math.max(0, Math.round((Date.now() - parsed.at) / 60000)),
  };
}
