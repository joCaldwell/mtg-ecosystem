import type { ReactNode } from "react";
import styles from "./PageHeader.module.css";

export interface PageHeaderProps {
  title: ReactNode;
  /** Compact state beside the title, rather than explanatory heading copy. */
  meta?: ReactNode;
  actions?: ReactNode;
}

export function PageHeader({ title, meta, actions }: PageHeaderProps) {
  return (
    <header className={styles.header}>
      <div className={styles.frame}>
        <div className={styles.text}>
          <div className={styles.heading}>
            <h1 className={styles.title}>{title}</h1>
            {meta ? <div className={styles.meta}>{meta}</div> : null}
          </div>
        </div>
        {actions ? <div className={styles.actions}>{actions}</div> : null}
      </div>
    </header>
  );
}
