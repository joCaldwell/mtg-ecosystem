import type { ReactNode } from "react";
import styles from "./EmptyState.module.css";

export interface EmptyStateProps {
  title: ReactNode;
  /** What would put something here. */
  children?: ReactNode;
  /** When the empty state is all the page has, its title is the page heading. */
  heading?: "h1" | "h2";
}

export function EmptyState({ title, children, heading }: EmptyStateProps) {
  const Title = heading ?? "div";
  return (
    <div className={styles.empty}>
      <Title className={styles.title}>{title}</Title>
      {children ? <div className={styles.body}>{children}</div> : null}
    </div>
  );
}
