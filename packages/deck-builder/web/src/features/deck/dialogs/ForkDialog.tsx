import { useId, useState, type FormEvent } from "react";
import { api } from "../../../api/client.ts";
import { navigate } from "../../../app/routes.ts";
import { Button, Dialog, ErrorNote, Field, Stack, Text, TextField } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { useAttempt } from "./useAttempt.ts";

// Spec §5.3. The fork is a new deck, so its state is not applied to this
// deck's store: the route changes and the fork's own page loads it.
export function ForkDialog({ onClose }: { onClose: () => void }) {
  const { deckId } = useDeck();
  const { deck } = useDeckState();
  const [name, setName] = useState(`${deck.name} (fork)`);
  const { error, busy, attempt } = useAttempt();
  const formId = useId();

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!name.trim() || busy) return;
    const result = await attempt(() => api.forkDeck(deckId, name.trim()));
    if (!result) return;
    onClose();
    navigate({ page: "deck", deckId: result.state.deck.id });
  }

  return (
    <Dialog
      title="Fork this deck"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" type="submit" form={formId} disabled={!name.trim() || busy}>
            {busy ? "Forking…" : "Fork"}
          </Button>
        </>
      }
    >
      <form id={formId} onSubmit={submit}>
        <Stack gap={5}>
          <Text tone="muted" as="p">
            Copies the list, slots, tags, maybe list, brief and this deck's memories into a new draft, and
            keeps this deck as it is. The two then change independently.
          </Text>
          <Field label="Name for the fork">
            <TextField
              value={name}
              onChange={(e) => setName(e.target.value)}
              onFocus={(e) => e.currentTarget.select()}
              data-autofocus=""
            />
          </Field>
          {error ? <ErrorNote>{error}</ErrorNote> : null}
        </Stack>
      </form>
    </Dialog>
  );
}
