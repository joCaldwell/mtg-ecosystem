import type { DeckState } from "../../api/client.ts";

type Version = Pick<DeckState["deck"], "id" | "revision" | "edit_version">;

/** Membership and organization have independent producer-owned versions.
 * Equal versions can still contain new proposals, prices or memory metadata,
 * so those full server composites must remain eligible for refresh. */
export function acceptsDeckState(deckId: number, current: { deck: Version } | null, incoming: { deck: Version }): boolean {
  if (incoming.deck.id !== deckId) return false;
  if (!current) return true;
  return incoming.deck.revision >= current.deck.revision && incoming.deck.edit_version >= current.deck.edit_version;
}
