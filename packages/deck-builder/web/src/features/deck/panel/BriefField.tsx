// One brief field: prose for readers, an on-demand editor for the owner.
import { lazy, Suspense, useState, type ReactNode } from "react";
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
  /** false leaves the draft open, with the deck's error visible. */
  onSave: (next: string) => Promise<boolean>;
  children?: ReactNode;
  /** false removes the editing action altogether on shared/phone pages. */
  editable?: boolean;
}
export function BriefField({ title, value, purpose, onSave, children, readOnly = false, editable = true }: BriefFieldProps) {
  const [editing, setEditing] = useState(false);
  return <section className={styles.field}>
    {editing && !readOnly ? <Suspense fallback={<Loading />}>
      <BriefEditor title={title} value={value} purpose={purpose} onSave={onSave} onClose={() => setEditing(false)} />
    </Suspense> : <>
      <div className={styles.head}>
        <Eyebrow>{title}</Eyebrow>
        {editable && <div className={styles.actions}><Button size="sm" onClick={() => setEditing(true)} disabled={readOnly} aria-label={`Edit the ${title.toLowerCase()}`}>Edit</Button></div>}
      </div>
      {value.trim() ? <RichText text={value} /> : <Text tone="muted" as="p" className={styles.empty}>Nothing here yet. {purpose}</Text>}
    </>}
    {children}
  </section>;
}
