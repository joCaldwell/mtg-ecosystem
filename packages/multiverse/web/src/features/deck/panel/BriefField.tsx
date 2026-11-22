// One brief field: prose for readers, an on-demand editor for the owner.
import { lazy, Suspense, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { Envelope } from "../../../api/client.ts";
import { RichText } from "../../../mtg/index.ts";
import { Button, Eyebrow, Loading, Text } from "../../../ui/index.ts";
import styles from "./BriefField.module.css";

const BriefEditor = lazy(() => import("./BriefEditor.tsx").then(m => ({ default: m.BriefEditor })));
interface BriefFieldProps {
  readOnly?: boolean;
  title: string;
  value: string;
  /** What the field is for, shown when empty and in the editor. */
  purpose: string;
  onSave: (next: string) => Promise<Envelope>;
  children?: ReactNode;
  /** false removes the editing action altogether on shared/phone pages. */
  editable?: boolean;
}
export function BriefField({ title, value, purpose, onSave, children, readOnly = false, editable = true }: BriefFieldProps) {
  const [editing, setEditing] = useState(false);
  const field = useRef<HTMLElement>(null);
  const edit = useRef<HTMLButtonElement>(null);
  const restoreFocus = useRef(false);
  useLayoutEffect(() => {
    if (!editing && restoreFocus.current) {
      restoreFocus.current = false;
      edit.current?.focus({ preventScroll: true });
    }
  }, [editing]);
  function close() {
    const active = document.activeElement;
    restoreFocus.current = active === document.body || !!field.current?.contains(active);
    setEditing(false);
  }
  return <section ref={field} className={styles.field}>
    {editing && !readOnly ? <Suspense fallback={<Loading />}>
      <BriefEditor title={title} value={value} purpose={purpose} onSave={onSave} onClose={close} />
    </Suspense> : <>
      <div className={styles.head}>
        <Eyebrow as="h2">{title}</Eyebrow>
        {editable && <div className={styles.actions}><Button ref={edit} size="sm" reach onClick={() => setEditing(true)} disabled={readOnly} aria-label={`Edit the ${title.toLowerCase()}`}>Edit</Button></div>}
      </div>
      {value.trim() ? <RichText text={value} /> : <Text tone="muted" as="p" className={styles.empty}>Nothing here yet. {purpose}</Text>}
    </>}
    {children}
  </section>;
}
