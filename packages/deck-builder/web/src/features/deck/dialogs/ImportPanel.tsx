import { useEffect, useRef, useState } from "react";
import { api, type ImportDiff } from "../../../api/client.ts";
import { Button, ErrorNote, Field, Row, Spacer, Stack, Text, TextArea, TextField, useToast } from "../../../ui/index.ts";
import { useDeck } from "../DeckStore.tsx";
import { ImportPreview } from "./ImportPreview.tsx";
import { useAttempt } from "./useAttempt.ts";
import styles from "./ImportPanel.module.css";

export interface ImportDraft {
  paste: string;
  note: string;
  diff: ImportDiff | null;
}

interface Props {
  draft: ImportDraft;
  onDraft: (patch: Partial<ImportDraft>) => void;
  onClose: () => void;
}

export function ImportPanel({ draft, onDraft, onClose }: Props) {
  const { deckId, run } = useDeck();
  const { toast } = useToast();
  const { error, busy, attempt, mutate, setError } = useAttempt();
  const { paste, note, diff } = draft;
  // Cuts take a second step: the same button, asked again.
  const [confirming, setConfirming] = useState(false);
  const [nothing, setNothing] = useState(false);
  const goBack = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (confirming) goBack.current?.focus();
  }, [confirming]);

  async function preview() {
    setConfirming(false);
    setNothing(false);
    const result = await attempt(() => api.previewImport(deckId, paste));
    if (result) onDraft({ diff: result });
  }

  async function commit() {
    if (!diff || busy) return;
    const result = await mutate(() => api.importList(deckId, paste, note.trim() || undefined));
    if (!result) return;
    const { log_id, applied } = result.import;
    if (log_id == null) {
      setConfirming(false);
      setNothing(true);
      return;
    }
    const bits = [`${applied.added} added`, `${applied.cut} cut`];
    if (applied.quantity_changed) bits.push(`${applied.quantity_changed} quantity changed`);
    onClose();
    toast(`Imported: ${bits.join(" · ")}.`, {
      duration: 12000,
      action: {
        label: "Undo",
        onAction: () =>
          void run(() => api.undo(deckId, log_id)).then((undone) => {
            if (!undone) return;
            toast(
              undone.skipped.length
                ? `Undid the import. ${undone.skipped.length} could not be undone: ${undone.skipped.join("; ")}`
                : "Undid the import.",
            );
          }),
      },
    });
  }

  const changes = diff ? diff.add_count + diff.cut_count : 0;

  function changePaste(text: string) {
    setConfirming(false);
    setNothing(false);
    setError(null);
    onDraft({ paste: text, diff: null });
  }

  return (
    <Stack gap={5} className={styles.panel}>
      <Text tone="muted" as="p">
        Paste a list from Archidekt or any one-card-per-line list. Nothing changes until you read the
        preview and apply it.
      </Text>
      <TextArea
        rows={7}
        value={paste}
        onChange={(e) => changePaste(e.target.value)}
        aria-label="List to import"
        placeholder={"Paste a list here…\n1x Card name [Category]\n1 Another card"}
        className={styles.paste}
        data-autofocus=""
      />
      <Row>
        <Button onClick={preview} disabled={busy || !paste.trim()}>
          {busy && !diff ? "Reading…" : "Preview"}
        </Button>
        {paste.trim() && !diff ? (
          <Text tone="muted" size="sm">
            Read the list against this deck before anything is applied.
          </Text>
        ) : null}
      </Row>

      {error && !diff ? <ErrorNote>{error}</ErrorNote> : null}
      {nothing ? (
        <Text tone="muted" as="p">
          No differences: the deck already matches that list.
        </Text>
      ) : null}

      {diff ? <ImportPreview diff={diff} /> : null}

      {diff ? (
        <div className={styles.actions}>
          {error ? <ErrorNote className={styles.error}>{error}</ErrorNote> : null}
          {changes === 0 ? (
            <Text tone="muted">Nothing to apply: this list would not change any card.</Text>
          ) : confirming ? (
            <Row gap={5} wrap>
              <Text tone="cut" weight="medium">
                Cut {diff.cut_count} {diff.cut_count === 1 ? "card" : "cards"} from this deck?
              </Text>
              <Spacer />
              <Button ref={goBack} variant="ghost" onClick={() => setConfirming(false)}>
                Go back
              </Button>
              <Button variant="reject" onClick={commit} disabled={busy}>
                {busy ? "Applying…" : "Apply the import"}
              </Button>
            </Row>
          ) : (
            <Stack gap={3}>
              <Row gap={5}>
                <Field label="Note for History (optional)" className={styles.note}>
                  <TextField
                    value={note}
                    onChange={(e) => onDraft({ note: e.target.value })}
                    placeholder="Imported after playtesting"
                  />
                </Field>
                <Button
                  variant={diff.cut_count > 0 ? "reject" : "primary"}
                  onClick={() => (diff.cut_count > 0 ? setConfirming(true) : void commit())}
                  disabled={busy}
                  className={styles.apply}
                >
                  {busy ? "Applying…" : "Apply the import"}
                </Button>
              </Row>
              <Text tone="muted" size="sm" as="p">
                The whole import is one entry in History. One undo reverts all of it.
              </Text>
            </Stack>
          )}
        </div>
      ) : null}
    </Stack>
  );
}
