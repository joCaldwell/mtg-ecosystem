import { useVoice } from "../../app/Preferences.tsx";
import { useId, useState, type FormEvent } from "react";
import { api, type DeckCategory, type DeckSummary } from "../../api/client.ts";
import { DECK_CATEGORY_LABELS, DECK_CATEGORY_OPTIONS } from "../../domain/deckCategory.ts";
import { Button, Dialog, ErrorNote, Field, Select, Stack, Text, TextField, useToast } from "../../ui/index.ts";
import type { DeckAction } from "./DeckActions.tsx";

export function DeckActionDialog({ deck, action, onClose, onSaved }: {
  deck: DeckSummary;
  action: DeckAction;
  onClose: () => void;
  onSaved: () => void;
}) {
  const voice = useVoice();
  const formId = useId();
  const { toast } = useToast();
  const [name, setName] = useState(deck.name);
  const [category, setCategory] = useState<DeckCategory>(deck.category);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const title = action === "rename" ? "Rename deck" : action === "category" ? "Change lifecycle" : "Delete deck";
  const unchanged = action === "rename" ? name.trim() === deck.name : action === "category" ? category === deck.category : false;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || unchanged || (action === "rename" && !name.trim())) return;
    setBusy(true);
    setError(null);
    try {
      if (action === "rename") {
        await api.renameDeck(deck.id, name.trim());
        toast(`Renamed "${deck.name}" to "${name.trim()}".`);
      } else if (action === "category") {
        await api.setDeckCategory(deck.id, category);
        toast(`"${deck.name}" is now ${DECK_CATEGORY_LABELS[category]}.`);
      } else {
        await api.deleteDeck(deck.id);
        toast(`Deleted "${deck.name}".`);
      }
      onSaved();
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setBusy(false);
    }
  }

  return (
    <Dialog
      title={title}
      onClose={() => { if (!busy) onClose(); }}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={busy} data-autofocus={action === "delete" ? "" : undefined}>Cancel</Button>
          <Button
            type="submit"
            form={formId}
            variant={action === "delete" ? "reject" : "primary"}
            disabled={busy || unchanged || (action === "rename" && !name.trim())}
          >
            {busy ? "Saving…" : action === "delete" ? "Delete deck" : "Save changes"}
          </Button>
        </>
      }
    >
      <form id={formId} onSubmit={submit}>
        <Stack gap={4}>
          {action === "rename" ? (
            <Field label="Deck name">
              <TextField data-autofocus value={name} disabled={busy} onFocus={(e) => e.currentTarget.select()} onChange={(e) => setName(e.target.value)} autoComplete="off" />
            </Field>
          ) : action === "category" ? (
            <Field label={`Lifecycle for ${deck.name}`} hint={voice("Paper is a deck you play. Theorycrafted is a work in progress. Archived is read-only. Only you can change this.", "Paper requires agent card proposals; Theorycrafted allows direct agent card edits; Archived is read-only. Only you can change lifecycle.")}>
              <Select data-autofocus options={DECK_CATEGORY_OPTIONS.map(({ value, label }) => ({ value, label }))} value={category} disabled={busy} onChange={(value) => setCategory(value as DeckCategory)} />
            </Field>
          ) : (
            <>
              <Text as="p">Delete <strong>{deck.name}</strong>? This permanently removes its list, slots, tags, brief, proposals, decisions and deck-scoped memories. This cannot be undone.</Text>
              <Text as="p" tone="muted">Global memories, fork decks and activity entries remain. Forks lose their link to this deck.</Text>
            </>
          )}
          {error && <ErrorNote>{error}</ErrorNote>}
        </Stack>
      </form>
    </Dialog>
  );
}
