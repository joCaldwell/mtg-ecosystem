// Loaded only when the owner asks to edit. Public readers need only prose.
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Button, Eyebrow, Text, TextArea } from "../../../ui/index.ts";
import styles from "./BriefField.module.css";

export function BriefEditor({ title, value, purpose, onSave, onClose }: {
  title: string; value: string; purpose: string; onSave: (value: string) => Promise<boolean>; onClose: () => void;
}) {
  const [draft, setDraft] = useState(value);
  const [busy, setBusy] = useState(false);
  const area = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    const el = area.current;
    el?.focus();
    el?.setSelectionRange(el.value.length, el.value.length);
  }, []);
  async function commit() {
    if (busy) return;
    setBusy(true);
    try { if (await onSave(draft)) onClose(); }
    finally { setBusy(false); }
  }
  function keyDown(event: KeyboardEvent) {
    if (event.key === "Escape") { event.stopPropagation(); onClose(); }
    else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) { event.preventDefault(); void commit(); }
  }
  return <>
    <div className={styles.head}>
      <Eyebrow>{title}</Eyebrow>
      <div className={styles.actions}>
        <Button size="sm" onClick={onClose} disabled={busy}>Cancel</Button>
        <Button size="sm" variant="primary" onClick={() => void commit()} disabled={busy || draft === value}>{busy ? "Saving…" : "Save"}</Button>
      </div>
    </div>
    <TextArea ref={area} autosize rows={3} value={draft} onChange={e => setDraft(e.target.value)} onKeyDown={keyDown} placeholder={purpose} aria-label={title} />
    <Text tone="faint" size="xs" as="p">Markdown, and [[Card Name]] for a card. Ctrl+Enter saves, Esc cancels.</Text>
  </>;
}
