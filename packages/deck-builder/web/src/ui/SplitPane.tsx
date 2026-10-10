import type { ReactNode } from "react";
import { cx } from "./internal.ts";
import styles from "./SplitPane.module.css";

export interface SplitPaneProps {
  list: ReactNode;
  detail: ReactNode;
  className?: string;
}

/** A list column on the left and a detail column on the right; each scrolls on its own. */
export function SplitPane({ list, detail, className }: SplitPaneProps) {
  return (
    <div className={cx(styles.split, className)}>
      <div className={styles.list}>{list}</div>
      <div className={styles.detail}>{detail}</div>
    </div>
  );
}
