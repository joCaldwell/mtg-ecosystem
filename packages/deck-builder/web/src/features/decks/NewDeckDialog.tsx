import { useId, useState, type FormEvent } from "react";
import { api, type DeckCategory } from "../../api/client.ts";
import { navigate } from "../../app/routes.ts";
import { DECK_CATEGORY_OPTIONS } from "../../domain/deckCategory.ts";
import { Button, Dialog, ErrorNote, Field, Select, TextField } from "../../ui/index.ts";
import styles from "./NewDeckDialog.module.css";

/** Creates a deck and opens it. The server makes it a draft with no slots (createDeck defaults status to draft). */
export function NewDeckDialog({ onClose }: { onClose: () => void }) {
  const formId = useId();
  const [name, setName] = useState("");
  const [category, setCategory] = useState<DeckCategory>("theorycrafted");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function create(e: FormEvent) {
    e.preventDefault();
    if (!name.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      const { state } = await api.createDeck(name.trim(), category);
      onClose();
      navigate({ page: "deck", deckId: state.deck.id });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setBusy(false);
    }
  }

  return (
    <Dialog
      title="New deck"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" form={formId} variant="primary" disabled={!name.trim() || busy}>
            {busy ? "Creating…" : "Create deck"}
          </Button>
        </>
      }
    >
      <form id={formId} onSubmit={create} className={styles.form}>
        <Field label="Deck name" hint="New decks start as drafts with no slots.">
          <TextField data-autofocus value={name} onChange={(e) => setName(e.target.value)} autoComplete="off" />
        </Field>
        <Field label="Category" hint="Paper for physical decks, Theorycrafted for ideas, Legacy for past decks.">
          <Select value={category} onChange={(e) => setCategory(e.target.value as DeckCategory)} disabled={busy}>
            {DECK_CATEGORY_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>{option.label}</option>
            ))}
          </Select>
        </Field>
        {error && <ErrorNote>{error}</ErrorNote>}
      </form>
    </Dialog>
  );
}
