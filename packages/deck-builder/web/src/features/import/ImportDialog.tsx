import { useId, useLayoutEffect, useRef, useState } from "react";
import { api, type DeckCategory, type Envelope, type ImportDiff } from "../../api/client.ts";
import { navigate } from "../../app/routes.ts";
import { DECK_CATEGORY_OPTIONS } from "../../domain/deckCategory.ts";
import { Button, Dialog, ErrorNote, Field, Row, Select, Text, TextArea, TextField, useToast } from "../../ui/index.ts";
import { ImportPreview } from "./ImportPreview.tsx";
import { applyImportDraft } from "./workflow.ts";
import { importName, hasImportChanges } from "../../domain/import.ts";
import styles from "./ImportDialog.module.css";

/** Both entry points share the preview and apply steps (§12). */
export function ImportDialog({ deckId = null, onClose, onApplied }: {
  deckId?: number | null; onClose: () => void; onApplied?: (result: Envelope) => void;
}) {
  const { toast } = useToast();
  const formId = useId();
  const [name, setName] = useState("");
  const [category, setCategory] = useState<DeckCategory>("theorycrafted");
  const [paste, setPaste] = useState("");
  const [note, setNote] = useState("");
  const [diff, setDiff] = useState<ImportDiff | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  // Retry after a failed apply must reuse the deck that was just created.
  const created = useRef<number | null>(null);
  const body = useRef<HTMLDivElement>(null);
  const textarea = useRef<HTMLTextAreaElement>(null);
  const back = useRef<HTMLButtonElement>(null);
  useLayoutEffect(() => {
    if (diff) body.current?.querySelector<HTMLElement>("[data-import-summary]")?.focus();
    else textarea.current?.focus();
  }, [diff]);
  useLayoutEffect(() => { if (confirming) back.current?.focus(); }, [confirming]);

  function close() {
    if (busy) return;
    onClose();
    if (created.current !== null) navigate({ page: "deck", deckId: created.current });
  }
  async function preview() {
    if (busy || !paste.trim()) return;
    setBusy(true); setError(null); setConfirming(false);
    try { setDiff(await api.previewImport(deckId ?? created.current, paste)); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  }
  async function commit() {
    if (!diff || busy) return;
    setBusy(true); setError(null);
    try {
      const result = await applyImportDraft(api, {
        deckId: deckId ?? created.current, name: importName(name, diff), category, paste, note,
      }, id => { created.current = id; setName(importName(name, diff)); });
      const id = result.state.deck.id;
      onApplied?.(result); onClose();
      if (deckId === null) navigate({ page: "deck", deckId: id });
      const { applied, log_id } = result.import;
      toast(`Imported: ${applied.added} added · ${applied.cut} cut.`, {
        duration: 12000,
        ...(log_id === null ? {} : { action: {
          label: "Undo",
          onAction: () => void api.undo(id, log_id).then(undone => {
            onApplied?.(undone);
            toast(undone.skipped.length ? `Undid the import. ${undone.skipped.length} could not be undone: ${undone.skipped.join("; ")}` : "Undid the import.");
          }).catch(e => toast(e instanceof Error ? e.message : String(e))),
        } }),
      });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setError(created.current === null ? message : `Deck created; import failed. ${message} Retry or close to open the empty deck.`);
    } finally { setBusy(false); }
  }

  return <Dialog title={deckId === null ? "Import deck" : "Import"} width={diff ? "wide" : "lg"} onClose={close} bodyClassName={styles.body}
    footer={<div className={styles.footer}>
      {diff && <Field label="Note for History (optional)" className={styles.note}><TextField value={note} disabled={busy} onChange={e => setNote(e.target.value)} /></Field>}
      <Row gap={4} wrap>
        {confirming && <Text tone="cut">Cut {diff?.cut_count} cards?</Text>}
        <Button ref={back} variant="ghost" disabled={busy} onClick={confirming ? () => setConfirming(false) : close}>{confirming ? "Go back" : "Cancel"}</Button>
        <Button variant={confirming ? "reject" : "primary"} disabled={busy || (diff ? !hasImportChanges(diff) : !paste.trim())}
          type={diff ? "button" : "submit"} form={diff ? undefined : formId}
          onClick={diff ? () => diff.cut_count > 0 && !confirming ? setConfirming(true) : void commit() : undefined}>
          {busy ? diff ? "Applying…" : "Reading…" : diff ? "Apply the import" : "Preview"}
        </Button>
      </Row>
    </div>}>
    <div ref={body} className={styles.content}>
      {error && <ErrorNote>{error}</ErrorNote>}
      {diff ? <>
        <Row justify="between" wrap>
          <Text as="h3" size="lg" weight="medium">{deckId === null ? importName(name, diff) : "Review changes"}{deckId === null && <Text tone="muted" weight="normal"> · {category === "paper" ? "Paper" : "Theorycrafted"}</Text>}</Text>
          <Button size="touch" variant="ghost" disabled={busy} onClick={() => { setDiff(null); setConfirming(false); setError(null); }}>Edit list</Button>
        </Row>
        <ImportPreview diff={diff} />
        {!hasImportChanges(diff) && <Text tone="muted">Nothing to apply: this list would not change any card.</Text>}
      </> : <form id={formId} className={styles.form} onSubmit={e => { e.preventDefault(); void preview(); }}>
        {deckId === null && <div className={styles.metadata}>
          <Field label="Deck name (optional)"><TextField value={name} placeholder="Use commander name" disabled={busy || created.current !== null} onChange={e => setName(e.target.value)} /></Field>
          <Field label="Lifecycle"><Select value={category} disabled={busy || created.current !== null} onChange={e => setCategory(e.target.value as DeckCategory)}>
            {DECK_CATEGORY_OPTIONS.filter(o => o.value !== "archived").map(o => <option key={o.value} value={o.value}>{o.label}</option>)}
          </Select></Field>
        </div>}
        <Field label="List to import" className={styles.source}><TextArea ref={textarea} data-autofocus value={paste} rows={12} disabled={busy} onChange={e => setPaste(e.target.value)} placeholder="Paste your deck list…" className={styles.paste} /></Field>
        <details className={styles.formats}><summary>Formats</summary><Text as="p" tone="muted" size="sm">Moxfield: Export → Copy Plain Text includes the commander. Archidekt text exports and quantity-and-name lists also work. Sideboard and Maybeboard go to the maybe list. Paste up to 5,000 lines.</Text></details>
      </form>}
    </div>
  </Dialog>;
}
