// Loaded only when the owner asks to edit. Public readers need only prose.
import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import type { Envelope } from "../../../api/client.ts";
import { Button, ErrorNote, Eyebrow, Text, TextArea } from "../../../ui/index.ts";
import { useAttempt } from "../dialogs/useAttempt.ts";
import styles from "./BriefField.module.css";

export function BriefEditor({ title, value, purpose, onSave, onClose }: {
  title: string; value: string; purpose: string; onSave: (value: string) => Promise<Envelope>; onClose: () => void;
}) {
  const [draft, setDraft] = useState(value);
  const { busy, error, mutate } = useAttempt();
  const describedBy = useId();
  const errorId = `${describedBy}-error`;
  const area = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    const el = area.current;
    el?.focus();
    el?.setSelectionRange(el.value.length, el.value.length);
  }, []);
  async function commit() {
    if (busy || draft === value) return;
    if (await mutate(() => onSave(draft))) onClose();
    else if (document.activeElement === document.body) area.current?.focus({ preventScroll: true });
  }
  function keyDown(event: KeyboardEvent) {
    if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      if (!busy) onClose();
    }
    else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) { event.preventDefault(); void commit(); }
  }
  return <>
    <div className={styles.head}>
      <Eyebrow as="h2">{title}</Eyebrow>
      <div className={styles.actions}>
        <Button size="sm" onClick={onClose} disabled={busy}>Cancel</Button>
        <Button size="sm" variant="primary" onClick={() => void commit()} disabled={busy || draft === value}>{busy ? "Saving…" : "Save"}</Button>
      </div>
    </div>
    <TextArea ref={area} autosize rows={3} readOnly={busy} value={draft} onChange={e => setDraft(e.target.value)} onKeyDown={keyDown} placeholder={purpose} aria-label={title} aria-describedby={error ? `${describedBy} ${errorId}` : describedBy} />
    {error && <ErrorNote id={errorId}>{error}</ErrorNote>}
    <Text id={describedBy} tone="faint" size="xs" as="p">Markdown, and [[Card Name]] for a card. Ctrl+Enter saves, Esc cancels.</Text>
  </>;
}
