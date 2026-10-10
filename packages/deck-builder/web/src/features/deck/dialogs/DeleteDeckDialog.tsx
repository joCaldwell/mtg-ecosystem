import { api } from "../../../api/client.ts";
import { navigate } from "../../../app/routes.ts";
import { ConfirmDialog, ErrorNote, Stack, Text } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { useAttempt } from "./useAttempt.ts";

// What is listed here is what deleteDeck removes: every table that refers to
// the deck cascades (schema in src/db.ts). Global memories do not refer to
// a deck and stay; a fork only loses its link to this deck (parent_id is set
// to null), so it is never deleted along with it.
export function DeleteDeckDialog({ onClose }: { onClose: () => void }) {
  const { deckId } = useDeck();
  const { deck } = useDeckState();
  const { error, busy, attempt } = useAttempt();

  async function confirm() {
    if (busy) return;
    const result = await attempt(() => api.deleteDeck(deckId));
    if (!result) return;
    onClose();
    navigate({ page: "decks" });
  }

  return (
    <ConfirmDialog
      title="Delete this deck"
      confirmLabel={busy ? "Deleting…" : "Delete deck"}
      tone="danger"
      onConfirm={confirm}
      onClose={onClose}
    >
      <Stack gap={5}>
        <Text as="p">
          Delete <strong>{deck.name}</strong>? This removes its list and maybe list, its slots and tags,
          its brief, its proposals, and its rulings log with every import and undo recorded in it. Memories
          written for this deck only are deleted too.
        </Text>
        <Text as="p" tone="muted">
          Global memories stay. Forks of this deck stay as decks of their own and lose the link to it. Entries
          about it stay in the activity feed. This cannot be undone.
        </Text>
        {error ? <ErrorNote>{error}</ErrorNote> : null}
      </Stack>
    </ConfirmDialog>
  );
}
