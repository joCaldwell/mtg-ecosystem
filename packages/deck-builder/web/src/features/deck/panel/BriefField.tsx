// One text field of the brief (thesis or constraints). At rest it is prose,
// rendered like every other agent output, so [[Card Name]] is a hover card and
// markdown reads as markdown; it becomes an editor only when asked. The draft
// doubles as the mode: null is reading, a string is editing, so a stale draft
// cannot outlive the editor it belongs to.

import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { RichText } from "../../../mtg/index.ts";
import { Button, Eyebrow, Text, TextArea } from "../../../ui/index.ts";
import styles from "./BriefField.module.css";

interface BriefFieldProps {
  title: string;
  value: string;
  /** What the field is for; shown when it is empty and in the editor. */
  purpose: string;
  /** Resolves false on failure, in which case the draft stays open and unlost. */
  onSave: (next: string) => Promise<boolean>;
  /** Pending proposed edits, shown in place under the field. */
  children?: ReactNode;
}

export function BriefField({ title, value, purpose, onSave, children }: BriefFieldProps) {
  const [draft, setDraft] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const area = useRef<HTMLTextAreaElement>(null);
  const editing = draft !== null;

  useEffect(() => {
    const el = area.current;
    if (!editing || !el) return;
    el.focus();
    // Caret at the end: the usual edit appends a clause, and a select-all
    // would be one keystroke from losing the text.
    el.setSelectionRange(el.value.length, el.value.length);
  }, [editing]);

  async function commit() {
    if (draft === null || busy) return;
    setBusy(true);
    const ok = await onSave(draft);
    setBusy(false);
    if (ok) setDraft(null);
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      // Stopped here so the screen's Escape does not also back out of the panel.
      e.stopPropagation();
      setDraft(null);
    } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      void commit();
    }
  }

  return (
    <section className={styles.field}>
      <div className={styles.head}>
        <Eyebrow>{title}</Eyebrow>
        <div className={styles.actions}>
          {editing ? (
            <>
              <Button size="sm" variant="ghost" onClick={() => setDraft(null)} disabled={busy}>
                Cancel
              </Button>
              <Button size="sm" variant="primary" onClick={() => void commit()} disabled={busy || draft === value}>
                {busy ? "Saving…" : "Save"}
              </Button>
            </>
          ) : (
            <Button size="sm" variant="ghost" onClick={() => setDraft(value)} aria-label={`Edit the ${title.toLowerCase()}`}>
              Edit
            </Button>
          )}
        </div>
      </div>
      {editing ? (
        <>
          <TextArea
            ref={area}
            autosize
            rows={3}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder={purpose}
            aria-label={title}
          />
          <Text tone="faint" size="xs" as="p">
            Markdown, and [[Card Name]] for a card. Ctrl+Enter saves, Esc cancels.
          </Text>
        </>
      ) : value.trim() ? (
        <RichText text={value} />
      ) : (
        <Text tone="muted" as="p" className={styles.empty}>
          Nothing here yet. {purpose}
        </Text>
      )}
      {children}
    </section>
  );
}
