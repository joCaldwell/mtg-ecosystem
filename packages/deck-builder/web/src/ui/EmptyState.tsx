import type { ReactNode } from "react";
import styles from "./EmptyState.module.css";

export interface EmptyStateProps {
  title: ReactNode;
  /** What would put something here. */
  children?: ReactNode;
}

export function EmptyState({ title, children }: EmptyStateProps) {
  return (
    <div className={styles.empty}>
      <div className={styles.title}>{title}</div>
      {children ? <div className={styles.body}>{children}</div> : null}
    </div>
  );
}
