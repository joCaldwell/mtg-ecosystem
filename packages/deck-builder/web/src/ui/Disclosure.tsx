import { useId, type ReactNode } from "react";
import { Icon } from "./Icon.tsx";
import styles from "./Disclosure.module.css";

/** A controlled disclosure. The caller owns persistence and write errors. */
export function Disclosure({ label = "Advanced", open, onChange, disabled = false, inline = false, children }: {
  label?: string; open: boolean; onChange: (open: boolean) => void; disabled?: boolean; inline?: boolean; children: ReactNode;
}) {
  const id = useId();
  return <section className={`${styles.disclosure} ${inline ? styles.inline : ""}`}>
    <button type="button" className={styles.toggle} aria-expanded={open} aria-controls={id} disabled={disabled} onClick={() => onChange(!open)}>
      <Icon name="chevronDown" /><span>{label}</span>
    </button>
    <div id={id} hidden={!open} className={styles.content}>{children}</div>
  </section>;
}
