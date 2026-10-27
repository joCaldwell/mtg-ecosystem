import { useVoice } from "../../app/Preferences.tsx";
import { useState } from "react";
import { api } from "../../api/client.ts";
import { Button, ErrorNote, TextArea } from "../../ui/index.ts";
import styles from "./AddMemory.module.css";

/** Write a memory yourself: it is yours from the start, and stated. */
export function AddMemory({
  scope,
  deckId,
  onAdded,
  scopeLabel,
}: {
  scope: "global" | "deck";
  deckId?: number;
  onAdded: () => void;
  scopeLabel?: string;
}) {
  const voice = useVoice();
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit() {
    setError(null);
    setBusy(true);
    try {
      await api.addMemory({ text, scope, deck_id: deckId ?? null });
      setText("");
      onAdded();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form
      className={styles.form}
      onSubmit={(e) => {
        e.preventDefault();
        if (text.trim() && !busy) void submit();
      }}
    >
      <TextArea
        autosize
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="Write a memory. Name cards as [[Card Name]]."
        aria-label="New memory"
      />
      {error && <ErrorNote>{error}</ErrorNote>}
      <div className={styles.row}>
        <span className={styles.hint}>{scopeLabel ?? voice("Keep one preference or lesson and its reason. You can change it later.", "A memory you write is yours. Agents can read it and never change it.")}</span>
        <Button type="submit" size="sm" variant="primary" disabled={busy || !text.trim()}>
          Remember
        </Button>
      </div>
    </form>
  );
}
