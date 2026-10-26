import { useId, useState, useRef, useEffect, type ReactNode } from "react";
import { Button } from "./Button.tsx";
import { Popover } from "./Popover.tsx";
import styles from "./Explainer.module.css";

/** Help at the point of use: one sentence, then an explicit learn-more action. */
export function Explainer({ title, summary, children }: { title: string; summary: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const id = useId();
  const content = useRef<HTMLElement>(null);
  useEffect(() => { if (open) content.current?.focus(); }, [open]);
  const close = () => { setOpen(false); anchor?.focus(); };
  return <div className={styles.explainer}>
    <span>{summary} </span>
    <button ref={setAnchor} type="button" className={styles.learn} aria-expanded={open} aria-controls={open ? id : undefined} onClick={() => setOpen(!open)}>Learn more<span className={styles.sr}> about {title.toLowerCase()}</span></button>
    <Popover anchor={anchor} open={open} onClose={close}>
      <section ref={content} tabIndex={-1} id={id} role="region" aria-label={title} className={styles.panel}>
        <h3>{title}</h3><div>{children}</div><Button size="sm" onClick={close}>Close explanation</Button>
      </section>
    </Popover>
  </div>;
}
