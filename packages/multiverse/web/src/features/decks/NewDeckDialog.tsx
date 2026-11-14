import { useVoice } from "../../app/Preferences.tsx";
import { useId, useState, type FormEvent } from "react";
import { api, type DeckCategory } from "../../api/client.ts";
import { navigate } from "../../app/routes.ts";
import { DECK_CATEGORY_OPTIONS } from "../../domain/deckCategory.ts";
import { Button, Dialog, ErrorNote, Field, Select, TextField } from "../../ui/index.ts";
import styles from "./NewDeckDialog.module.css";

/** Creates a deck and opens it. The lifecycle defaults to Theorycrafted, with no slots. */
export function NewDeckDialog({ onClose }: { onClose: () => void }) {
  const voice = useVoice();
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
        <Field label="Deck name">
          <TextField data-autofocus value={name} onChange={(e) => setName(e.target.value)} autoComplete="off" />
        </Field>
        <Field label="Lifecycle" hint={voice("Theorycrafted is a work in progress. Paper is a deck you play. You can archive either later.", "Paper requires agent card proposals; Theorycrafted allows direct agent card edits. Brief changes always need approval. You can archive either later.")}>
          <Select options={DECK_CATEGORY_OPTIONS.filter(o => o.value !== "archived").map(({ value, label }) => ({ value, label }))} value={category} onChange={(value) => setCategory(value as DeckCategory)} disabled={busy} />
        </Field>
        {error && <ErrorNote>{error}</ErrorNote>}
      </form>
    </Dialog>
  );
}
