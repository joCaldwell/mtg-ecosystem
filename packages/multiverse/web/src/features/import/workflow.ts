import type { api, DeckCategory } from "../../api/client.ts";

/** Separate calls preserve the empty deck if import fails. */
export async function applyImportDraft(client: Pick<typeof api, "createDeck" | "importList">, draft: {
  deckId: number | null; name: string; category: DeckCategory; paste: string; note: string;
}, onCreated: (id: number) => void) {
  let id = draft.deckId;
  if (id === null) {
    const result = await client.createDeck(draft.name, draft.category);
    id = result.state.deck.id; onCreated(id);
  }
  return client.importList(id, draft.paste, draft.note.trim() || undefined);
}
